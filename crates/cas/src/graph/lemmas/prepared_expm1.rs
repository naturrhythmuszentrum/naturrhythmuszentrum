// SPDX-License-Identifier: AGPL-3.0-only
//! Enclosure receipts join the shared staged anonymous-judgment publication path.

use super::{AnonymousLemma, Cas, Edge, EdgeIndex, LogicalId, PendingJudgment, Vertex, VertexKind};
use crate::budget::Budget;
use crate::enclosure::prepared_expm1::Prepared;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::size;
use std::sync::Arc;

/// Preserve the expm1 retention accounting boundary.
const AT: Location = Location::new("exact constant cache");

/// Complete checked statement and its authenticated original scope await final publication.
#[derive(Clone, Debug)]
pub(crate) struct Expm1Inventory {
    /// Kind-qualified portable request, grids and complete transcript.
    pub(super) key: Arc<[u8]>,
    /// Independently checked result; never admitted from untrusted decoding alone.
    value: Box<Prepared>,
    /// Original target and context in this store.
    pub(super) scope: [LogicalId; 2],
    /// Complete key and checked payload accounting.
    pub(super) bytes: u64,
}

/// Graph evidence authenticates ownership, rollback namespace and the actual citation edge.
#[derive(Clone, Debug)]
pub(crate) struct Expm1Lemma {
    /// Complete checked statement retained by the graph itself.
    pub(super) inventory: Expm1Inventory,
    /// Store identity is runtime authority, not mathematical content.
    owner: Arc<()>,
    /// Last independently checked namespace.
    epoch: u64,
    /// Original-target-to-judgment evidence edge.
    citation: EdgeIndex<u32>,
}

impl Cas {
    /// Observe all three production owners while the publication journal is live.
    #[cfg(test)]
    pub(crate) fn expm1_key_owners(&self) -> Result<(std::sync::Weak<[u8]>, bool, bool), Fault> {
        for (id, index) in self.state.indices.iter() {
            if let Vertex::AnonymousLemma(node) = &self.state.graph[*index]
                && let AnonymousLemma::PreparedExpm1(node) = node.as_ref()
            {
                let key = &node.inventory.key;
                let index_shared = self.state.intern.shares_key(*id, key);
                let journal_shared = self.state.journal.as_ref().is_some_and(|journal| journal.iter().any(|entry| {
                    matches!(entry, crate::graph::Undo::Vertex { id: other, key: Some(stored), .. } if other == id && std::ptr::eq(stored.parts()[0], key.as_ref()))
                }));
                return Ok((Arc::downgrade(key), index_shared, journal_shared));
            }
        }
        Err(invalid())
    }

    /// Borrow only current authenticated evidence; stale namespaces must independently replay.
    pub(crate) fn retained_expm1(
        &self,
        key: &[u8],
        budget: &mut Budget,
    ) -> Result<Option<&Prepared>, Fault> {
        budget.work(
            size(key.len(), AT)?.saturating_mul(size(self.state.indices.len(), AT)?.max(1)),
            AT,
        )?;
        for (id, index) in self.state.indices.iter() {
            if let Vertex::AnonymousLemma(node) = &self.state.graph[*index]
                && let AnonymousLemma::PreparedExpm1(node) = node.as_ref()
                && node.inventory.key.starts_with(key)
            {
                let node = self.expm1_lemma(*id, budget)?;
                if node.epoch != self.memo_epoch
                    || self.memo_epoch == u64::MAX
                    || !Arc::ptr_eq(&node.owner, &self.memo_owner)
                {
                    continue;
                }
                super::retained(self, self.state.judgment_bytes, budget)?;
                return Ok(Some(&node.inventory.value));
            }
        }
        Ok(None)
    }

