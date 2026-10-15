// SPDX-License-Identifier: AGPL-3.0-only
//! Distribution and test arities, exact spaces, and typed statement roles.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_model::DistributionStatement;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Expression, LogicalId, State};
use crate::model::{AtomicSort, Field, Operator, Sort};

/// Distribution admission location.
pub(crate) const AT: Location = Location::new("distribution syntax");

/// A malformed distribution expression is never reinterpreted as scalar syntax.
pub(crate) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Sort, AT)
}

/// These scalar expressions carry the space of their distributional action.
pub(crate) const fn scalar_action(value: &Operator) -> bool {
    matches!(
        value,
        Operator::Pair | Operator::PairNearSupport | Operator::TestApply | Operator::RegularAction
    )
}

/// Exact scalar combinations of action values preserve their common space.
pub(crate) const fn action_arithmetic(value: &Operator) -> bool {
    matches!(
        value,
        Operator::Neg | Operator::Add | Operator::Sub | Operator::Mul
    )
}

/// Check constructor arities and value categories without granting analytic membership.
pub(crate) fn sorts(
    value: &Operator,
    result: &Sort,
    children: &[&Expression],
    budget: &Budget,
) -> Result<bool, Fault> {
    let real = Sort::Value(AtomicSort::Scalar(Field::Real));
    let sorts: Vec<_> = children.iter().map(|child| &child.sort).collect();
    Ok(match value {
        Operator::RegularEmbed => *result == Sort::Distribution && sorts == [&real],
        Operator::DistributionZero => *result == Sort::Distribution && sorts.is_empty(),
        Operator::Delta { order } => {
            budget.check(BudgetKind::DeltaOrder, u64::from(*order), AT)?;
            *result == Sort::Distribution && sorts == [&real]
        }
        Operator::DistributionAdd => {
            *result == Sort::Distribution && sorts == [&Sort::Distribution, &Sort::Distribution]
        }
        Operator::DistributionScale | Operator::SmoothMultiply => {
            *result == Sort::Distribution && sorts == [&real, &Sort::Distribution]
        }
        Operator::DistributionDerivative { order } => {
            *order > 0 && *result == Sort::Distribution && sorts == [&Sort::Distribution]
        }
        Operator::Pair => *result == real && sorts == [&Sort::Distribution, &Sort::Test],
        Operator::PairNearSupport => *result == real && sorts == [&Sort::Distribution, &real],
        Operator::TestDerivative { order } => {
            *order > 0 && *result == Sort::Test && sorts == [&Sort::Test]
        }
        Operator::TestMultiply => *result == Sort::Test && sorts == [&real, &Sort::Test],
        Operator::TestApply => *result == real && sorts == [&Sort::Test, &real],
        Operator::RegularAction => *result == real && sorts == [&real, &Sort::Test],
        _ => false,
    })
}

/// Enforce field, operand-space and rational coefficient invariants after resolving references.
pub(crate) fn space(
    state: &State,
    operator: &Operator,
    sort: &Sort,
    space: LogicalId,
    operands: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let definition = state.space(space)?;
    if definition.field != Field::Real {
        return Err(invalid());
    }
    for operand in operands {
        let child = state.expression(*operand)?;
        if crate::header::space_sort(&child.sort) && child.space != Some(space) {
            return Err(invalid());
        }
    }
    if matches!(operator, Operator::DistributionScale) {
        let coefficient = state.expression(*operands.first().ok_or_else(invalid)?)?;
        if !matches!(coefficient.operator, Operator::Rational(_)) || coefficient.space.is_some() {
            return Err(invalid());
        }
    }
    if let Operator::Delta { order } = operator {
        budget.specialized(BudgetKind::DeltaOrder, u64::from(*order), false, AT)?;
    }
    if !crate::header::space_sort(sort) && !scalar_action(operator) && !action_arithmetic(operator)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Every statement has exact, noninterchangeable expression and region roles.
pub(crate) fn statement(
    cas: &Cas,
    value: &DistributionStatement,
    budget: &Budget,
) -> Result<(), Fault> {
    let space = cas.checked(value.space())?;
    cas.state.space(space)?;
    let real = Sort::Value(AtomicSort::Scalar(Field::Real));
    match value {
        DistributionStatement::Pullback(value) => pullback(cas, value)?,
        DistributionStatement::Regularity { function, .. } => {
            endpoint(cas, function, &real, None)?;
        }
        DistributionStatement::Embedding {
            function,
            embedding,
            ..
        } => {
            endpoint(cas, function, &real, None)?;
            endpoint(cas, embedding, &Sort::Distribution, Some(space))?;
        }
        DistributionStatement::Equality { left, right, .. }
        | DistributionStatement::PrimitiveUnique { left, right, .. } => {
            endpoint(cas, left, &Sort::Distribution, Some(space))?;
            endpoint(cas, right, &Sort::Distribution, Some(space))?;
        }
        DistributionStatement::Derivative {
            distribution,
            derivative,
            order,
            ..
        } => {
            if *order == 0 {
                return Err(invalid());
            }
            budget.check(BudgetKind::Depth, u64::from(*order), AT)?;
            endpoint(cas, distribution, &Sort::Distribution, Some(space))?;
            endpoint(cas, derivative, &Sort::Distribution, Some(space))?;
        }
        DistributionStatement::Pairing { pairing, value, .. } => {
            endpoint(cas, pairing, &real, Some(space))?;
            let output = cas.state.expression(cas.checked(value)?)?;
            if output.sort != real
                || !output.scope.is_empty()
                || output.space.is_some_and(|actual| actual != space)
            {
                return Err(invalid());
            }
        }
        DistributionStatement::SupportAction {
            distribution,
            function,
            neighborhood,
            ..
        } => {
            endpoint(cas, distribution, &Sort::Distribution, Some(space))?;
            endpoint(cas, function, &real, None)?;
            let region = cas.state.region(cas.checked(neighborhood)?)?;
            if region.ambient
                != crate::domain_model::AmbientDomain::Real(
                    cas.state.space(space)?.coordinate.clone(),
                )
                || !region.scope.is_empty()
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Check one closed endpoint without changing its header, scope, field or exact space.
fn endpoint(
    cas: &Cas,
    handle: &crate::graph::Handle,
    sort: &Sort,
    space: Option<LogicalId>,
) -> Result<(), Fault> {
    let value = cas.state.expression(cas.checked(handle)?)?;
    if value.sort != *sort || value.space != space || !value.scope.is_empty() {
        return Err(invalid());
    }
    Ok(())
}

/// Source and target coordinate roles cannot be exchanged by changing a displayed map.
fn pullback(cas: &Cas, value: &crate::pullback_model::PullbackValue) -> Result<(), Fault> {
    let source = cas.checked(&value.source_space)?;
    let target = cas.checked(&value.target_space)?;
    endpoint(cas, &value.source, &Sort::Distribution, Some(source))?;
    endpoint(cas, &value.result, &Sort::Distribution, Some(target))?;
    let real = Sort::Value(AtomicSort::Scalar(Field::Real));
    endpoint(cas, &value.map, &real, None)?;
    crate::boundary_roles::coordinate(cas, &value.map, &cas.state.space(target)?.coordinate)?;
    if let Some(inverse) = &value.inverse {
        endpoint(cas, inverse, &real, None)?;
        crate::boundary_roles::coordinate(cas, inverse, &cas.state.space(source)?.coordinate)?;
    }
    Ok(())
}
