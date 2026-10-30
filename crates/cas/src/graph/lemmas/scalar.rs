// SPDX-License-Identifier: AGPL-3.0-only
//! Complete scalar inventories are staged until their enclosing receipt succeeds.

use std::sync::Arc;

use super::{AnonymousLemma, Cas, Edge, EdgeIndex, LogicalId, State, Undo, Vertex, VertexKind};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::exact_constant::ExactConstant;
use crate::graph::{size, storage_check};

/// Preserve the original scalar retention accounting and refusal location.
const AT: Location = Location::new("checked scalar records");

/// Canonical statement bytes and checked meanings have one allocation shared with the index.
#[derive(Clone, Debug)]
pub(crate) struct ScalarInventory {
    /// Complete original certificate bytes; the index and vertex separately carry the kind tag.
    pub(crate) key: Arc<[u8]>,
    /// Exact meanings in their original record order.
    pub(crate) values: Arc<Vec<ExactConstant>>,
    /// Original expression, context and actual input family, authenticated before staging.
    pub(crate) scope: [LogicalId; 3],
    /// Existing conservative key, value and fixed metadata allowance.
    pub(crate) bytes: u64,
}

/// Only a successful enclosing attempt can turn staged evidence into a graph node.
#[derive(Debug)]
pub(crate) enum PendingJudgment {
    /// A new complete statement with independently checked values.
    NewScalar(ScalarInventory),
    /// An earlier node independently rechecked after rollback or store copying.
    RecheckedScalar(LogicalId),
    /// Complete newly checked finite Machin argument.
    NewPi(super::PiInventory),
    /// Earlier Machin node independently rechecked in this namespace.
    RecheckedPi(LogicalId),
}

/// Graph-owned scalar evidence carries the same namespace authentication as constants.
#[derive(Clone, Debug)]
pub(crate) struct ScalarLemma {
    /// The complete checked statement and ordered result inventory.
    pub(super) inventory: ScalarInventory,
    /// Strong store identity is excluded from portable mathematical bytes.
    owner: Arc<()>,
    /// Namespace in which the complete inventory was independently checked.
    epoch: u64,
    /// Original-expression-to-lemma checked-evidence edge.
    citation: EdgeIndex<u32>,
}

impl Cas {
    /// Authenticate the actual graph payload and its incoming checked-evidence edge.
    pub(crate) fn scalar_inventory(&self, id: LogicalId) -> Result<&ScalarInventory, Fault> {
        Ok(&self.scalar_lemma(id)?.inventory)
    }

    /// A retained key can authorize reuse only in its independently checked namespace.
    pub(crate) fn scalar_records(
        &self,
        key: &[u8],
    ) -> Result<Option<Arc<Vec<ExactConstant>>>, Fault> {
        let Some(id) = self.state.scalar_judgments.get(key) else {
            return Ok(None);
        };
        let node = self.scalar_lemma(*id)?;
        if node.inventory.key.as_ref() != key {
            return Err(invalid());
        }
        let owned = Arc::ptr_eq(&node.owner, &self.memo_owner);
        let current = node.epoch == self.memo_epoch;
        let reusable = self.memo_epoch != u64::MAX;
        Ok((owned && current && reusable).then(|| Arc::clone(&node.inventory.values)))
    }

    /// Complete authentication never substitutes an index entry for the graph's checked edge.
    fn scalar_lemma(&self, id: LogicalId) -> Result<&ScalarLemma, Fault> {
        let Vertex::AnonymousLemma(node) = self.state.node(id)? else {
            return Err(invalid());
        };
        let AnonymousLemma::Scalar(node) = node.as_ref() else {
            return Err(invalid());
        };
        let original = self.state.indices[&node.inventory.scope[0]];
        let target = self.state.indices[&id];
        let endpoints = self.state.graph.edge_endpoints(node.citation) == Some((original, target));
        let checked = matches!(
            self.state.graph.edge_weight(node.citation),
            Some(Edge::CheckedEvidence)
        );
        if !(endpoints && checked) {
            return Err(invalid());
        }
        Ok(node)
    }

