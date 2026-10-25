// SPDX-License-Identifier: AGPL-3.0-only
//! Dense exact affine coefficients preserve repeated-coordinate cancellation.

use std::collections::BTreeMap;

use super::evaluate::Value;
use super::{AT, unsupported};
use crate::budget::Budget;
use crate::decision::CoordinateInterval;
use crate::error::Fault;
use crate::graph::{LogicalId, size};
use crate::model::Operator;
use crate::number::Rational;

/// Already checked coefficients and Boolean bounds, indexed by exact local syntax.
type Values = BTreeMap<LogicalId, Value>;

/// Borrow only a previously reconstructed affine value.
pub(super) fn get(values: &Values, id: LogicalId) -> Result<&[Rational], Fault> {
    match values.get(&id) {
        Some(Value::Affine(value)) => Ok(value),
        _ => Err(unsupported()),
    }
}

/// Interpret the closed affine grammar without simplifying unsupported original operands.
pub(super) fn build(
    operator: &Operator,
    arguments: &[LogicalId],
    id: LogicalId,
    intervals: &[CoordinateInterval],
    values: &Values,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    match operator {
        Operator::Rational(value) => constant(value, intervals.len(), budget),
        Operator::Free(_) => {
            let index = intervals
                .iter()
                .position(|interval| interval.coordinate.logical_id() == id)
                .ok_or_else(unsupported)?;
            let mut result = constant(&Rational::zero(), intervals.len(), budget)?;
            *result
                .get_mut(index.saturating_add(1))
                .ok_or_else(unsupported)? = Rational::one();
            Ok(result)
        }
        Operator::Neg => scale(
            first(values, arguments)?,
            &Rational::from_ratio(-1, 1, budget)?,
            false,
            budget,
        ),
        Operator::Add | Operator::Sub => combine(
            first(values, arguments)?,
            second(values, arguments)?,
            matches!(operator, Operator::Sub),
            budget,
        ),
        Operator::Mul => multiply(
            first(values, arguments)?,
            second(values, arguments)?,
            budget,
        ),
        Operator::Div => {
            let right = second(values, arguments)?;
            if !is_constant(right, budget)? {
                return Err(unsupported());
            }
            let divisor = right.first().ok_or_else(unsupported)?;
            if divisor.is_zero() {
                return Err(unsupported());
            }
            scale(first(values, arguments)?, divisor, true, budget)
        }
        _ => Err(unsupported()),
    }
}

/// Borrow the first ordered scalar operand.
fn first<'a>(values: &'a Values, arguments: &[LogicalId]) -> Result<&'a [Rational], Fault> {
    get(values, *arguments.first().ok_or_else(unsupported)?)
}

/// Borrow the second ordered scalar operand.
fn second<'a>(values: &'a Values, arguments: &[LogicalId]) -> Result<&'a [Rational], Fault> {
    get(values, *arguments.get(1).ok_or_else(unsupported)?)
}

/// Allocate inside the enclosing prepaid dense-coefficient reservation.
fn vector(count: usize) -> Result<Vec<Rational>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(result)
}

/// Put the exact constant before one coefficient for every retained coordinate.
fn constant(
    value: &Rational,
    dimensions: usize,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    let count = dimensions.checked_add(1).ok_or_else(unsupported)?;
    budget.work(size(count, AT)?, AT)?;
    let mut result = vector(count)?;
    result.push(value.add(&Rational::zero(), budget)?);
    result.resize_with(count, Rational::zero);
    Ok(result)
}

/// A constant affine expression has no nonzero coordinate coefficient.
pub(super) fn is_constant(value: &[Rational], budget: &mut Budget) -> Result<bool, Fault> {
    budget.work(size(value.len(), AT)?, AT)?;
    Ok(value.iter().skip(1).all(Rational::is_zero))
}

/// Add or subtract every corresponding exact coefficient.
pub(super) fn combine(
    left: &[Rational],
    right: &[Rational],
    subtract: bool,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    if left.len() != right.len() {
        return Err(unsupported());
    }
    let mut result = vector(left.len())?;
    for (a, b) in left.iter().zip(right) {
        result.push(if subtract {
            a.sub(b, budget)?
        } else {
            a.add(b, budget)?
        });
    }
    Ok(result)
}

/// Multiply or divide every coefficient by an already checked nonzero divisor when required.
fn scale(
    value: &[Rational],
    factor: &Rational,
    divide: bool,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    let mut result = vector(value.len())?;
    for coefficient in value {
        result.push(if divide {
            coefficient.div(factor, budget)?
        } else {
            coefficient.mul(factor, budget)?
        });
    }
    Ok(result)
}

/// Multiplication is affine only when at least one reconstructed operand is constant.
fn multiply(
    left: &[Rational],
    right: &[Rational],
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    if is_constant(left, budget)? {
        scale(right, left.first().ok_or_else(unsupported)?, false, budget)
    } else if is_constant(right, budget)? {
        scale(left, right.first().ok_or_else(unsupported)?, false, budget)
    } else {
        Err(unsupported())
    }
}
