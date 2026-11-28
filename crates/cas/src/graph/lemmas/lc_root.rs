// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic graph retention for the complete conditional LC family judgment.

use super::{AnonymousLemma, Cas, Edge, EdgeIndex, LogicalId, Undo, Vertex, VertexKind};
use crate::budget::{Budget, BudgetKind};
use crate::enclosure::lc_root::{Candidate, Request, candidate_bytes};
use crate::graph::{Handle, NodeIndex, StoredKey, size, storage_check};
use crate::{Fault, InvalidCode, Location};
use std::sync::Arc;

/// Registration is distinct from the local mathematical checker.
const AT: Location = Location::new("LC root judgment publication");

/// Full checked claim and shared canonical key, owned once by this graph.
#[derive(Clone, Debug)]
pub(crate) struct LcRootLemma {
    /// One allocation shared by the node, canonical index and rollback journal.
    pub(super) key: Arc<[u8]>,
    /// Complete ordered original claim; edges retain every referenced original.
    candidate: Box<Candidate>,
    /// Original-product-to-judgment checked evidence edge.
    citation: EdgeIndex<u32>,
    /// Complete retained record and key inventory, excluding the original graph.
    pub(super) bytes: u64,
}

impl Cas {
    /// Authenticate the receipt against the live graph and actual checked-evidence edge.
    pub(crate) fn lc_root_request(
        &self,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<&Request, Fault> {
        Ok(&self.lc_root_node(handle, budget)?.candidate.original)
    }

    /// Borrow the portable checked transcript after authenticating its durable owner.
    pub(crate) fn lc_root_wire(
        &self,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<&[u8], Fault> {
        Ok(&self.lc_root_node(handle, budget)?.key[2..])
    }

    /// A rolled-back graph handle never acquires authority from a canonical key.
    fn lc_root_node(&self, handle: &Handle, budget: &mut Budget) -> Result<&LcRootLemma, Fault> {
        let id = self.checked(handle, budget)?;
        let Vertex::AnonymousLemma(node) = self.state.node(id, budget)? else {
            return Err(invalid());
        };
        let AnonymousLemma::LcRoot(node) = node.as_ref() else {
            return Err(invalid());
        };
        let source = self.state.index(
            self.checked(&node.candidate.original.targets[0], budget)?,
            budget,
        )?;
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

    /// The caller has checked the complete candidate; all fallible publication work precedes append.
    #[expect(
        clippy::too_many_lines,
        reason = "the publication preflight and append share one atomic path"
    )]
    pub(crate) fn retain_lc_root(
        &mut self,
        candidate: Candidate,
        wire: &[u8],
        indexed: bool,
        additional_output: u64,
        budget: &mut Budget,
    ) -> Result<(Handle, Arc<[u8]>), Fault> {
        let bytes = candidate_bytes(&candidate)
            .saturating_add(wire.len() as u64)
            .saturating_add(2)
            .saturating_add(size_of::<AnonymousLemma>() as u64)
            .saturating_add(16);
        let count = candidate.original.handles().count();
        let slots = u32::try_from(count).map_err(|_| Fault::capacity(BudgetKind::Edges, AT))?;
        let temporary = (count as u64).saturating_mul(4).saturating_add(24);
        // Vec-to-Arc transfer and graph append are disjoint peaks. Shared key bytes
        // are held once by the graph, index and journal after the transfer finishes.
        let key_bytes = (wire.len() as u64).saturating_add(2);
        let transfer = candidate_bytes(&candidate)
            .saturating_add(key_bytes.saturating_mul(2))
            .saturating_add(16) // Arc allocation's two reference counts.
            .saturating_add(size_of::<Vec<u8>>() as u64)
            .saturating_add(size_of::<Arc<[u8]>>() as u64);
        let append_peak = bytes.saturating_add(temporary);
        let live = self.judgment_reservation(self.state.judgment_bytes, budget, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            live.saturating_add(transfer.max(append_peak)),
            AT,
        )?;
        budget.work(bytes, AT)?;
        let mut key = Vec::new();
        key.try_reserve_exact(wire.len().saturating_add(2))
            .map_err(|_| Fault::allocation(AT))?;
        key.push(VertexKind::AnonymousLemma.tag());
        key.push(6);
        key.extend_from_slice(wire);
        let key: Arc<[u8]> = key.into();
        let floor = size(key.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
        let existing = budget.with_work_phase("lc-root", "lookup", |budget| {
            self.state.intern.get(key.as_ref(), floor, budget, AT)
        })?;
        if let Some(id) = existing.copied() {
            budget.check(
                BudgetKind::OutputBytes,
                self.state.judgment_bytes.saturating_add(additional_output),
                AT,
            )?;
            let handle = self.handle(id);
            if self.lc_root_request(&handle, budget)? != &candidate.original {
                return Err(invalid());
            }
            let proof = if indexed {
                Arc::clone(&self.lc_root_node(&handle, budget)?.key)
            } else {
                key
            };
            return Ok((handle, proof));
        }
        preflight(self, count, bytes, additional_output, budget)?;
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for handle in candidate.original.handles() {
            budget.work(1, AT)?;
            dependencies.push(self.state.index(self.checked(handle, budget)?, budget)?);
        }
        let source = self
            .state
            .index(candidate.original.targets[0].logical_id(), budget)?;
        let stored_key: StoredKey = Arc::clone(&key).into();
        let (intern_position, _inverse_credit) = self.state.intern.prepare_insert_key(
            &stored_key,
            self.state.journal.is_some(),
            budget,
            AT,
        )?;
        let serial = self
            .token
            .identities
            .reserve(VertexKind::AnonymousLemma, 1, AT)?;
        let id = LogicalId {
            kind: VertexKind::AnonymousLemma,
            serial,
        };
        let index_position = self.state.prepare_index_insert(id, budget)?;
        let (count_position, next_count) = self
            .state
            .prepare_retain(VertexKind::AnonymousLemma, budget)?;
        self.state.record(Undo::Vertex {
            id,
            key: Some(Arc::clone(&key).into()),
            policy: None,
        })?;
        let proof = Arc::clone(&key);
        append(
            self,
            id,
            candidate,
            key,
            dependencies,
            bytes,
            slots,
            intern_position,
            index_position,
            count_position,
            next_count,
            source,
        );
        Ok((self.handle(id), proof))
    }
}

/// Transfer the preflighted payload and dependencies with no fallible suffix.
#[expect(
    clippy::too_many_arguments,
    reason = "the preflight plan supplies twelve fixed publication values"
)]
fn append(
    cas: &mut Cas,
    id: LogicalId,
    candidate: Candidate,
    key: Arc<[u8]>,
    dependencies: Vec<NodeIndex<u32>>,
    bytes: u64,
    slots: u32,
    intern_position: usize,
    index_position: usize,
    count_position: usize,
    next_count: u64,
    source: NodeIndex<u32>,
) {
    let citation = EdgeIndex::new(cas.state.graph.edge_count() + dependencies.len());
    let index = cas
        .state
        .graph
        .add_node(Vertex::AnonymousLemma(Box::new(AnonymousLemma::LcRoot(
            LcRootLemma {
                key: Arc::clone(&key),
                candidate: Box::new(candidate),
                citation,
                bytes,
            },
        ))));
    for (slot, target) in (0..slots).zip(dependencies) {
        cas.state.graph.add_edge(
            index,
            target,
            if slot == 0 {
                Edge::InContext
            } else {
                Edge::Input(slot)
            },
        );
    }
    cas.state
        .graph
        .add_edge(source, index, Edge::CheckedEvidence);
    cas.state.indices.insert_prepared(index_position, id, index);
    cas.state
        .intern
        .insert_prepared_key(intern_position, key.into(), id, &cas.token);
    cas.state
        .retain_prepared(VertexKind::AnonymousLemma, count_position, next_count);
    cas.state.payload = cas.state.payload.saturating_add(bytes);
    cas.state.judgment_bytes = cas.state.judgment_bytes.saturating_add(bytes);
    cas.state.judgment_count += 1;
}

/// Check permanent and temporary store bounds before reserving a durable identity.
fn preflight(
    cas: &Cas,
    count: usize,
    bytes: u64,
    additional_output: u64,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let nodes = size(cas.state.graph.node_count(), AT)?.saturating_add(1);
    let edges = size(cas.state.graph.edge_count(), AT)?
        .saturating_add(count as u64)
        .saturating_add(1);
    budget.check(
        BudgetKind::OutputBytes,
        cas.state
            .judgment_bytes
            .saturating_add(bytes)
            .saturating_add(additional_output),
        AT,
    )?;
    let unregistered =
        cas.judgment_reservation(cas.state.judgment_bytes.saturating_add(bytes), budget, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, unregistered, AT)?;
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
    Ok(())
}

/// Wrong kind or missing graph evidence cannot be interpreted as a family proof.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
