// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic graph retention for one checked source substitution and its root proof.

use std::sync::Arc;

use super::{AnonymousLemma, Cas, Edge, EdgeIndex, LogicalId, Undo, Vertex, VertexKind};
use crate::budget::{Budget, BudgetKind};
use crate::enclosure::lc_source_substitution::Receipt;
use crate::graph::{Handle, NodeIndex, StoredKey, size, storage_check};
use crate::{Fault, InvalidCode, Location};

/// Location reported for checked source substitution publication.
const AT: Location = Location::new("LC source substitution publication");

/// Complete checked receipt and its two graph evidence edges.
#[derive(Clone, Debug)]
pub(crate) struct LcSourceSubstitutionLemma {
    /// Canonical proof key shared with the intern index.
    pub(super) key: Arc<[u8]>,
    /// Complete checked receipt and original inventory.
    receipt: Box<Receipt>,
    /// Admission rollback namespace; earlier committed nodes remain valid in later epochs.
    epoch: u64,
    /// Checked original-target evidence edge.
    citation: EdgeIndex<u32>,
    /// Particular root proof dependency edge.
    root_edge: EdgeIndex<u32>,
    /// Charged graph payload residence.
    pub(super) bytes: u64,
}

impl Cas {
    /// Authenticate the live checked-edge and particular root dependency.
    pub(crate) fn lc_source_substitution_receipt(
        &self,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<&Receipt, Fault> {
        let id = self.checked(handle, budget)?;
        let Vertex::AnonymousLemma(node) = self.state.node(id, budget)? else {
            return Err(invalid());
        };
        let AnonymousLemma::LcSourceSubstitution(node) = node.as_ref() else {
            return Err(invalid());
        };
        if !Arc::ptr_eq(&node.receipt.owner, &self.memo_owner) {
            return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
        }
        if node.epoch > self.memo_epoch {
            return Err(invalid());
        }
        let source = self.state.index(
            self.checked(&node.receipt.request.targets[0], budget)?,
            budget,
        )?;
        let root_id = self.checked(&node.receipt.root.judgment, budget)?;
        self.lc_root_request(&node.receipt.root.judgment, budget)?;
        let root = self.state.index(root_id, budget)?;
        let target = self.state.index(id, budget)?;
        let root_slot = u32::try_from(node.receipt.request.handles().count())
            .map_err(|_| Fault::capacity(BudgetKind::Edges, AT))?;
        if self.state.graph.edge_endpoints(node.citation) != Some((source, target))
            || !matches!(
                self.state.graph.edge_weight(node.citation),
                Some(Edge::CheckedEvidence)
            )
            || self.state.graph.edge_endpoints(node.root_edge) != Some((target, root))
            || !matches!(self.state.graph.edge_weight(node.root_edge), Some(Edge::Input(slot)) if *slot == root_slot)
        {
            return Err(invalid());
        }
        Ok(&node.receipt)
    }

    /// Publish a checked receipt after all fallible graph work has been prepaid.
    #[expect(
        clippy::too_many_lines,
        reason = "the publication preflight and append share one atomic path"
    )]
    pub(crate) fn retain_lc_source_substitution(
        &mut self,
        receipt: Receipt,
        additional_output: u64,
        budget: &mut Budget,
    ) -> Result<(Handle, Arc<[u8]>), Fault> {
        let count = receipt.request.handles().count();
        let edges_count = count.saturating_add(1);
        let slots = u32::try_from(count).map_err(|_| Fault::capacity(BudgetKind::Edges, AT))?;
        let bytes = receipt
            .retained_bytes()
            .saturating_add(receipt.wire.len() as u64)
            .saturating_add(size_of::<AnonymousLemma>() as u64)
            .saturating_add(size_of::<LcSourceSubstitutionLemma>() as u64)
            .saturating_add(18);
        let key_len = receipt.wire.len().saturating_add(2);
        let key_bytes = (receipt.wire.len() as u64).saturating_add(2);
        let transfer = receipt
            .retained_bytes()
            .saturating_add(key_bytes.saturating_mul(2))
            .saturating_add(16)
            .saturating_add(size_of::<Vec<u8>>() as u64)
            .saturating_add(size_of::<Arc<[u8]>>() as u64);
        let dependency_scratch = (edges_count as u64)
            .saturating_mul(size_of::<NodeIndex<u32>>() as u64)
            .saturating_add(size_of::<Vec<NodeIndex<u32>>>() as u64);
        let append_peak = bytes.saturating_add(dependency_scratch);
        let live = self.judgment_reservation(self.state.judgment_bytes, budget, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            live.saturating_add(transfer.max(append_peak)),
            AT,
        )?;
        budget.work(bytes, AT)?;
        let mut key = Vec::new();
        key.try_reserve_exact(key_len)
            .map_err(|_| Fault::allocation(AT))?;
        key.push(VertexKind::AnonymousLemma.tag());
        key.push(7);
        key.extend_from_slice(&receipt.wire);
        let key: Arc<[u8]> = key.into();
        let floor = size(key.len(), AT)?.saturating_add(1);
        if let Some(id) = self
            .state
            .intern
            .get(key.as_ref(), floor, budget, AT)?
            .copied()
        {
            let handle = self.handle(id);
            let existing = self.lc_source_substitution_receipt(&handle, budget)?;
            if existing.request != receipt.request
                || existing.root.judgment != receipt.root.judgment
            {
                return Err(invalid());
            }
            budget.check(
                BudgetKind::OutputBytes,
                self.state.judgment_bytes.saturating_add(additional_output),
                AT,
            )?;
            let proof = self.lc_source_substitution_key(&handle, budget)?;
            return Ok((handle, proof));
        }
        preflight(self, edges_count, bytes, additional_output, budget)?;
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(edges_count)
            .map_err(|_| Fault::allocation(AT))?;
        for handle in receipt
            .request
            .handles()
            .chain(std::iter::once(&receipt.root.judgment))
        {
            budget.work(1, AT)?;
            dependencies.push(self.state.index(self.checked(handle, budget)?, budget)?);
        }
        let source = self
            .state
            .index(receipt.request.targets[0].logical_id(), budget)?;
        let stored_key: StoredKey = Arc::clone(&key).into();
        let (intern_position, _) = self.state.intern.prepare_insert_key(
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
            receipt,
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

    /// Return the canonical proof key of a live substitution judgment.
    fn lc_source_substitution_key(
        &self,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<Arc<[u8]>, Fault> {
        let id = self.checked(handle, budget)?;
        let Vertex::AnonymousLemma(node) = self.state.node(id, budget)? else {
            return Err(invalid());
        };
        let AnonymousLemma::LcSourceSubstitution(node) = node.as_ref() else {
            return Err(invalid());
        };
        Ok(Arc::clone(&node.key))
    }
}

/// Append the prepaid node, dependencies and checked evidence edge.
#[expect(
    clippy::too_many_arguments,
    reason = "the preflight supplies fixed graph publication values"
)]
fn append(
    cas: &mut Cas,
    id: LogicalId,
    receipt: Receipt,
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
    debug_assert_eq!(slots as usize + 1, dependencies.len());
    let start = cas.state.graph.edge_count();
    let root_edge = EdgeIndex::new(start + dependencies.len() - 1);
    let citation = EdgeIndex::new(start + dependencies.len());
    let index = cas.state.graph.add_node(Vertex::AnonymousLemma(Box::new(
        AnonymousLemma::LcSourceSubstitution(Box::new(LcSourceSubstitutionLemma {
            key: Arc::clone(&key),
            receipt: Box::new(receipt),
            epoch: cas.memo_epoch,
            citation,
            root_edge,
            bytes,
        })),
    )));
    for (slot, target) in (0..=slots).zip(dependencies) {
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

/// Check the complete graph and output residence before allocating publication state.
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

/// Reference fault for an unauthenticated graph proof.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
