// SPDX-License-Identifier: AGPL-3.0-only
//! Complete scalar inventories are staged until their enclosing receipt succeeds.

use std::cmp::Ordering;
use std::sync::Arc;

use super::{AnonymousLemma, Cas, Edge, EdgeIndex, LogicalId, State, Undo, Vertex, VertexKind};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::exact_constant::ExactConstant;
use crate::graph::{NodeIndex, StoredKey, size, storage_check};

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
    /// Complete newly checked original-scope enclosure.
    NewEnclosure(super::EnclosureInventory),
    /// Newly checked native exponential schedule.
    NewExpm1(super::Expm1Inventory),
    /// Native schedule independently replayed in this namespace.
    RecheckedExpm1(LogicalId),
    /// Earlier enclosure independently rechecked in this namespace.
    RecheckedEnclosure(LogicalId),
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

/// Locally, each new ID is larger than earlier IDs of its kind, and each planned
/// key position includes only preceding keys in this batch. Therefore prepared
/// insertions replay in plan order without another lookup or a fallible allocation.
pub(super) struct NewPlan {
    /// Reserved logical identifier for the new judgment.
    pub(super) id: LogicalId,
    /// Prepared position in the graph identifier index.
    pub(super) index_position: usize,
    /// Prepared position in the retained-kind count index.
    pub(super) retained_position: usize,
    /// Count after publishing this judgment.
    pub(super) retained_next: u64,
    /// Prepared position in the judgment key index.
    pub(super) key_position: usize,
    /// Resolved graph sources for the judgment.
    pub(super) sources: [Option<NodeIndex<u32>>; 3],
    /// Retained key allocation when this judgment interns one.
    pub(super) intern_key: Option<StoredKey>,
    /// Prepared inverse insertion credit for rollback.
    undo: Option<Undo>,
}

/// Prepared publication action for a new or refreshed judgment.
enum PublicationPlan {
    /// Publish a new judgment using the reserved positions.
    New(NewPlan),
    /// Refresh an already retained graph node.
    Refresh(NodeIndex<u32>),
}

/// Copy an encoded judgment key into a fallibly reserved allocation.
fn copy_key(bytes: &[u8]) -> Result<StoredKey, Fault> {
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(bytes.len())
        .map_err(|_| Fault::allocation(AT))?;
    copied.extend_from_slice(bytes);
    Ok(copied.into())
}

impl Cas {
    /// Authenticate the actual graph payload and its incoming checked-evidence edge.
    pub(crate) fn scalar_inventory(
        &self,
        id: LogicalId,
        budget: &mut Budget,
    ) -> Result<&ScalarInventory, Fault> {
        Ok(&self.scalar_lemma(id, budget)?.inventory)
    }

    /// A retained key can authorize reuse only in its independently checked namespace.
    pub(crate) fn scalar_records(
        &self,
        key: &[u8],
        budget: &mut Budget,
    ) -> Result<Option<Arc<Vec<ExactConstant>>>, Fault> {
        let count = size(self.state.scalar_judgments.len(), AT)?;
        let length = size(key.len(), AT)?;
        let floor = length
            .checked_mul(u64::from(count.bit_width()))
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
        let Some(id) = self.state.scalar_judgments.get(key, floor, budget, AT)? else {
            return Ok(None);
        };
        let node = self.scalar_lemma(*id, budget)?;
        if node.inventory.key.as_ref() != key {
            return Err(invalid());
        }
        let owned = Arc::ptr_eq(&node.owner, &self.memo_owner);
        let current = node.epoch == self.memo_epoch;
        let reusable = self.memo_epoch != u64::MAX;
        Ok((owned && current && reusable).then(|| Arc::clone(&node.inventory.values)))
    }

