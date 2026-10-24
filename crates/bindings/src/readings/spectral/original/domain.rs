// SPDX-License-Identifier: AGPL-3.0-only
//! Runtime guards cover every premise of the original positive family and no hidden restriction.

use super::{AT, Error, syntax};
use nrz_cas::{
    Cas, Handle,
    domain::AmbientDomain,
    limits::Budget,
    solving::{CoordinateRole, SolveProblemData, SolveView},
    source::{RegionConstructor, SyntaxNode, SyntaxRole},
    syntax::{AtomicSort, Field, Operator, PredicateKind, Sort},
};

/// Positive L/C are the entire coefficient stratum; the Complex unknown has its full ambient domain.
pub(super) fn check(
    cas: &Cas,
    problem: &SolveProblemData<Handle>,
    inputs: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Error> {
    coordinates(cas, &problem.coefficients, inputs, budget)?;
    ambient(cas, &problem.domain, &problem.unknowns, budget)?;
    let region = cas.describe_syntax(&problem.region, budget)?;
    let family = syntax::dependency(&region, SyntaxRole::Family)?.clone();
    let stratum = syntax::dependency(&region, SyntaxRole::BaseStratum)?;
    positive(cas, stratum, &problem.coefficients, inputs, budget)?;
    positive(cas, &problem.region, &problem.coefficients, inputs, budget)?;
    family_domain(cas, &family, problem, budget)?;
    Ok(family)
}

/// The original base projection is discharged by the same two numerical positive guards.
fn positive(
    cas: &Cas,
    region: &Handle,
    coordinates: &Handle,
    inputs: &[Handle],
    budget: &mut Budget,
) -> Result<(), Error> {
    let mut pending = region.clone();
    for input in inputs.iter().rev() {
        let guard = cas.describe_syntax(&pending, budget)?;
        if !matches!(
            guard.node,
            SyntaxNode::Region(_, RegionConstructor::GuardTrue)
        ) {
            return Err(Error::Domain);
        }
        let predicate = syntax::dependency(&guard, SyntaxRole::Predicate)?;
        let arguments = syntax::condition(cas, predicate, &PredicateKind::Positive, budget)?;
        if arguments != std::slice::from_ref(input) {
            return Err(Error::Domain);
        }
        pending = syntax::dependency(&guard, SyntaxRole::Base)?.clone();
    }
    ambient(cas, &pending, coordinates, budget)
}

/// The retained family's base and fibre introduce no additional unexecuted domain restriction.
fn family_domain(
    cas: &Cas,
    family: &Handle,
    problem: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.work(3, AT)?;
    let SolveView::SpaceFamily(data) = cas.solve_view(family)? else {
        return Err(Error::Domain);
    };
    if data.unknowns != problem.unknowns {
        return Err(Error::Domain);
    }
    let SolveView::CoordinateSpace(coefficients) = cas.solve_view(&data.coefficients)? else {
        return Err(Error::Domain);
    };
    if !coefficients.coordinates.is_empty() {
        return Err(Error::Domain);
    }
    let SolveView::ParameterBase(base) = cas.solve_view(&data.base)? else {
        return Err(Error::Domain);
    };
    if base.coordinates != problem.coefficients {
        return Err(Error::Domain);
    }
    ambient(cas, &base.domain, &base.coordinates, budget)
}

/// There is no unexecuted fibre-domain predicate behind an otherwise complete root theorem.
fn ambient(
    cas: &Cas,
    node: &Handle,
    coordinates: &Handle,
    budget: &mut Budget,
) -> Result<(), Error> {
    let syntax = cas.describe_syntax(node, budget)?;
    let expected = AmbientDomain::Product {
        coordinates: coordinates.clone(),
    };
    if !matches!(syntax.node, SyntaxNode::Region(actual, RegionConstructor::Ambient) if actual == expected)
        || !syntax.scope.is_empty()
        || !syntax.dependencies.is_empty()
    {
        return Err(Error::Domain);
    }
    Ok(())
}

/// Exactly the original two formal positive inputs populate the coefficient space.
fn coordinates(
    cas: &Cas,
    node: &Handle,
    inputs: &[Handle],
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.work(1, AT)?;
    let SolveView::CoordinateSpace(data) = cas.solve_view(node)? else {
        return Err(Error::Domain);
    };
    if data.coordinates.len() != 2 || inputs.len() != 2 {
        return Err(Error::Domain);
    }
    for (coordinate, input) in data.coordinates.iter().zip(inputs) {
        budget.work(1, AT)?;
        if coordinate.role != CoordinateRole::Coefficient || coordinate.field != Field::Real {
            return Err(Error::Domain);
        }
        syntax::operands(
            cas,
            input,
            Field::Real,
            &Operator::Free(coordinate.key.clone()),
            0,
            budget,
        )?;
    }
    Ok(())
}

/// The original unknown is one unmodified Complex scalar declaration.
pub(super) fn unknown(
    cas: &Cas,
    space: &Handle,
    value: &Handle,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.work(1, AT)?;
    let SolveView::CoordinateSpace(data) = cas.solve_view(space)? else {
        return Err(Error::Structure);
    };
    let [coordinate] = data.coordinates.as_slice() else {
        return Err(Error::Structure);
    };
    if coordinate.field != Field::Complex || coordinate.role != CoordinateRole::Unknown {
        return Err(Error::Structure);
    }
    syntax::operands(
        cas,
        value,
        Field::Complex,
        &Operator::Free(coordinate.key.clone()),
        0,
        budget,
    )?;
    let (_, sort) = cas.expression(value)?;
    if *sort != Sort::Value(AtomicSort::Scalar(Field::Complex)) {
        return Err(Error::Structure);
    }
    Ok(())
}

/// The real target stratum executes exactly the same two positive guards as the original base.
pub(super) fn target(
    cas: &Cas,
    operator: &super::PositiveReciprocalMap,
    budget: &mut Budget,
) -> Result<(), Error> {
    let region = cas.admissible_region(operator.frame().context())?;
    let described = cas.describe_syntax(&region, budget)?;
    let SyntaxNode::Region(AmbientDomain::Product { coordinates: space }, _) = described.node
    else {
        return Err(Error::Domain);
    };
    if described
        .dependencies
        .iter()
        .any(|edge| matches!(edge.role, SyntaxRole::Family | SyntaxRole::BaseStratum))
    {
        return Err(Error::Domain);
    }
    coordinates(cas, &space, operator.inputs(), budget)?;
    positive(cas, &region, &space, operator.inputs(), budget)
}
