// SPDX-License-Identifier: AGPL-3.0-only
//! Structural reconstruction of closed primitive conclusions and ordered premises.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Expression, LogicalId, State, size};
use crate::model::{AtomicSort, Branch, Field, Operator, Sort};
use crate::number::Rational;
use crate::rule::Primitive;
use crate::scalar_model::{CheckedStep, ProofStep, Relation};

/// Static schema-checking location.
const AT: Location = Location::new("primitive schema");

/// A malformed schema instance never becomes an identity.
pub(crate) const fn mismatch() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Look up a strictly earlier ordered premise.
pub(crate) fn premise<'a>(
    step: &ProofStep,
    earlier: &'a [CheckedStep],
    position: usize,
) -> Result<&'a CheckedStep, Fault> {
    step.premises
        .get(position)
        .and_then(|index| usize::try_from(*index).ok())
        .and_then(|index| earlier.get(index))
        .ok_or_else(mismatch)
}

/// Require an exact premise count, with no unused or hidden justifications.
const fn count(step: &ProofStep, needed: usize) -> Result<(), Fault> {
    if step.premises.len() == needed {
        Ok(())
    } else {
        Err(mismatch())
    }
}

/// Read an exact unary shape.
pub(crate) fn unary(state: &State, id: LogicalId, operator: &Operator) -> Result<LogicalId, Fault> {
    let term = state.expression(id)?;
    if term.operator != *operator || term.operands.len() != 1 {
        return Err(mismatch());
    }
    term.operands.first().copied().ok_or_else(mismatch)
}

/// Read an exact binary shape.
pub(crate) fn binary(
    state: &State,
    id: LogicalId,
    operator: &Operator,
) -> Result<(LogicalId, LogicalId), Fault> {
    let term = state.expression(id)?;
    if term.operator != *operator || term.operands.len() != 2 {
        return Err(mismatch());
    }
    Ok((
        term.operands.first().copied().ok_or_else(mismatch)?,
        term.operands.get(1).copied().ok_or_else(mismatch)?,
    ))
}

/// Check a small literal without a textual conversion.
pub(crate) fn literal(
    state: &State,
    id: LogicalId,
    expected: i64,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let Operator::Rational(value) = &state.expression(id)?.operator else {
        return Ok(false);
    };
    value.check(budget, AT)?;
    budget.work(budget.limits().component_bits, AT)?;
    Ok(*value == Rational::from_ratio(expected, 1, budget)?)
}