    /// Complete authentication never substitutes an index entry for the graph's checked edge.
    fn scalar_lemma(&self, id: LogicalId, budget: &mut Budget) -> Result<&ScalarLemma, Fault> {
        let Vertex::AnonymousLemma(node) = self.state.node(id, budget)? else {
            return Err(invalid());
        };
        let AnonymousLemma::Scalar(node) = node.as_ref() else {
            return Err(invalid());
        };
        let original = self.state.index(node.inventory.scope[0], budget)?;
        let target = self.state.index(id, budget)?;
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
        budget: &mut Budget,
    ) -> Result<u64, Fault> {
        if pending.is_empty() {
            return Ok(0);
        }
        let count = pending
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    PendingJudgment::NewScalar(_)
                        | PendingJudgment::NewPi(_)
                        | PendingJudgment::NewEnclosure(_)
                        | PendingJudgment::NewExpm1(_)
                )
            })
            .count();
        let bytes = pending.iter().fold(0_u64, |total, entry| match entry {
            PendingJudgment::NewScalar(inventory) => total.saturating_add(inventory.bytes),
            PendingJudgment::NewPi(inventory) => total.saturating_add(inventory.bytes),
            PendingJudgment::NewEnclosure(inventory) => total.saturating_add(inventory.bytes),
            PendingJudgment::NewExpm1(inventory) => total.saturating_add(inventory.bytes),
            PendingJudgment::RecheckedScalar(_)
            | PendingJudgment::RecheckedPi(_)
            | PendingJudgment::RecheckedExpm1(_)
            | PendingJudgment::RecheckedEnclosure(_) => total,
        });
        preflight(self, pending, count, bytes, budget)?;
        let first =
            self.token
                .identities
                .reserve(VertexKind::AnonymousLemma, size(count, AT)?, AT)?;
        let mut plan = prepare_publication(self, pending, first, budget)?;
        if let Some(journal) = &mut self.state.journal {
            journal
                .try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
        }
        for prepared in &mut plan {
            if let PublicationPlan::New(prepared) = prepared {
                self.state
                    .record(prepared.undo.take().ok_or_else(invalid)?)?;
            }
        }
        self.append_staged_judgments(std::mem::take(pending), plan);
        Ok(bytes)
    }

    /// Publish only the fully reserved and journaled suffix, with no fallible steps remaining.
    fn append_staged_judgments(
        &mut self,
        pending: Vec<PendingJudgment>,
        plan: Vec<PublicationPlan>,
    ) {
        for (entry, prepared) in pending.into_iter().zip(plan) {
            match (entry, prepared) {
                (PendingJudgment::NewScalar(inventory), PublicationPlan::New(plan)) => {
                    append(self, inventory, &plan);
                }
                (PendingJudgment::NewPi(inventory), PublicationPlan::New(plan)) => {
                    super::pi::append(self, inventory, &plan);
                }
                (PendingJudgment::NewEnclosure(inventory), PublicationPlan::New(plan)) => {
                    super::enclosure::append(self, inventory, plan);
                }
                (PendingJudgment::NewExpm1(inventory), PublicationPlan::New(plan)) => {
                    super::prepared_expm1::append(self, inventory, plan);
                }
                (PendingJudgment::RecheckedExpm1(_), PublicationPlan::Refresh(index)) => {
                    self.refresh_expm1(index);
                }
                (PendingJudgment::RecheckedEnclosure(_), PublicationPlan::Refresh(index)) => {
                    self.refresh_enclosure(index);
                }
                (PendingJudgment::RecheckedPi(_), PublicationPlan::Refresh(index)) => {
                    self.refresh_pi(index);
                }
                (PendingJudgment::RecheckedScalar(_), PublicationPlan::Refresh(index)) => {
                    if let Vertex::AnonymousLemma(node) = &mut self.state.graph[index]
                        && let AnonymousLemma::Scalar(node) = node.as_mut()
                    {
                        node.owner = Arc::clone(&self.memo_owner);
                        node.epoch = self.memo_epoch;
                    }
                }
                _ => unreachable!("preflight plan follows staged entry order"),
            }
        }
    }
}