    /// Preflight the entire staged suffix before publishing any node or refreshing any epoch.
    pub(crate) fn publish_staged_judgments(
        &mut self,
        pending: &mut Vec<PendingJudgment>,
        budget: &Budget,
    ) -> Result<u64, Fault> {
        if pending.is_empty() {
            return Ok(0);
        }
        let count = pending
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    PendingJudgment::NewScalar(_) | PendingJudgment::NewPi(_)
                )
            })
            .count();
        let bytes = pending.iter().fold(0_u64, |total, entry| match entry {
            PendingJudgment::NewScalar(inventory) => total.saturating_add(inventory.bytes),
            PendingJudgment::NewPi(inventory) => total.saturating_add(inventory.bytes),
            PendingJudgment::RecheckedScalar(_) | PendingJudgment::RecheckedPi(_) => total,
        });
        preflight(self, pending, count, bytes, budget)?;
        if let Some(journal) = &mut self.state.journal {
            journal
                .try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
        }
        let mut serial = self
            .state
            .next
            .get(&VertexKind::AnonymousLemma)
            .copied()
            .unwrap_or(0);
        for entry in pending.iter() {
            let id = LogicalId {
                kind: VertexKind::AnonymousLemma,
                serial,
            };
            let undo = match entry {
                PendingJudgment::NewScalar(inventory) => Undo::ScalarJudgment {
                    id,
                    key: Arc::clone(&inventory.key),
                },
                PendingJudgment::NewPi(inventory) => Undo::PiJudgment {
                    id,
                    certificate: Arc::clone(&inventory.certificate),
                },
                PendingJudgment::RecheckedScalar(_) | PendingJudgment::RecheckedPi(_) => continue,
            };
            self.state.record(undo)?;
            serial += 1;
        }
        for entry in std::mem::take(pending) {
            match entry {
                PendingJudgment::NewScalar(inventory) => append(self, inventory),
                PendingJudgment::NewPi(inventory) => super::pi::append(self, inventory),
                PendingJudgment::RecheckedPi(id) => self.refresh_pi(id),
                PendingJudgment::RecheckedScalar(id) => {
                    let index = self.state.indices[&id];
                    if let Vertex::AnonymousLemma(node) = &mut self.state.graph[index]
                        && let AnonymousLemma::Scalar(node) = node.as_mut()
                    {
                        node.owner = Arc::clone(&self.memo_owner);
                        node.epoch = self.memo_epoch;
                    }
                }
            }
        }
        Ok(bytes)
    }
}

/// All fallible bounds and original references precede the first graph edit.
fn preflight(
    cas: &Cas,
    pending: &[PendingJudgment],
    count: usize,
    bytes: u64,
    budget: &Budget,
) -> Result<(), Fault> {
    let count = size(count, AT)?;
    let nodes = size(cas.state.graph.node_count(), AT)?.saturating_add(count);
    let added_edges = pending.iter().fold(0_u64, |total, entry| {
        total.saturating_add(match entry {
            PendingJudgment::NewScalar(_) => 4,
            PendingJudgment::NewPi(_) => 2,
            PendingJudgment::RecheckedScalar(_) | PendingJudgment::RecheckedPi(_) => 0,
        })
    });
    let edges = size(cas.state.graph.edge_count(), AT)?.saturating_add(added_edges);
    for (kind, amount) in [
        (BudgetKind::Vertices, nodes),
        (BudgetKind::StagedVertices, nodes),
        (BudgetKind::Edges, edges),
        (BudgetKind::StagedEdges, edges),
        (
            BudgetKind::PayloadBytes,
            cas.state.payload.saturating_add(bytes),
        ),
    ] {
        storage_check(&cas.limits, budget, kind, amount, AT)?;
    }
    if nodes >= u64::from(u32::MAX) || edges >= u64::from(u32::MAX) {
        return Err(Fault::capacity(BudgetKind::Vertices, AT));
    }
    cas.state
        .next
        .get(&VertexKind::AnonymousLemma)
        .copied()
        .unwrap_or(0)
        .checked_add(count)
        .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?;
    for entry in pending {
        match entry {
            PendingJudgment::NewScalar(inventory) => {
                for id in inventory.scope {
                    cas.state.node(id)?;
                }
            }
            PendingJudgment::NewPi(inventory) => {
                cas.state.expression(inventory.original)?;
                let _validated = inventory.certificate.canonical_key()?;
            }
            PendingJudgment::RecheckedPi(id) => {
                cas.pi_inventory(*id)?;
            }
            PendingJudgment::RecheckedScalar(id) => {
                cas.scalar_lemma(*id)?;
            }
        }
    }
    Ok(())
}

/// The already checked payload moves once into the authoritative graph after complete preflight.
fn append(cas: &mut Cas, inventory: ScalarInventory) {
    let state: &mut State = &mut cas.state;
    let serial = state
        .next
        .get(&VertexKind::AnonymousLemma)
        .copied()
        .unwrap_or(0);
    let id = LogicalId {
        kind: VertexKind::AnonymousLemma,
        serial,
    };
    let [original, context, family] = inventory.scope.map(|id| state.indices[&id]);
    let key = Arc::clone(&inventory.key);
    let bytes = inventory.bytes;
    let citation = EdgeIndex::new(state.graph.edge_count() + 3);
    let index = state
        .graph
        .add_node(Vertex::AnonymousLemma(Box::new(AnonymousLemma::Scalar(
            ScalarLemma {
                inventory,
                owner: Arc::clone(&cas.memo_owner),
                epoch: cas.memo_epoch,
                citation,
            },
        ))));
    state.graph.add_edge(index, original, Edge::Input(0));
    state.graph.add_edge(index, context, Edge::InContext);
    state.graph.add_edge(index, family, Edge::Input(1));
    state.graph.add_edge(original, index, Edge::CheckedEvidence);
    state.indices.insert(id, index);
    state.scalar_judgments.insert(key, id);
    state.next.insert(VertexKind::AnonymousLemma, serial + 1);
    state.payload = state.payload.saturating_add(bytes);
    state.judgment_bytes = state.judgment_bytes.saturating_add(bytes);
    state.judgment_count += 1;
}

/// Foreign keys, wrong node kinds and missing evidence edges cannot supply checked authority.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