    /// Stage a checked receipt after the enclosing operation's final fallible copy.
    pub(crate) fn stage_expm1(
        &self,
        key: Vec<u8>,
        value: Prepared,
        budget: &mut Budget,
    ) -> Result<PendingJudgment, Fault> {
        let bytes = crate::enclosure::prepared_expm1::footprint(&value)
            .saturating_add(size(key.len(), AT)?);
        budget.work(bytes, AT)?;
        if let Some(id) = self.state.intern.get(&key, 0, budget, AT)?.copied() {
            let old = self.expm1_inventory(id, budget)?;
            if old.value.proof() != value.proof() || old.value.request() != value.request() {
                return Err(Fault::invalid(InvalidCode::ReuseKeyMismatch, AT));
            }
            super::retained(self, self.state.judgment_bytes, budget)?;
            return Ok(PendingJudgment::RecheckedExpm1(id));
        }
        let scope = [
            self.checked(&value.request().original.target, budget)?,
            self.checked(&value.request().original.context, budget)?,
        ];
        budget.with_retained_payload(
            size(key.len(), AT)?,
            Location::new("prepared expm1"),
            |_budget| {
                Ok(PendingJudgment::NewExpm1(Expm1Inventory {
                    key: key.into(),
                    value: Box::new(value),
                    scope,
                    bytes,
                }))
            },
        )
    }

    /// Publish through the shared all-before-any preflight, with no fallible suffix.
    pub(crate) fn retain_expm1(
        &mut self,
        key: Vec<u8>,
        value: Prepared,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let bytes =
            crate::enclosure::prepared_expm1::footprint(&value).saturating_add(key.len() as u64);
        let pending =
            budget.with_retained_payload(bytes, Location::new("prepared expm1"), |budget| {
                self.stage_expm1(key, value, budget)
            })?;
        // Pending ownership transfers to shared graph preflight after staging ends.
        self.publish_staged_judgments(&mut vec![pending], budget)?;
        Ok(())
    }

    /// The shared preflight authenticates existing receipts before refreshing their namespace.
    pub(super) fn expm1_inventory(
        &self,
        id: LogicalId,
        budget: &mut Budget,
    ) -> Result<&Expm1Inventory, Fault> {
        Ok(&self.expm1_lemma(id, budget)?.inventory)
    }

    /// A canonical index never substitutes for its graph payload or actual checked edge.
    fn expm1_lemma(&self, id: LogicalId, budget: &mut Budget) -> Result<&Expm1Lemma, Fault> {
        let Vertex::AnonymousLemma(node) = self.state.node(id, budget)? else {
            return Err(invalid());
        };
        let AnonymousLemma::PreparedExpm1(node) = node.as_ref() else {
            return Err(invalid());
        };
        let source = self.state.index(node.inventory.scope[0], budget)?;
        let target = self.state.index(id, budget)?;
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
    pub(super) fn refresh_expm1(&mut self, index: crate::graph::NodeIndex<u32>) {
        if let Vertex::AnonymousLemma(node) = &mut self.state.graph[index]
            && let AnonymousLemma::PreparedExpm1(node) = node.as_mut()
        {
            node.owner = Arc::clone(&self.memo_owner);
            node.epoch = self.memo_epoch;
        }
    }
}

/// Move the already preflighted payload into the common graph and rollback journal.
pub(super) fn append(cas: &mut Cas, inventory: Expm1Inventory, plan: super::scalar::NewPlan) {
    let state = &mut cas.state;
    let [Some(original), Some(context), _] = plan.sources else {
        unreachable!("preflight resolved expm1 scope")
    };
    let key = plan.intern_key.expect("preflighted expm1 key");
    let bytes = inventory.bytes;
    let citation = EdgeIndex::new(state.graph.edge_count() + 2);
    let index = state.graph.add_node(Vertex::AnonymousLemma(Box::new(
        AnonymousLemma::PreparedExpm1(Expm1Lemma {
            inventory,
            owner: Arc::clone(&cas.memo_owner),
            epoch: cas.memo_epoch,
            citation,
        }),
    )));
    state.graph.add_edge(index, original, Edge::Input(0));
    state.graph.add_edge(index, context, Edge::InContext);
    state.graph.add_edge(original, index, Edge::CheckedEvidence);
    state
        .indices
        .insert_prepared(plan.index_position, plan.id, index);
    state
        .intern
        .insert_prepared_key(plan.key_position, key, plan.id, &cas.token);
    state.retain_prepared(
        VertexKind::AnonymousLemma,
        plan.retained_position,
        plan.retained_next,
    );
    state.payload = state.payload.saturating_add(bytes);
    state.judgment_bytes = state.judgment_bytes.saturating_add(bytes);
    state.judgment_count += 1;
}

/// Missing payloads or evidence edges cannot authorize reuse.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
