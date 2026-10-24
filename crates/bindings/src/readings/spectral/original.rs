// SPDX-License-Identifier: AGPL-3.0-only
//! Authenticate the original determinant, complete domain and finite roots without normalizing it.

mod domain;
mod syntax;

use super::{AT, PositiveReciprocalMap, RootCoverage, SpectralLawError as Error};
use nrz_cas::{
    Cas, Handle,
    limits::Budget,
    solving::{FamilyArgumentRole, SolutionBranchKind, SolveProblemData, SolveView},
    syntax::{Field, Operator, PredicateKind},
};

/// The original polynomial and its whole positive stratum are checked before interpreting roots.
pub(super) fn check(
    cas: &Cas,
    coverage: &RootCoverage,
    parameters: &[Handle; 2],
    operator: &PositiveReciprocalMap,
    budget: &mut Budget,
) -> Result<[Handle; 2], Error> {
    domain::target(cas, operator, budget)?;
    budget.work(1, AT)?;
    let SolveView::SolveProblem(problem) = cas.solve_view(coverage.original().problem())? else {
        return Err(Error::Structure);
    };
    if problem.ambient != Field::Complex
        || !problem.scope.is_empty()
        || problem.region != cas.admissible_region(&coverage.original().request().context)?
    {
        return Err(Error::Domain);
    }
    let family = domain::check(cas, problem, parameters, budget)?;
    determinant(cas, problem, &family, parameters, budget)?;
    roots(cas, coverage, problem, budget)
}

/// Exactly `(s-0)*(s-0)-(1/C)*(-1/L)=0` retains all written original operands.
fn determinant(
    cas: &Cas,
    problem: &SolveProblemData<Handle>,
    family: &Handle,
    inputs: &[Handle],
    budget: &mut Budget,
) -> Result<(), Error> {
    let sides = syntax::condition(cas, &problem.predicate, &PredicateKind::Equal, budget)?;
    let [left, right] = sides.as_slice() else {
        return Err(Error::Structure);
    };
    syntax::literal(cas, right, Field::Complex, false, budget)?;
    let pair = syntax::local(
        cas,
        left,
        (family, &problem.region),
        &Operator::Sub,
        [FamilyArgumentRole::Family, FamilyArgumentRole::Base],
        budget,
    )?;
    let diagonals = syntax::local(
        cas,
        &pair[0],
        (family, &problem.region),
        &Operator::Mul,
        [FamilyArgumentRole::Family; 2],
        budget,
    )?;
    for diagonal in &diagonals {
        let values = syntax::local(
            cas,
            diagonal,
            (family, &problem.region),
            &Operator::Sub,
            [FamilyArgumentRole::Fibre, FamilyArgumentRole::Base],
            budget,
        )?;
        domain::unknown(cas, &problem.unknowns, &values[0], budget)?;
        syntax::literal(cas, &values[1], Field::Real, false, budget).map_err(
            |error| match error {
                Error::Structure => Error::NonzeroDiagonal,
                other => other,
            },
        )?;
    }
    let cross = syntax::operands(cas, &pair[1], Field::Real, &Operator::Mul, 2, budget)?;
    syntax::reciprocal(cas, &cross[0], &inputs[1], budget)?;
    let lower = syntax::operands(cas, &cross[1], Field::Real, &Operator::Neg, 1, budget)?;
    syntax::reciprocal(cas, &lower[0], &inputs[0], budget)
}

/// The replayed factor-product theorem covers this exact pair over the full original domain.
fn roots(
    cas: &Cas,
    coverage: &RootCoverage,
    problem: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<[Handle; 2], Error> {
    budget.work(1, AT)?;
    let SolveView::SolutionSet(set) = cas.solve_view(coverage.set())? else {
        return Err(Error::Incomplete);
    };
    let [branch] = set.branches.as_slice() else {
        return Err(Error::Incomplete);
    };
    if &set.problem != coverage.original().problem() {
        return Err(Error::Incomplete);
    }
    budget.work(1, AT)?;
    let SolveView::SolutionBranch(branch) = cas.solve_view(branch)? else {
        return Err(Error::Incomplete);
    };
    if branch.problem != set.problem
        || branch.region != problem.region
        || branch.domain != problem.domain
    {
        return Err(Error::Incomplete);
    }
    let SolutionBranchKind::Finite { tuples } = &branch.kind else {
        return Err(Error::Incomplete);
    };
    let [first, second] = tuples.as_slice() else {
        return Err(Error::Incomplete);
    };
    let pair = [
        root(cas, first, &problem.unknowns, budget)?,
        root(cas, second, &problem.unknowns, budget)?,
    ];
    if pair[0] == pair[1] {
        return Err(Error::Incomplete);
    }
    Ok(pair)
}

/// Root values remain native CAS selections; no numerical sign is assigned to a selection slot.
fn root(
    cas: &Cas,
    tuple: &Handle,
    unknowns: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Error> {
    budget.work(1, AT)?;
    let SolveView::Tuple(data) = cas.solve_view(tuple)? else {
        return Err(Error::Incomplete);
    };
    let [value] = data.entries.as_slice() else {
        return Err(Error::Incomplete);
    };
    if &data.coordinates != unknowns || !data.scope.is_empty() {
        return Err(Error::Incomplete);
    }
    Ok(value.clone())
}
