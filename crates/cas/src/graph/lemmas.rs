// SPDX-License-Identifier: AGPL-3.0-only
//! Checked anonymous judgments use the graph's canonical index and evidence edges.

mod enclosure;
mod lc_root;
mod prepared_expm1;
pub(crate) use enclosure::EnclosureInventory;
pub(crate) use prepared_expm1::Expm1Inventory;
mod pi;
pub(crate) use pi::{PiIndex, PiInventory};
mod scalar;
mod series;
pub(crate) use scalar::{PendingJudgment, ScalarInventory};

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
    /// Complete finite formal-series application under its exact original context.
    Series(Box<series::SeriesLemma>),
    /// Original closed finite-real preparation and its normalized value.
    Constant(ConstantLemma),
    /// Complete checked scalar record inventory under the original scope.
    Scalar(scalar::ScalarLemma),
    /// Complete finite Machin argument under the registered constant theorem.
    Pi(pi::PiLemma),
    /// Independently checked original-scope rational enclosure.
    Enclosure(enclosure::EnclosureLemma),
    /// Checked native schedule under its original claim and arithmetic environment.
    PreparedExpm1(prepared_expm1::Expm1Lemma),
    /// Complete conditional LC positive-root family and original dependency inventory.
    LcRoot(lc_root::LcRootLemma),
}

impl AnonymousLemma {
    /// Canonical payload key length, independent of the disjoint graph kind discriminator.
    #[cfg(test)]
    pub(crate) fn key_bytes(&self) -> usize {
        match self {
            Self::Series(node) => node.key.len(),
            Self::Constant(node) => node.key.len(),
            Self::Enclosure(node) => node.inventory.key.len(),
            Self::PreparedExpm1(node) => node.inventory.key.len(),
            Self::LcRoot(node) => node.key.len(),
            Self::Scalar(node) => node.inventory.key.len(),
            Self::Pi(node) => node
                .inventory
                .certificate
                .canonical_key()
                .expect("validated key")
                .count(),
        }
    }

    /// Complete graph payload is accounted by the independently checked judgment kind.
    pub(crate) fn payload(&self) -> Result<u64, Fault> {
        match self {
            Self::Series(node) => Ok(node.bytes),
            Self::Constant(node) => crate::exact_constant::preparation_bytes(node.constant.value()),
            Self::Scalar(node) => Ok(node.inventory.bytes),
            Self::Pi(node) => Ok(node.inventory.bytes),
            Self::Enclosure(node) => Ok(node.inventory.bytes),
            Self::PreparedExpm1(node) => Ok(node.inventory.bytes),
            Self::LcRoot(node) => Ok(node.bytes),
        }
    }

    /// Graph snapshots retain the disjoint formal schema tag and complete canonical statement.
    pub(crate) fn encode(&self, out: &mut crate::encoding::Encoder<'_>) -> Result<(), Fault> {
        match self {
            Self::Series(node) => out.raw(&node.key),
            Self::Constant(node) => out.raw(&node.key[1..]),
            Self::Enclosure(node) => out.raw(&node.inventory.key[1..]),
            Self::PreparedExpm1(node) => out.raw(&node.inventory.key[1..]),
            Self::LcRoot(node) => out.raw(&node.key[1..]),
            Self::Scalar(node) => {
                out.byte(1)?;
                out.unsigned(u64::from(crate::proof::RETAINED_SCHEMAS[1].version))?;
                out.raw(&node.inventory.key)
            }
            Self::Pi(node) => {
                out.byte(2)?;
                out.unsigned(u64::from(crate::proof::CONSTANT_SCHEMAS[0].version))?;
                for byte in node.inventory.certificate.canonical_key()? {
                    out.byte(byte)?;
                }
                Ok(())
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
    /// Reserve this graph once beside the ledger's other live owners for a callback.
    ///
    /// An existing registration supplies this graph's judgment payload; otherwise
    /// that payload stays in the scoped reservation. Other owners and caller
    /// reservations remain additive. No caller can replace the graph's byte count.
    /// Limits are restored on successful and error returns, without refunding Work.
    /// # Errors
    /// Preserves typed registration, Work and payload failures before the callback,
    /// and the callback's own error through `From<Fault>`.
    pub fn with_retained_graph<T, E>(
        &self,
        budget: &mut Budget,
        operation: impl FnOnce(&mut Budget) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<Fault>,
    {
        let at = Location::new("retained graph");
        let judgments = self.retained_judgment_bytes();
        let unregistered = self.judgment_reservation(judgments, budget, at)?;
        let bytes = self
            .counts()
            .payload_bytes
            .saturating_sub(judgments)
            .saturating_add(unregistered);
        budget.with_retained_payload(bytes, at, operation)
    }

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

    /// Reuse a present caller registration, otherwise preserve the standalone reservation.
    pub(crate) fn judgment_reservation(
        &self,
        bytes: u64,
        budget: &mut Budget,
        at: Location,
    ) -> Result<u64, Fault> {
        let pending = self
            .receipt_records
            .lock()
            .map_err(|_| Fault::invalid(InvalidCode::Reference, at))?
            .pending_bytes();
        if budget.refresh_judgment_reservation(
            &self.memo_owner,
            bytes.saturating_add(pending),
            at,
        )? {
            Ok(0)
        } else {
            Ok(bytes)
        }
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
                self.constant_preparation(id)?;
                retained(self, self.state.judgment_bytes, budget)?;
                return Ok(id);
            }
        }
        let reserved = self.judgment_reservation(self.state.judgment_bytes, budget, AT)?;
        let value = budget.with_retained_payload(reserved, AT, |budget| {
            ExactConstant::from_expression(self, original, budget)
        })?;
        if let Some(id) = existing {
            self.constant_preparation(id)?;
            retained(self, self.state.judgment_bytes, budget)?;
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
        self.with_state(|cas, state| {
            crate::decision_kernel::cache::atomic(cas, budget, |budget| {
                publish(cas, state, prepared, key, bytes, budget)
            })
        })
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
fn retained(cas: &Cas, bytes: u64, budget: &mut Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    let unregistered = cas.judgment_reservation(bytes, budget, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, unregistered, AT)
}

/// Preflight every fallible bound before the graph takes ownership of a checked judgment.
fn publish(
    cas: &Cas,
    state: &mut State,
    constant: PreparedConstant,
    key: Vec<u8>,
    bytes: u64,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let total = state.judgment_bytes.saturating_add(bytes);
    retained(cas, total, budget)?;
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
    let serial = cas
        .token
        .identities
        .reserve(VertexKind::AnonymousLemma, 1, AT)?;
    let id = LogicalId {
        kind: VertexKind::AnonymousLemma,
        serial,
    };
    state.record(Undo::Vertex {
        id,
        key: Some(key.clone().into()),
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
    state.retain(VertexKind::AnonymousLemma);
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

#[cfg(test)]
mod tests;
