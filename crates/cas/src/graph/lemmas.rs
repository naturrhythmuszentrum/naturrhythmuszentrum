// SPDX-License-Identifier: AGPL-3.0-only
//! Checked anonymous judgments use the graph's canonical index and evidence edges.

mod scalar;
pub(crate) use scalar::{PendingScalar, ScalarInventory};

use super::{
    Cas, Edge, EdgeIndex, LogicalId, State, Undo, Vertex, VertexKind, size, storage_check,
};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::exact_constant::{ExactConstant, PreparedConstant};
use crate::graph::Handle;

/// Preserve the existing constant admission and citation accounting boundary.
const AT: Location = Location::new("exact constant cache");

/// The graph owns each checked payload; kind tags separate otherwise equal statement bytes.
#[derive(Clone, Debug)]
pub(crate) enum AnonymousLemma {
    /// Original closed finite-real preparation and its normalized value.
    Constant(ConstantLemma),
    /// Complete checked scalar record inventory under the original scope.
    Scalar(scalar::ScalarLemma),
}

impl AnonymousLemma {
    /// Canonical payload key length, independent of the disjoint graph kind discriminator.
    #[cfg(test)]
    pub(crate) fn key_bytes(&self) -> usize {
        match self {
            Self::Constant(node) => node.key.len(),
            Self::Scalar(node) => node.inventory.key.len(),
        }
    }

    /// Complete graph payload is accounted by the independently checked judgment kind.
    pub(crate) fn payload(&self) -> Result<u64, Fault> {
        match self {
            Self::Constant(node) => crate::exact_constant::preparation_bytes(node.constant.value()),
            Self::Scalar(node) => Ok(node.inventory.bytes),
        }
    }

    /// Graph snapshots retain the disjoint formal schema tag and complete canonical statement.
    pub(crate) fn encode(&self, out: &mut crate::encoding::Encoder<'_>) -> Result<(), Fault> {
        match self {
            Self::Constant(node) => out.raw(&node.key[1..]),
            Self::Scalar(node) => {
                out.byte(1)?;
                out.unsigned(u64::from(crate::proof::RETAINED_SCHEMAS[1].version))?;
                out.raw(&node.inventory.key)
            }
        }
    }
}

/// One graph-owned checked statement, never a portable proof or source assumption.
#[derive(Debug)]
pub(crate) struct ConstantLemma {
    /// Complete kind-qualified statement under this graph's canonical reference namespace.
    pub(crate) key: Vec<u8>,
    /// Strong runtime ownership prevents a copied store from inheriting checked authority.
    owner: std::sync::Arc<()>,
    /// Last independently checked rollback namespace; excluded from mathematical bytes.
    epoch: u64,
    /// Source-to-lemma edge authenticated before borrowing the checked payload.
    citation: EdgeIndex<u32>,
    /// Original finite-real judgment and its independently reconstructed normalized value.
    pub(crate) constant: PreparedConstant,
}

impl Clone for ConstantLemma {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            owner: std::sync::Arc::clone(&self.owner),
            epoch: self.epoch,
            citation: self.citation,
            constant: PreparedConstant::checked(
                self.constant.original().clone(),
                self.constant.context().clone(),
                self.constant.value().clone(),
            ),
        }
    }
}

impl Cas {
    /// Number of retained anonymous judgment nodes, including older namespace evidence.
    #[must_use]
    pub const fn retained_judgment_count(&self) -> usize {
        self.state.judgment_count
    }

    /// One graph-owned payload account for all anonymous retained judgments.
    #[must_use]
    pub const fn retained_judgment_bytes(&self) -> u64 {
        self.state.judgment_bytes
    }

    /// Authenticate original scope before probing the graph's complete canonical index.
    pub(crate) fn prepare_constant_node(
        &mut self,
        context: &Handle,
        original: &Handle,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let expression = self.checked(original)?;
        let frame = self.checked(context)?;
        if self.state.expression(expression)?.header != self.state.context(frame)?.header_id {
            return Err(invalid(InvalidCode::Scope));
        }
        let key = constant_key(frame, expression);
        let existing = self.state.intern.get(&key).copied();
        if let Some(id) = existing {
            let node = self.lemma(id)?;
            let same_key = node.key == key;
            let same_original = node.constant.original() == original;
            let same_context = node.constant.context() == context;
            if !(same_key && same_original && same_context) {
                return Err(invalid(InvalidCode::Reference));
            }
            let current = node.epoch == self.memo_epoch;
            let reusable = self.memo_epoch != u64::MAX;
            let owned = std::sync::Arc::ptr_eq(&node.owner, &self.memo_owner);
            if current && reusable && owned {
                budget.work(
                    u64::from(size(self.retained_judgment_count(), AT)?.bit_width().max(1)),
                    AT,
                )?;
                retained(self.state.judgment_bytes, budget)?;
                self.constant_preparation(id)?;
                return Ok(id);
            }
        }
        let value = budget.with_retained_payload(self.state.judgment_bytes, AT, |budget| {
            ExactConstant::from_expression(self, original, budget)
        })?;
        if let Some(id) = existing {
            retained(self.state.judgment_bytes, budget)?;
            self.constant_preparation(id)?;
            let index = self.state.indices[&id];
            let Vertex::AnonymousLemma(node) = &mut self.state.graph[index] else {
                return Err(invalid(InvalidCode::Reference));
            };
            let AnonymousLemma::Constant(node) = node.as_mut() else {
                return Err(invalid(InvalidCode::Reference));
            };
            node.epoch = self.memo_epoch;
            node.owner = std::sync::Arc::clone(&self.memo_owner);
            return Ok(id);
        }
        let bytes = crate::exact_constant::preparation_bytes(&value)?;
        let prepared = PreparedConstant::checked(original.clone(), context.clone(), value);
        self.with_state(|cas, state| publish(cas, state, prepared, key, bytes, budget))
    }