/// Resolve every target and reserve every sorted insertion before publication begins.
#[expect(
    clippy::too_many_lines,
    reason = "one ordered preflight covers all judgment variants and their shared reservations"
)]
fn prepare_publication(
    cas: &mut Cas,
    pending: &[PendingJudgment],
    first: u64,
    budget: &mut Budget,
) -> Result<Vec<PublicationPlan>, Fault> {
    let state = &mut cas.state;
    let mut plans = Vec::new();
    plans
        .try_reserve_exact(pending.len())
        .map_err(|_| Fault::allocation(AT))?;
    let mut fresh = 0_usize;
    let mut count_slot = None;
    for (slot, entry) in pending.iter().enumerate() {
        let scope: &[LogicalId] = match entry {
            PendingJudgment::NewScalar(value) => &value.scope,
            PendingJudgment::NewPi(value) => std::slice::from_ref(&value.original),
            PendingJudgment::NewEnclosure(value) => &value.scope,
            PendingJudgment::NewExpm1(value) => &value.scope,
            PendingJudgment::RecheckedScalar(id)
            | PendingJudgment::RecheckedPi(id)
            | PendingJudgment::RecheckedEnclosure(id)
            | PendingJudgment::RecheckedExpm1(id) => {
                plans.push(PublicationPlan::Refresh(state.index(*id, budget)?));
                continue;
            }
        };
        let mut sources = [None; 3];
        for (target, id) in sources.iter_mut().zip(scope) {
            *target = Some(state.index(*id, budget)?);
        }
        if matches!(entry, PendingJudgment::NewPi(_))
            && !matches!(
                sources[0].and_then(|index| state.graph.node_weight(index)),
                Some(Vertex::Expression(_))
            )
        {
            return Err(invalid());
        }
        let key_position = match entry {
            PendingJudgment::NewScalar(value) => {
                let Err(base) = state
                    .scalar_judgments
                    .find(value.key.as_ref(), 0, budget, AT)?
                else {
                    return Err(invalid());
                };
                let mut prior = 0_usize;
                let mut before = 0_usize;
                for previous in &pending[..slot] {
                    if let PendingJudgment::NewScalar(previous) = previous {
                        prior = prior
                            .checked_add(1)
                            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, AT))?;
                        match previous.key.as_ref().cmp(value.key.as_ref()) {
                            Ordering::Less => {
                                before = before
                                    .checked_add(1)
                                    .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, AT))?;
                            }
                            Ordering::Equal => return Err(invalid()),
                            Ordering::Greater => {}
                        }
                    }
                }
                let position = base
                    .checked_add(before)
                    .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, AT))?;
                state.scalar_judgments.prepare_virtual_insert(
                    state
                        .scalar_judgments
                        .len()
                        .checked_add(prior)
                        .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, AT))?,
                    position,
                    state.journal.is_some(),
                    budget,
                    AT,
                )?;
                position
            }
            PendingJudgment::NewPi(value) => state.pi_judgments.prepare_virtual_insert(
                &value.certificate,
                pending[..slot].iter().filter_map(|entry| match entry {
                    PendingJudgment::NewPi(prior) => Some(prior.certificate.as_ref()),
                    _ => None,
                }),
                state.journal.is_some(),
                budget,
            )?,
            PendingJudgment::NewEnclosure(value) => state.intern.prepare_virtual_insert_bytes(
                &value.key,
                pending[..slot].iter().filter_map(|entry| match entry {
                    PendingJudgment::NewEnclosure(prior) => Some(prior.key.as_slice()),
                    PendingJudgment::NewExpm1(prior) => Some(prior.key.as_ref()),
                    _ => None,
                }),
                state.journal.is_some(),
                budget,
                AT,
            )?,
            PendingJudgment::NewExpm1(value) => state.intern.prepare_virtual_insert_bytes(
                &value.key,
                pending[..slot].iter().filter_map(|entry| match entry {
                    PendingJudgment::NewEnclosure(prior) => Some(prior.key.as_slice()),
                    PendingJudgment::NewExpm1(prior) => Some(prior.key.as_ref()),
                    _ => None,
                }),
                state.journal.is_some(),
                budget,
                AT,
            )?,
            _ => unreachable!("fresh entry matched above"),
        };
        let id = LogicalId {
            kind: VertexKind::AnonymousLemma,
            serial: first
                .checked_add(
                    u64::try_from(fresh).map_err(|_| Fault::capacity(BudgetKind::Vertices, AT))?,
                )
                .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?,
        };
        let Err(base) = state.indices.find(&id, 0, budget, AT)? else {
            return Err(invalid());
        };
        let index_position = base
            .checked_add(fresh)
            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?;
        state.indices.prepare_virtual_insert(
            state
                .indices
                .len()
                .checked_add(fresh)
                .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?,
            index_position,
            state.journal.is_some(),
            budget,
            AT,
        )?;
        let (retained_position, retained_next) = if let Some((position, next)) = count_slot {
            budget.work(2, AT)?;
            (position, next)
        } else {
            state.prepare_retain(VertexKind::AnonymousLemma, budget)?
        };
        count_slot = Some((
            retained_position,
            retained_next
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?,
        ));
        let (intern_key, undo) = match entry {
            PendingJudgment::NewScalar(value) => (
                None,
                Undo::ScalarJudgment {
                    id,
                    key: Arc::clone(&value.key),
                },
            ),
            PendingJudgment::NewPi(value) => (
                None,
                Undo::PiJudgment {
                    id,
                    certificate: Arc::clone(&value.certificate),
                },
            ),
            PendingJudgment::NewEnclosure(value) => (
                Some(copy_key(&value.key)?),
                Undo::Vertex {
                    id,
                    key: Some(copy_key(&value.key)?),
                    policy: None,
                },
            ),
            PendingJudgment::NewExpm1(value) => (
                Some(Arc::clone(&value.key).into()),
                Undo::Vertex {
                    id,
                    key: Some(Arc::clone(&value.key).into()),
                    policy: None,
                },
            ),
            _ => unreachable!("fresh entry matched above"),
        };
        plans.push(PublicationPlan::New(NewPlan {
            id,
            index_position,
            retained_position,
            retained_next,
            key_position,
            sources,
            intern_key,
            undo: Some(undo),
        }));
        fresh = fresh
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?;
    }
    Ok(plans)
}