/// Enforce the scalar field and strict-power convention before any field schema.
pub(crate) fn scalar(state: &State, context: LogicalId, id: LogicalId) -> Result<(), Fault> {
    let environment = &state.context(context)?.environment;
    if !matches!(environment.field, Field::Rational | Field::Real)
        || environment.branch != Branch::StrictReal
        || !matches!(
            state.expression(id)?.sort,
            Sort::Value(AtomicSort::Scalar(Field::Rational | Field::Real))
        )
    {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    Ok(())
}

/// Prepay complete metadata comparisons, including arbitrarily large checked scope frames.
pub(crate) fn prepay_shape(expression: &Expression, budget: &mut Budget) -> Result<(), Fault> {
    let mut out = crate::encoding::Encoder::new(budget, AT);
    crate::encoding::operator(&expression.operator, &mut out)?;
    crate::encoding::sort(&expression.sort, &mut out)?;
    crate::encoding::scope(&expression.scope, &mut out)?;
    crate::encoding::binding(expression.binding.as_ref(), &mut out)?;
    let bytes = size(out.finish().len(), AT)?;
    budget.work(bytes, AT)
}

/// Check all equality schemas except simultaneous substitution.
pub(crate) fn equality(
    cas: &Cas,
    step: &ProofStep,
    earlier: &[CheckedStep],
    context: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if step.relation != Relation::EqualOnDomain {
        return Err(mismatch());
    }
    let left = cas.checked(&step.left)?;
    let right = cas.checked(&step.right)?;
    let state = &cas.state;
    let valid = match step.primitive {
        Primitive::Jets if crate::polynomial::matrix::applicable(state, left, right)? => {
            count(step, 0)?;
            crate::polynomial::matrix::equal(state, left, right, budget)?
        }
        Primitive::Jets if crate::readings::check::applicable(state, left, right)? => {
            count(step, 0)?;
            crate::readings::check::equal(state, left, right, budget)?
        }
        Primitive::Reflexivity => {
            count(step, 0)?;
            left == right
        }
        Primitive::Symmetry => {
            count(step, 1)?;
            let previous = premise(step, earlier, 0)?;
            previous.relation == Relation::EqualOnDomain
                && previous.left == right
                && previous.right == left
        }
        Primitive::Transitivity => {
            count(step, 2)?;
            let a = premise(step, earlier, 0)?;
            let b = premise(step, earlier, 1)?;
            a.relation == Relation::EqualOnDomain
                && b.relation == Relation::EqualOnDomain
                && a.left == left
                && a.right == b.left
                && b.right == right
        }
        Primitive::Congruence => {
            count(step, 1)?;
            congruence(state, left, right, step, premise(step, earlier, 0)?, budget)?
        }
        _ => {
            scalar(state, context, left)?;
            count(step, 0)?;
            field(state, left, right, step.primitive, budget)?
        }
    };
    if valid { Ok(()) } else { Err(mismatch()) }
}

/// Verify a single ordered replacement path without treating adjacency as a proof.
fn congruence(
    state: &State,
    mut left: LogicalId,
    mut right: LogicalId,
    step: &ProofStep,
    previous: &CheckedStep,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    if previous.relation != Relation::EqualOnDomain {
        return Ok(false);
    }
    for position in &step.path {
        let a = state.expression(left)?;
        let b = state.expression(right)?;
        prepay_shape(a, budget)?;
        prepay_shape(b, budget)?;
        budget.work(size(a.operands.len(), AT)?, AT)?;
        if a.operator != b.operator
            || a.sort != b.sort
            || a.scope != b.scope
            || a.binding != b.binding
            || a.operands.len() != b.operands.len()
        {
            return Ok(false);
        }
        let position = usize::try_from(*position).map_err(|_| mismatch())?;
        for (index, (x, y)) in a.operands.iter().zip(&b.operands).enumerate() {
            if index != position && x != y {
                return Ok(false);
            }
        }
        left = a.operands.get(position).copied().ok_or_else(mismatch)?;
        right = b.operands.get(position).copied().ok_or_else(mismatch)?;
    }
    Ok(left == previous.left && right == previous.right)
}

/// Fixed scalar field shapes, finite-jet identities and exact arithmetic.
fn field(
    state: &State,
    left: LogicalId,
    right: LogicalId,
    primitive: Primitive,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    match primitive {
        Primitive::AddCommute
        | Primitive::MulCommute
        | Primitive::AddAssociate
        | Primitive::MulAssociate
        | Primitive::Distribute => structural_field(state, left, right, primitive),
        Primitive::AddZero | Primitive::MulOne | Primitive::MulZero => {
            let operator = if primitive == Primitive::AddZero {
                Operator::Add
            } else {
                Operator::Mul
            };
            let (a, b) = binary(state, left, &operator)?;
            let expected = i64::from(primitive == Primitive::MulOne);
            Ok(literal(state, b, expected, budget)?
                && if primitive == Primitive::MulZero {
                    literal(state, right, 0, budget)?
                } else {
                    a == right
                })
        }
        Primitive::AddInverse => {
            let (a, b) = binary(state, left, &Operator::Add)?;
            Ok(unary(state, b, &Operator::Neg)? == a && literal(state, right, 0, budget)?)
        }
        Primitive::Cancel => {
            let (a, b) = binary(state, left, &Operator::Div)?;
            Ok(a == b && literal(state, right, 1, budget)?)
        }
        Primitive::Power => power(state, left, right, budget),
        Primitive::Exact => exact(state, left, right, budget),
        Primitive::Jets | Primitive::RationalIdentity => crate::polynomial::equal(
            state,
            left,
            right,
            primitive == Primitive::RationalIdentity,
            budget,
        ),
        _ => Err(mismatch()),
    }
}

/// Reconstruct associativity, commutativity and distributivity with repeated identities exact.
fn structural_field(
    state: &State,
    left: LogicalId,
    right: LogicalId,
    primitive: Primitive,
) -> Result<bool, Fault> {
    match primitive {
        Primitive::AddCommute | Primitive::MulCommute => {
            let operator = if primitive == Primitive::AddCommute {
                Operator::Add
            } else {
                Operator::Mul
            };
            let (a, b) = binary(state, left, &operator)?;
            Ok(binary(state, right, &operator)? == (b, a))
        }
        Primitive::AddAssociate | Primitive::MulAssociate => {
            let operator = if primitive == Primitive::AddAssociate {
                Operator::Add
            } else {
                Operator::Mul
            };
            let (ab, c) = binary(state, left, &operator)?;
            let (a, b) = binary(state, ab, &operator)?;
            let (x, bc) = binary(state, right, &operator)?;
            Ok(x == a && binary(state, bc, &operator)? == (b, c))
        }
        Primitive::Distribute => {
            let (a, bc) = binary(state, left, &Operator::Mul)?;
            let (b, c) = binary(state, bc, &Operator::Add)?;
            let (ab, ac) = binary(state, right, &Operator::Add)?;
            Ok(binary(state, ab, &Operator::Mul)? == (a, b)
                && binary(state, ac, &Operator::Mul)? == (a, c))
        }
        _ => Err(mismatch()),
    }
}

/// Strict-power recurrence avoids a new zero-power domain restriction at exponent one.
fn power(
    state: &State,
    left: LogicalId,
    right: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let term = state.expression(left)?;
    let Operator::IntegerPower(exponent) = term.operator else {
        return Err(mismatch());
    };
    let base = term.operands.first().copied().ok_or_else(mismatch)?;
    match exponent {
        0 => literal(state, right, 1, budget),
        1 => Ok(right == base),
        2.. => {
            let (a, b) = binary(state, right, &Operator::Mul)?;
            let previous = exponent.checked_sub(1).ok_or_else(mismatch)?;
            Ok(a == base && unary(state, b, &Operator::IntegerPower(previous))? == base)
        }
        _ => {
            let positive = exponent
                .checked_neg()
                .ok_or_else(|| Fault::capacity(BudgetKind::DecimalExponent, AT))?;
            let (a, b) = binary(state, right, &Operator::Div)?;
            Ok(literal(state, a, 1, budget)?
                && unary(state, b, &Operator::IntegerPower(positive))? == base)
        }
    }
}

/// Recompute literal results with the crate's measured exact rational primitives.
fn exact(
    state: &State,
    left: LogicalId,
    right: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let term = state.expression(left)?;
    let number = |position: usize| -> Result<&Rational, Fault> {
        let id = term.operands.get(position).copied().ok_or_else(mismatch)?;
        match &state.expression(id)?.operator {
            Operator::Rational(value) => Ok(value),
            _ => Err(mismatch()),
        }
    };
    let value = match &term.operator {
        Operator::Rational(value) => {
            value.check(budget, AT)?;
            value.clone()
        }
        Operator::Neg => number(0)?.negated(budget)?,
        Operator::Add => number(0)?.add(number(1)?, budget)?,
        Operator::Sub => number(0)?.sub(number(1)?, budget)?,
        Operator::Mul => number(0)?.mul(number(1)?, budget)?,
        Operator::Div => number(0)?.div(number(1)?, budget)?,
        Operator::IntegerPower(exponent) => number(0)?.pow(*exponent, budget)?,
        _ => return Err(mismatch()),
    };
    budget.work(budget.limits().component_bits, AT)?;
    Ok(matches!(
        &state.expression(right)?.operator,
        Operator::Rational(expected) if *expected == value,
    ))
}