    /// Borrow the actual graph payload only after authenticating its checked evidence edge.
    pub(crate) fn constant_preparation(&self, id: LogicalId) -> Result<&PreparedConstant, Fault> {
        let node = self.lemma(id)?;
        let source = self.state.indices[&self.checked(node.constant.original())?];
        let target = self.state.indices[&id];
        let endpoints = self.state.graph.edge_endpoints(node.citation) == Some((source, target));
        let checked = matches!(
            self.state.graph.edge_weight(node.citation),
            Some(Edge::CheckedEvidence)
        );
        if !(endpoints && checked) {
            return Err(invalid(InvalidCode::Reference));
        }
        Ok(&node.constant)
    }

    /// A structural index entry can refer only to the expected checked vertex kind.
    fn lemma(&self, id: LogicalId) -> Result<&ConstantLemma, Fault> {
        let Vertex::AnonymousLemma(node) = self.state.node(id)? else {
            return Err(invalid(InvalidCode::Reference));
        };
        let AnonymousLemma::Constant(node) = node.as_ref() else {
            return Err(invalid(InvalidCode::Reference));
        };
        Ok(node)
    }
}

/// Fixed-width complete source references avoid rebuilding their immutable dependency closures.
fn constant_key(context: LogicalId, original: LogicalId) -> Vec<u8> {
    let schema = &crate::proof::RETAINED_SCHEMAS[0];
    let mut key = Vec::with_capacity(24);
    key.push(VertexKind::AnonymousLemma.tag());
    key.push(0); // Stable schema tag for constant.closed-preparation, never a display name.
    key.extend_from_slice(&schema.version.to_le_bytes());
    for id in [context, original] {
        key.push(id.kind.tag());
        key.extend_from_slice(&id.serial.to_le_bytes());
    }
    key
}

/// Retained node payload is charged once beside the caller's unrelated live bytes.
fn retained(bytes: u64, budget: &Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)
}

/// Preflight every fallible bound before the graph takes ownership of a checked judgment.
fn publish(
    cas: &Cas,
    state: &mut State,
    constant: PreparedConstant,
    key: Vec<u8>,
    bytes: u64,
    budget: &Budget,
) -> Result<LogicalId, Fault> {
    let total = state.judgment_bytes.saturating_add(bytes);
    retained(total, budget)?;
    let payload = state.payload.saturating_add(bytes);
    storage_check(&cas.limits, budget, BudgetKind::PayloadBytes, payload, AT)?;
    let nodes = size(state.graph.node_count(), AT)?.saturating_add(1);
    let edges = size(state.graph.edge_count(), AT)?.saturating_add(3);
    for (kind, count) in [
        (BudgetKind::Vertices, nodes),
        (BudgetKind::StagedVertices, nodes),
        (BudgetKind::Edges, edges),
        (BudgetKind::StagedEdges, edges),
    ] {
        storage_check(&cas.limits, budget, kind, count, AT)?;
    }
    if nodes >= u64::from(u32::MAX) || edges >= u64::from(u32::MAX) {
        return Err(Fault::capacity(BudgetKind::Vertices, AT));
    }
    let original = state.indices[&cas.checked_in(state, constant.original())?];
    let context = state.indices[&cas.checked_in(state, constant.context())?];
    let serial = state
        .next
        .get(&VertexKind::AnonymousLemma)
        .copied()
        .unwrap_or(0);
    let id = LogicalId {
        kind: VertexKind::AnonymousLemma,
        serial,
    };
    state.record(Undo::Vertex {
        id,
        key: Some(key.clone()),
        policy: None,
    })?;
    let citation = EdgeIndex::new(state.graph.edge_count() + 2);
    let index = state
        .graph
        .add_node(Vertex::AnonymousLemma(Box::new(AnonymousLemma::Constant(
            ConstantLemma {
                key: key.clone(),
                owner: std::sync::Arc::clone(&cas.memo_owner),
                epoch: cas.memo_epoch,
                citation,
                constant,
            },
        ))));
    state.graph.add_edge(index, original, Edge::Input(0));
    state.graph.add_edge(index, context, Edge::InContext);
    state.graph.add_edge(original, index, Edge::CheckedEvidence);
    state.indices.insert(id, index);
    state.next.insert(VertexKind::AnonymousLemma, serial + 1);
    state.intern.insert(key, id, &cas.token);
    state.payload = payload;
    state.judgment_bytes = total;
    state.judgment_count += 1;
    Ok(id)
}

/// A failed namespace or evidence-edge authentication never manufactures a checked result.
const fn invalid(code: InvalidCode) -> Fault {
    Fault::invalid(code, AT)
}