/// All fallible bounds and original references precede the first graph edit.
fn preflight(
    cas: &Cas,
    pending: &[PendingJudgment],
    count: usize,
    bytes: u64,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = size(count, AT)?;
    let nodes = size(cas.state.graph.node_count(), AT)?.saturating_add(count);
    let added_edges = pending.iter().fold(0_u64, |total, entry| {
        total.saturating_add(match entry {
            PendingJudgment::NewScalar(_) => 4,
            PendingJudgment::NewPi(_) => 2,
            PendingJudgment::NewEnclosure(_) | PendingJudgment::NewExpm1(_) => 3,
            PendingJudgment::RecheckedScalar(_)
            | PendingJudgment::RecheckedPi(_)
            | PendingJudgment::RecheckedExpm1(_)
            | PendingJudgment::RecheckedEnclosure(_) => 0,
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
    for entry in pending {
        match entry {
            PendingJudgment::NewEnclosure(_) | PendingJudgment::NewExpm1(_) => {
                let total = cas.state.judgment_bytes.saturating_add(bytes);
                budget.check(BudgetKind::OutputBytes, total, AT)?;
                let unregistered = cas.judgment_reservation(total, budget, AT)?;
                budget.check(BudgetKind::StagedPayloadBytes, unregistered, AT)?;
            }
            PendingJudgment::RecheckedEnclosure(id) => {
                cas.enclosure_inventory(*id, budget)?;
            }
            PendingJudgment::RecheckedExpm1(id) => {
                cas.expm1_inventory(*id, budget)?;
            }
            PendingJudgment::NewScalar(_) => {}
            PendingJudgment::NewPi(inventory) => {
                let _validated = inventory.certificate.canonical_key()?;
            }
            PendingJudgment::RecheckedPi(id) => {
                cas.pi_inventory(*id, budget)?;
            }
            PendingJudgment::RecheckedScalar(id) => {
                cas.scalar_lemma(*id, budget)?;
            }
        }
    }
    Ok(())
}

/// The already checked payload moves once into the authoritative graph after complete preflight.
fn append(cas: &mut Cas, inventory: ScalarInventory, plan: &NewPlan) {
    let state: &mut State = &mut cas.state;
    let [Some(original), Some(context), Some(family)] = plan.sources else {
        unreachable!("preflight resolved scalar scope")
    };
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
    state
        .indices
        .insert_prepared(plan.index_position, plan.id, index);
    state
        .scalar_judgments
        .insert_prepared(plan.key_position, key, plan.id);
    state.retain_prepared(
        VertexKind::AnonymousLemma,
        plan.retained_position,
        plan.retained_next,
    );
    state.payload = state.payload.saturating_add(bytes);
    state.judgment_bytes = state.judgment_bytes.saturating_add(bytes);
    state.judgment_count += 1;
}

/// Foreign keys, wrong node kinds and missing evidence edges cannot supply checked authority.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
