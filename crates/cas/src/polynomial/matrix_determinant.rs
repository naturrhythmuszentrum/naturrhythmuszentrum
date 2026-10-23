// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact determinant checking by elimination in the scalar rational-function field.

use super::matrix::{bytes, expand, shape};
use super::{AT, Fraction, Polynomial, fragment};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{LogicalId, State};
use crate::model::Operator;
use crate::number::Rational;
use crate::readings::affine::length;

/// Internal field pivots are formal polynomials, never pointwise nonsingularity premises.
pub(super) fn determinant(
    state: &State,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<Fraction, Fault> {
    let (count, columns) = shape(state, root)?;
    if count != columns {
        return Err(fragment());
    }
    let mut values = expand(state, root, budget)?;
    let mut result = Fraction::polynomial(Polynomial::constant(&Rational::one(), budget)?, budget)?;
    for column in 0..count {
        let Some(pivot) = pivot(&values, count, column, budget)? else {
            return Fraction::polynomial(Polynomial::constant(&Rational::zero(), budget)?, budget);
        };
        if pivot != column {
            budget.work(u64::from(count), AT)?;
            for index in 0..count {
                values.swap(
                    position(pivot, index, count)?,
                    position(column, index, count)?,
                );
            }
            result.numerator = result.numerator.negated(budget)?;
        }
        let value = values
            .get(position(column, column, count)?)
            .ok_or_else(fragment)?;
        let retained = bytes(&values, budget)?
            .saturating_add(bytes(std::slice::from_ref(&result), budget)?)
            .saturating_mul(8);
        let previous = budget.reserve_temporary(retained, AT)?;
        let product = result.binary(value, &Operator::Mul, budget);
        budget.restore_limits(previous);
        result = product?;
        let retained = bytes(std::slice::from_ref(&result), budget)?.saturating_mul(8);
        let previous = budget.reserve_temporary(retained, AT)?;
        let reduced = eliminate(&mut values, count, column, budget);
        budget.restore_limits(previous);
        reduced?;
    }
    Ok(result)
}

/// Find an identically nonzero formal pivot; no sampled value participates.
fn pivot(
    values: &[Fraction],
    count: u32,
    column: u32,
    budget: &mut Budget,
) -> Result<Option<u32>, Fault> {
    for row in column..count {
        budget.work(1, AT)?;
        let value = values
            .get(position(row, column, count)?)
            .ok_or_else(fragment)?;
        if value.denominator.terms.is_empty() {
            return Err(fragment());
        }
        if !value.numerator.terms.is_empty() {
            return Ok(Some(row));
        }
    }
    Ok(None)
}

/// Elimination is an identity in rational functions; only the final determinant is a domain fact.
fn eliminate(
    values: &mut [Fraction],
    count: u32,
    column: u32,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for row in column.saturating_add(1)..count {
        let retained = bytes(values, budget)?.saturating_mul(8);
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = eliminate_row(values, (count, row, column), budget);
        budget.restore_limits(previous);
        result?;
    }
    Ok(())
}

/// All products and differences use the existing exact scalar arithmetic ledger.
fn eliminate_row(
    values: &mut [Fraction],
    indices: (u32, u32, u32),
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (count, row, pivot) = indices;
    let factor = values
        .get(position(row, pivot, count)?)
        .ok_or_else(fragment)?
        .binary(
            values
                .get(position(pivot, pivot, count)?)
                .ok_or_else(fragment)?,
            &Operator::Div,
            budget,
        )?;
    for column in pivot.saturating_add(1)..count {
        budget.work(1, AT)?;
        let retained = bytes(values, budget)?
            .saturating_add(bytes(std::slice::from_ref(&factor), budget)?)
            .saturating_mul(8);
        let previous = budget.reserve_temporary(retained, AT)?;
        let updated = subtract(values, (count, row, column, pivot), &factor, budget);
        budget.restore_limits(previous);
        *values
            .get_mut(position(row, column, count)?)
            .ok_or_else(fragment)? = updated?;
    }
    Ok(())
}

/// One independently metered row update retains the complete surrounding coefficient array.
fn subtract(
    values: &[Fraction],
    indices: (u32, u32, u32, u32),
    factor: &Fraction,
    budget: &mut Budget,
) -> Result<Fraction, Fault> {
    let (count, row, column, pivot) = indices;
    let product = factor.binary(
        values
            .get(position(pivot, column, count)?)
            .ok_or_else(fragment)?,
        &Operator::Mul,
        budget,
    )?;
    values
        .get(position(row, column, count)?)
        .ok_or_else(fragment)?
        .binary(&product, &Operator::Sub, budget)
}

/// Positional arithmetic follows the validated finite row-major rectangle.
fn position(row: u32, column: u32, count: u32) -> Result<usize, Fault> {
    length(row, count)?
        .checked_add(length(column, 1)?)
        .ok_or_else(fragment)
}
