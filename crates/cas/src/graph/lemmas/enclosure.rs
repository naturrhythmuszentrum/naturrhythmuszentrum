// SPDX-License-Identifier: AGPL-3.0-only
//! Enclosure receipts join the shared staged anonymous-judgment publication path.

use super::{AnonymousLemma, Cas, Edge, EdgeIndex, LogicalId, PendingJudgment, Vertex, VertexKind};
use crate::budget::{Budget, BudgetKind};
use crate::enclosure::CheckedEnclosure;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::size;
use std::sync::Arc;

/// Preserve the enclosure retention accounting boundary.
const AT: Location = Location::new("exact constant cache");

/// Complete checked statement and its authenticated original scope await final publication.
#[derive(Clone, Debug)]
pub(crate) struct EnclosureInventory {
    /// Kind-qualified portable request, grids and complete transcript.
    pub(super) key: Vec<u8>,
    /// Independently checked result; never admitted from untrusted decoding alone.
    value: Box<CheckedEnclosure>,
    /// Original target and context in this store.
    pub(super) scope: [LogicalId; 2],
    /// Complete key and checked payload accounting.
    pub(super) bytes: u64,
}

/// Graph evidence authenticates ownership, rollback namespace and the actual citation edge.
#[derive(Clone, Debug)]
pub(crate) struct EnclosureLemma {
    /// Complete checked statement retained by the graph itself.
    pub(super) inventory: EnclosureInventory,
    /// Store identity is runtime authority, not mathematical content.
    owner: Arc<()>,
    /// Last independently checked namespace.
    epoch: u64,
    /// Original-target-to-judgment evidence edge.
    citation: EdgeIndex<u32>,
}

impl Cas {
    /// Borrow only current authenticated evidence; stale namespaces must independently replay.
    pub(crate) fn retained_enclosure(
        &self,
        key: &[u8],
        budget: &mut Budget,
    ) -> Result<Option<&CheckedEnclosure>, Fault> {
        budget.work(
            size(key.len(), AT)?.saturating_mul(size(self.state.indices.len(), AT)?.max(1)),
            AT,
        )?;
        let existing = self.state.indices.iter().find_map(|(id, index)| {
            if let Vertex::AnonymousLemma(node) = &self.state.graph[*index]
                && let AnonymousLemma::Enclosure(node) = node.as_ref()
                && node.inventory.key.starts_with(key)
            {
                Some(*id)
            } else {
                None
            }
        });
        let Some(id) = existing else {
            return Ok(None);
        };
        let node = self.enclosure_lemma(id)?;
        if node.epoch != self.memo_epoch
            || self.memo_epoch == u64::MAX
            || !Arc::ptr_eq(&node.owner, &self.memo_owner)
        {
            return Ok(None);
        }
        retained(self.state.judgment_bytes, budget)?;
        Ok(Some(&node.inventory.value))
    }

    /// Stage a checked receipt after the enclosing operation's final fallible copy.
    pub(crate) fn stage_enclosure(
        &self,
        key: Vec<u8>,
        value: CheckedEnclosure,
        budget: &mut Budget,
    ) -> Result<PendingJudgment, Fault> {
        let bytes = crate::enclosure::footprint(&value).saturating_add(size(key.len(), AT)?);
        budget.work(bytes, AT)?;
        if let Some(id) = self.state.intern.get(&key).copied() {
            let old = self.enclosure_inventory(id)?;
            if old.value.certificate() != value.certificate()
                || old.value.request() != value.request()
            {
                return Err(Fault::invalid(InvalidCode::ReuseKeyMismatch, AT));
            }
            retained(self.state.judgment_bytes, budget)?;
            return Ok(PendingJudgment::RecheckedEnclosure(id));
        }
        let scope = [
            self.checked(&value.request().target)?,
            self.checked(&value.request().context)?,
        ];
        Ok(PendingJudgment::NewEnclosure(EnclosureInventory {
            key,
            value: Box::new(value),
            scope,
            bytes,
        }))
    }

    /// Publish through the shared all-before-any preflight, with no fallible suffix.
    pub(crate) fn retain_enclosure(
        &mut self,
        key: Vec<u8>,
        value: CheckedEnclosure,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let pending = self.stage_enclosure(key, value, budget)?;
        self.publish_staged_judgments(&mut vec![pending], budget)?;
        Ok(())
    }

    /// The shared preflight authenticates existing receipts before refreshing their namespace.
    pub(super) fn enclosure_inventory(&self, id: LogicalId) -> Result<&EnclosureInventory, Fault> {
        Ok(&self.enclosure_lemma(id)?.inventory)
    }

    /// A canonical index never substitutes for its graph payload or actual checked edge.
    fn enclosure_lemma(&self, id: LogicalId) -> Result<&EnclosureLemma, Fault> {
        let Vertex::AnonymousLemma(node) = self.state.node(id)? else {
            return Err(invalid());
        };
        let AnonymousLemma::Enclosure(node) = node.as_ref() else {
            return Err(invalid());
        };
        let source = self.state.indices[&node.inventory.scope[0]];
        let target = self.state.indices[&id];
        if self.state.graph.edge_endpoints(node.citation) != Some((source, target))
            || !matches!(
                self.state.graph.edge_weight(node.citation),
                Some(Edge::CheckedEvidence)
            )
        {
            return Err(invalid());
        }
        Ok(node)
    }

    /// Refresh only after complete replay, receipt copying and shared publication preflight.
    pub(super) fn refresh_enclosure(&mut self, id: LogicalId) {
        let index = self.state.indices[&id];
        if let Vertex::AnonymousLemma(node) = &mut self.state.graph[index]
            && let AnonymousLemma::Enclosure(node) = node.as_mut()
        {
            node.owner = Arc::clone(&self.memo_owner);
            node.epoch = self.memo_epoch;
        }
    }
}

/// Move the already preflighted payload into the common graph and rollback journal.
pub(super) fn append(cas: &mut Cas, inventory: EnclosureInventory) {
    let state = &mut cas.state;
    let serial = state
        .next
        .get(&VertexKind::AnonymousLemma)
        .copied()
        .unwrap_or(0);
    let id = LogicalId {
        kind: VertexKind::AnonymousLemma,
        serial,
    };
    let [original, context] = inventory.scope.map(|id| state.indices[&id]);
    let key = inventory.key.clone();
    let bytes = inventory.bytes;
    let citation = EdgeIndex::new(state.graph.edge_count() + 2);
    let index = state
        .graph
        .add_node(Vertex::AnonymousLemma(Box::new(AnonymousLemma::Enclosure(
            EnclosureLemma {
                inventory,
                owner: Arc::clone(&cas.memo_owner),
                epoch: cas.memo_epoch,
                citation,
            },
        ))));
    state.graph.add_edge(index, original, Edge::Input(0));
    state.graph.add_edge(index, context, Edge::InContext);
    state.graph.add_edge(original, index, Edge::CheckedEvidence);
    state.indices.insert(id, index);
    state.intern.insert(key, id, &cas.token);
    state.next.insert(VertexKind::AnonymousLemma, serial + 1);
    state.payload = state.payload.saturating_add(bytes);
    state.judgment_bytes = state.judgment_bytes.saturating_add(bytes);
    state.judgment_count += 1;
}

/// Preserve the complete retained-payload ceilings on reuse and revalidation.
fn retained(bytes: u64, budget: &Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)
}

/// Missing payloads or evidence edges cannot authorize reuse.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
