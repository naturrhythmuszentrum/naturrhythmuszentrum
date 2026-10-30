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
        Edge::CheckedEvidence => (100, 0),
        Edge::Scalar(role) => role.tag(),
        Edge::Coordinates => (42, 0),
        Edge::CoefficientCoordinates => (43, 0),
        Edge::UnknownCoordinates => (44, 0),
        Edge::FamilyCoordinates => (45, 0),
        Edge::TupleEntry(position) => (46, position),
        Edge::FamilyBody => (47, 0),
        Edge::Problem => (48, 0),
        Edge::BranchMember(position) => (49, position),
        Edge::ParameterRegion => (50, 0),
        Edge::SolutionDomain => (51, 0),
        Edge::CoefficientBase => (52, 0),
        Edge::DefiningPolynomial => (53, 0),
        Edge::TowerParent => (54, 0),
        Edge::TowerLevel => (55, 0),
        Edge::CoefficientTerm(position) => (56, position),
        Edge::MapSource => (57, 0),
        Edge::MapTarget => (58, 0),
        Edge::GeneratorImage(position) => (59, position),
        Edge::RootSlot(position) => (60, position),
        Edge::SelectionPredicate => (61, 0),
        Edge::SelectedBy => (62, 0),
        Edge::CoefficientLiteralValue => (63, 0),
        Edge::CoefficientLiteralMap => (64, 0),
        Edge::SplitParent => (65, 0),
        Edge::SplitChild(position) => (66, position),
        Edge::SplitMap(position) => (67, position),
        Edge::OriginalPredicate => (68, 0),
        Edge::OriginalDenominator(position) => (69, position),
        Edge::LocalBase(position) => (70, position),
        Edge::LocalJacobian => (71, 0),
        Edge::LocalInverse => (72, 0),
        Edge::SplitFactor(position) => (73, position),
        Edge::DenominatorFactor(position) => (74, position),
        Edge::TupleMember(position) => (75, position),
        Edge::FamilyImage => (76, 0),
        Edge::LocalGerm => (77, 0),
        Edge::FractionNumerator => (78, 0),
        Edge::FractionDenominator => (79, 0),

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
        Edge::InHeader => (18, 0),
        Edge::ParentHeader => (19, 0),
        Edge::DeclarationSpace(position) => (20, position),
        Edge::SpaceDomain => (21, 0),
        Edge::RegionInput(position) => (22, position),
        Edge::MembershipPredicate => (23, 0),
        Edge::ArmRegion => (24, 0),
        Edge::ArmValue => (25, 0),
        Edge::BoundaryLocation => (26, 0),
        Edge::SourceArgument(position) => (27, position),
        Edge::CaseDomain => (28, 0),
        Edge::CaseMember(position) => (29, position),
        Edge::InSpace => (30, 0),
        Edge::AdmissibleRegion => (31, 0),
        Edge::ParentContext => (32, 0),
        Edge::ConclusionObject(position) => (33, position),
        Edge::IncidentRegion => (34, 0),
        Edge::AtBoundary => (35, 0),
        Edge::OfFunction => (36, 0),
        Edge::SideRegion(position) => (37, position.tag()),
        Edge::JetValue(position) => (38, position),
        Edge::ComparedRegion(position) => (39, position),
        Edge::CoverMember(position) => (40, position),
        Edge::ArgumentRegion(position) => (41, position),
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
        out.raw(b"NRZGRAPH03")?;
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
