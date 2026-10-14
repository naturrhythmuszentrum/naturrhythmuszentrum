// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic graph snapshots for diagnostics and construction-law tests.

use std::collections::BTreeMap;

use petgraph::visit::EdgeRef;

use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, Location};
use crate::graph::{Cas, Edge, encode_id, size, vertex_key};

/// Encode a stable role/position pair instead of a graph edge index.
const fn role(edge: Edge) -> (u8, u32) {
    match edge {
        Edge::Operand(position) => (0, position),
        Edge::Denotes => (1, 0),
        Edge::Source => (2, 0),
        Edge::Child(position) => (3, position),
        Edge::InContext => (4, 0),
        Edge::Assumption(position) => (5, position),
        Edge::PredicateArgument(position) => (6, position),
        Edge::Binder => (7, 0),
        Edge::Body(position) => (8, position),
        Edge::Provenance => (9, 0),
        Edge::Input(position) => (10, position),
        Edge::ConclusionLeft => (11, 0),
        Edge::ConclusionRight => (12, 0),
        Edge::Premise(position) => (13, position),
        Edge::Obligation(position) => (14, position),
        Edge::Justifies => (15, 0),
        Edge::DependsOn => (16, 0),
        Edge::CostPolicy => (17, 0),
    }
}

impl Cas {
    /// Export all logical records and sorted typed edges in a deterministic layout.
    ///
    /// This construction snapshot is not a mathematical witness or an import format.
    /// It omits runtime ownership tokens and every petgraph node/edge index.
    ///
    /// # Errors
    /// Refuses work, output, allocation or active occupancy limits before export completes.
    pub fn canonical_snapshot(&self, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        let at = Location::new("graph snapshot");
        budget.check(
            BudgetKind::Vertices,
            size(self.state.graph.node_count(), at)?,
            at,
        )?;
        budget.check(
            BudgetKind::Edges,
            size(self.state.graph.edge_count(), at)?,
            at,
        )?;
        let count = size(self.state.graph.edge_count(), at)?;
        budget.work(
            count
                .checked_mul(count)
                .and_then(|n| n.checked_mul(8))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
            at,
        )?;
        let mut physical = BTreeMap::new();
        for (id, index) in &self.state.indices {
            physical.insert(*index, *id);
        }
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(self.state.graph.edge_count())
            .map_err(|_| Fault::allocation(at))?;
        for edge in self.state.graph.edge_references() {
            let source = physical
                .get(&edge.source())
                .copied()
                .ok_or_else(|| Fault::capacity(BudgetKind::Edges, at))?;
            let target = physical
                .get(&edge.target())
                .copied()
                .ok_or_else(|| Fault::capacity(BudgetKind::Edges, at))?;
            edges.push((source, role(*edge.weight()), target));
        }
        edges.sort_unstable();
        let mut records = Vec::new();
        records
            .try_reserve_exact(self.state.indices.len())
            .map_err(|_| Fault::allocation(at))?;
        let mut total = 0_u64;
        for id in self.state.indices.keys() {
            let (_, key) = vertex_key(self.state.node(*id)?, &self.state, budget)?;
            total = total
                .checked_add(size(key.len(), at)?)
                .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
            budget.check(BudgetKind::OutputBytes, total, at)?;
            records.push((*id, key));
        }
        let mut out = Encoder::output(budget, at);
        out.raw(b"NRZGRAPH01")?;
        out.count(records.len())?;
        for (id, key) in records {
            encode_id(id, &mut out)?;
            out.count(key.len())?;
            out.raw(&key)?;
        }
        out.count(edges.len())?;
        for (source, (tag, position), target) in edges {
            encode_id(source, &mut out)?;
            out.byte(tag)?;
            out.unsigned(u64::from(position))?;
            encode_id(target, &mut out)?;
        }
        Ok(out.finish())
    }
}
