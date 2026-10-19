// SPDX-License-Identifier: AGPL-3.0-only
//! Complete semantic routing before any coefficient or search operation.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, Expression, Handle, LogicalId, Vertex, size, vertex_key};
use crate::model::{Field, Operator, Sort};
use crate::solving::SolveReason;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

/// Stable location of structural routing refusals.
const AT: Location = Location::new("declared scalar sort preflight");

/// The exact structural producer partition, without theorem authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarRoute {
    /// Real unknowns and no original Complex dependency.
    Real,
    /// Complex unknowns with an empty original Real base.
    Complex,
    /// Complex unknowns with original Real base dependencies.
    Family,
    /// Another admitted sort continues to its own capability handling.
    Other,
}

/// Immutable descriptive metadata, independently reconstructed on every request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SortRouting {
    /// Selected structural route.
    pub route: ScalarRoute,
    /// Original used Real declarations, qualified by their introducing header.
    pub real: Vec<(Handle, String)>,
    /// Original used Complex declarations, qualified by their introducing header.
    pub complex: Vec<(Handle, String)>,
    /// A named producer boundary; never a mathematical negative theorem.
    pub refusal: Option<SolveReason>,
}

/// One reached header's distinct original declaration uses.
type Uses<'a> = HashMap<NodeIndex<u32>, HashMap<&'a str, &'a Sort>>;

/// Prepaid per-request scratch, discarded when routing finishes.
struct Scan<'a> {
    /// Distinct physical records already scheduled.
    seen: HashSet<NodeIndex<u32>>,
    /// Pending records and their explicit path depth.
    pending: Vec<(NodeIndex<u32>, u64)>,
    /// Distinct declaration uses in each original semantic header.
    uses: Uses<'a>,
    /// Logical identities for exactly the used headers and their ancestors.
    headers: HashMap<NodeIndex<u32>, LogicalId>,
    /// Cumulative reached semantic edges.
    edges: u64,
}

impl Cas {
    /// Inspect all original semantic dependencies without coefficient arithmetic.
    ///
    /// # Errors
    /// Refuses foreign references, inconsistent stored sorts and exhausted limits.
    pub fn sort_routing(
        &self,
        problem: &Handle,
        roots: &[Handle],
        budget: &mut Budget,
    ) -> Result<SortRouting, Fault> {
        let original = crate::solving::graph::problem(&self.state, self.checked(problem)?)?;
        let mut scan = Scan {
            seen: HashSet::new(),
            pending: Vec::new(),
            uses: HashMap::new(),
            headers: HashMap::new(),
            edges: 0,
        };
        scan.enqueue(self, problem.logical_id(), 1, budget)?;
        for root in roots {
            scan.enqueue(self, self.checked(root)?, 1, budget)?;
        }
        scan.walk(self, budget)?;
        let (real, complex) = super::routing_index::resolve(
            self,
            &scan.headers,
            &scan.uses,
            scan.seen.len(),
            budget,
        )?;
        let refusal = (original.ambient == Field::Real && !complex.is_empty())
            .then_some(SolveReason::RealProblemHasComplexSymbol);
        let route = match original.ambient {
            Field::Real => ScalarRoute::Real,
            Field::Complex if real.is_empty() => ScalarRoute::Complex,
            Field::Complex => ScalarRoute::Family,
            _ => ScalarRoute::Other,
        };
        Ok(SortRouting {
            route,
            real,
            complex,
            refusal,
        })
    }
}

impl<'a> Scan<'a> {
    /// Reserve a distinct pending record before inserting either scratch entry.
    fn enqueue(
        &mut self,
        cas: &Cas,
        id: LogicalId,
        depth: u64,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        budget.work(256, AT)?;
        let index = *cas.state.indices.get(&id).ok_or_else(invalid)?;
        self.push(index, depth, budget)
    }

