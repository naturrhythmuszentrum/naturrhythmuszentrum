// SPDX-License-Identifier: AGPL-3.0-only
//! Finite series judgments have one graph owner and an authenticated evidence edge.

use super::{AnonymousLemma, Cas, Edge, EdgeIndex, LogicalId, Vertex};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Handle, size, storage_check};
use crate::proof_model::CheckedApplication;
use std::sync::Arc;

/// Attribution for graph-owned finite judgments.
const AT: Location = Location::new("checked formal series judgment");

/// Complete checked payload; the index retains only its full key and logical identifier.
#[derive(Clone, Debug)]
pub(crate) struct SeriesLemma {
    /// Disjoint kind four, schema one, and the complete checked application.
    pub(super) key: Vec<u8>,
    /// Exact retained payload and dependency accounting.
    pub(super) bytes: u64,
    /// The same application consumed by ordinary citation and witness APIs.
    application: CheckedApplication,
    /// Store ownership is never supplied by portable bytes.
    owner: Arc<()>,
    /// Publication generation; surviving immutable proofs remain valid after rollback.
    generation: u64,
    /// Incoming original-object checked-evidence edge.
    citation: Option<EdgeIndex<u32>>,
}

impl Cas {
    /// Borrow only graph-owned, current checked evidence with its actual incoming edge.
    pub(crate) fn series_application(
        &self,
        id: LogicalId,
        budget: &mut Budget,
    ) -> Result<&CheckedApplication, Fault> {
        let Vertex::AnonymousLemma(node) = self.state.node(id, budget)? else {
            return Err(invalid());
        };
        let AnonymousLemma::Series(node) = node.as_ref() else {
            return Err(invalid());
        };
        let crate::proof_model::Statement::Series(statement) = &node.application.judgment.statement
        else {
            return Err(invalid());
        };
        let source = self
            .state
            .index(self.checked(statement.input(), budget)?, budget)?;
        let target = self.state.index(id, budget)?;
        let citation = node.citation.ok_or_else(invalid)?;
        if !Arc::ptr_eq(&node.owner, &self.memo_owner)
            || node.generation > self.memo_epoch
            || self.state.graph.edge_endpoints(citation) != Some((source, target))
            || !matches!(
                self.state.graph.edge_weight(citation),
                Some(Edge::CheckedEvidence)
            )
        {
            return Err(invalid());
        }
        Ok(&node.application)
    }

    /// Publish only after independent checking, with one journalled payload and no parallel receipt.
    pub(crate) fn publish_series(
        &mut self,
        application: CheckedApplication,
        edges: &[(LogicalId, Edge)],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let crate::proof_model::Statement::Series(statement) = &application.judgment.statement
        else {
            return Err(invalid());
        };
        let original = self.checked(statement.input(), budget)?;
        let mut out = crate::encoding::Encoder::new(budget, AT);
        out.byte(4)?;
        out.unsigned(1)?;
        crate::statement::application(&application, &mut out)?;
        let key = out.finish();
        let bytes = size(key.len(), AT)?
            .checked_mul(3)
            .and_then(|n| n.checked_add(256))
            .and_then(|n| n.checked_add(size(edges.len() + 1, AT).ok()?.checked_mul(32)?))
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, AT))?;
        budget.work(bytes, AT)?;
        let total = self
            .state
            .judgment_bytes
            .checked_add(bytes)
            .ok_or_else(invalid)?;
        super::retained(self, total, budget)?;
        self.with_state(|cas, state| {
            let count = size(state.graph.edge_count(), AT)?
                .checked_add(size(edges.len(), AT)?)
                .and_then(|n| n.checked_add(2))
                .ok_or_else(invalid)?;
            for kind in [BudgetKind::Edges, BudgetKind::StagedEdges] {
                storage_check(&cas.limits, budget, kind, count, AT)?;
            }
            if count >= u64::from(u32::MAX) {
                return Err(Fault::capacity(BudgetKind::Edges, AT));
            }
            let total = state
                .judgment_bytes
                .checked_add(bytes)
                .ok_or_else(invalid)?;
            budget.check(BudgetKind::OutputBytes, total, AT)?;
            let source = state.index(original, budget)?;
            let before = state.graph.node_count();
            let id = cas.insert(
                state,
                Vertex::AnonymousLemma(Box::new(AnonymousLemma::Series(Box::new(SeriesLemma {
                    key,
                    bytes,
                    application,
                    owner: Arc::clone(&cas.memo_owner),
                    generation: cas.memo_epoch,
                    citation: None,
                })))),
                edges,
                bytes,
                budget,
            )?;
            if state.graph.node_count() > before {
                // An appended node occupies the old tail; source lookup was prepaid above.
                let index = crate::graph::NodeIndex::new(before);
                let citation = state.graph.add_edge(source, index, Edge::CheckedEvidence);
                let Vertex::AnonymousLemma(node) = &mut state.graph[index] else {
                    return Err(invalid());
                };
                let AnonymousLemma::Series(node) = node.as_mut() else {
                    return Err(invalid());
                };
                node.citation = Some(citation);
                state.judgment_bytes = total;
                state.judgment_count += 1;
            }
            let proof = cas.insert(
                state,
                Vertex::Proof(id),
                &[(id, Edge::Justifies)],
                0,
                budget,
            )?;
            let proof_count = state.proof_count(budget)?;
            budget.check(BudgetKind::ProofNodes, proof_count, AT)?;
            Ok(cas.handle(proof))
        })
    }
}

/// A missing or foreign evidence edge is a reference fault, never a capability refusal.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