    /// Charge hash lookup and finite storage before scheduling an unseen node.
    fn push(
        &mut self,
        index: NodeIndex<u32>,
        depth: u64,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        budget.work(256, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        if self.seen.contains(&index) {
            return Ok(());
        }
        let count = size(self.seen.len().saturating_add(1), AT)?;
        budget.check(BudgetKind::Vertices, count, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count.saturating_mul(1024),
            AT,
        )?;
        self.seen
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.pending
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.seen.insert(index);
        self.pending.push((index, depth));
        Ok(())
    }

    /// Every reached payload and semantic edge is inspected once, including inactive clauses.
    fn walk(&mut self, cas: &'a Cas, budget: &mut Budget) -> Result<(), Fault> {
        while let Some((index, depth)) = self.pending.pop() {
            budget.work(1024, AT)?;
            let node = cas.state.graph.node_weight(index).ok_or_else(invalid)?;
            let (_, bytes) = vertex_key(node, &cas.state, budget)?;
            budget.work(size(bytes.len(), AT)?, AT)?;
            if let Vertex::Expression(expression) = node {
                super::routing_shapes::expression(cas, expression, budget)?;
                self.expression(cas, expression, budget)?;
            }
            if let Vertex::Solve(value) = node {
                super::routing_shapes::solve(cas, value, budget)?;
            }
            for edge in cas.state.graph.edges(index) {
                if matches!(node, Vertex::Header(_)) && !matches!(edge.weight(), Edge::ParentHeader)
                {
                    continue;
                }
                self.edges = self
                    .edges
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Edges, AT))?;
                budget.check(BudgetKind::Edges, self.edges, AT)?;
                self.push(edge.target(), depth.saturating_add(1), budget)?;
            }
        }
        Ok(())
    }

    /// Compare explicit ambient identities and accumulate only actual expression dependencies.
    fn expression(
        &mut self,
        cas: &'a Cas,
        expression: &'a Expression,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if expression.ambient != super::ambient(&cas.state, expression.header, &expression.sort)? {
            return Err(Fault::invalid(InvalidCode::AmbientSpaceMismatch, AT));
        }
        match &expression.operator {
            Operator::Free(key) => {
                let index = *cas
                    .state
                    .indices
                    .get(&expression.header)
                    .ok_or_else(invalid)?;
                self.header(cas, expression.header, budget)?;
                budget.work(size(key.len(), AT)?, AT)?;
                self.uses
                    .try_reserve(1)
                    .map_err(|_| Fault::allocation(AT))?;
                let uses = self.uses.entry(index).or_default();
                uses.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                if uses
                    .insert(key, &expression.sort)
                    .is_some_and(|previous| previous != &expression.sort)
                {
                    return Err(invalid());
                }
            }
            Operator::Bound { depth, slot } => bound(expression, *depth, *slot)?,
            _ => {}
        }
        Ok(())
    }

    /// Retain each original introducing-header path only once.
    fn header(&mut self, cas: &Cas, mut id: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        loop {
            budget.work(256, AT)?;
            let index = *cas.state.indices.get(&id).ok_or_else(invalid)?;
            if self.headers.contains_key(&index) {
                return Ok(());
            }
            self.headers
                .try_reserve(1)
                .map_err(|_| Fault::allocation(AT))?;
            self.headers.insert(index, id);
            let Some(parent) = cas.state.header(id)?.parent else {
                return Ok(());
            };
            id = parent;
        }
    }
}

/// Reconstruct a de Bruijn claim from its actual retained binder frame.
fn bound(expression: &Expression, depth: u32, slot: u32) -> Result<(), Fault> {
    let offset = usize::try_from(depth).map_err(|_| invalid())?;
    let index = expression
        .scope
        .len()
        .checked_sub(offset.saturating_add(1))
        .ok_or_else(invalid)?;
    let atomic = expression
        .scope
        .get(index)
        .and_then(|frame| frame.get(usize::try_from(slot).ok()?))
        .ok_or_else(invalid)?;
    if expression.sort != Sort::Value(atomic.clone()) {
        return Err(invalid());
    }
    Ok(())
}

/// Preserve a named sort contradiction without invoking coefficient capabilities.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::SymbolSortMismatch, AT)
}

/// The service freezes descriptive routing before offering any expensive executor.
pub(crate) fn preflight(
    cas: &Cas,
    request: &crate::request_model::Request,
    budget: &mut Budget,
) -> Result<SortRouting, Fault> {
    let crate::task::Task::Solve(specification) = &request.task else {
        return Err(invalid());
    };
    let mut roots = Vec::new();
    budget.check(
        BudgetKind::StagedPayloadBytes,
        size(request.inputs.len().saturating_add(1), AT)?.saturating_mul(64),
        AT,
    )?;
    roots
        .try_reserve_exact(request.inputs.len().saturating_add(1))
        .map_err(|_| Fault::allocation(AT))?;
    roots.push(request.context.clone());
    roots.extend(request.inputs.iter().map(|input| input.root.clone()));
    cas.sort_routing(&specification.problem, &roots, budget)
}
