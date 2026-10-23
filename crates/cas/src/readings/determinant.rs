// SPDX-License-Identifier: AGPL-3.0-only
//! Exact determinant proposals use finite permutations; the matrix proof independently checks them.

use super::{AT, affine::length, invalid, reserve};
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId, size};
use crate::model::Operator;
use crate::number::Rational;

/// The empty determinant is one. All other sizes share the caller's finite Work ledger.
pub(super) fn determinant(
    cas: &mut Cas,
    header: LogicalId,
    count: u32,
    entries: &[LogicalId],
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut order = reserve(length(count, 1)?, budget)?;
    order.extend(0..count);
    let mut sum = None;
    loop {
        let mut product =
            cas.map_scalar(header, Operator::Rational(Rational::one()), &[], budget)?;
        for (row, column) in order.iter().enumerate() {
            budget.work(1, AT)?;
            let row = u32::try_from(row).map_err(|_| invalid(InvalidCode::ReadingShape))?;
            let value = *entries
                .get(length(row, count)?.saturating_add(length(*column, 1)?))
                .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
            product = cas.map_scalar(header, Operator::Mul, &[product, value], budget)?;
        }
        if odd(&order, budget)? {
            product = cas.map_scalar(header, Operator::Neg, &[product], budget)?;
        }
        sum = Some(match sum {
            None => product,
            Some(previous) => {
                cas.map_scalar(header, Operator::Add, &[previous, product], budget)?
            }
        });
        if !next(&mut order, budget)? {
            break;
        }
    }
    sum.ok_or_else(|| invalid(InvalidCode::ReadingShape))
}

/// A cofactor removes exactly one ordered row and column, without reordering the others.
pub(super) fn minor(
    entries: &[LogicalId],
    count: u32,
    row: u32,
    column: u32,
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    let reduced = count
        .checked_sub(1)
        .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
    let mut result = reserve(length(reduced, reduced)?, budget)?;
    budget.work(size(entries.len(), AT)?, AT)?;
    for (position, entry) in entries.iter().enumerate() {
        let position = u32::try_from(position).map_err(|_| invalid(InvalidCode::ReadingShape))?;
        if position / count != row && position % count != column {
            result.push(*entry);
        }
    }
    Ok(result)
}

/// Count permutation inversions with a prepaid quadratic comparison bound.
fn odd(order: &[u32], budget: &mut Budget) -> Result<bool, Fault> {
    let count = size(order.len(), AT)?;
    budget.work(count.saturating_mul(count), AT)?;
    let mut parity = false;
    for (position, left) in order.iter().enumerate() {
        for right in order
            .get(position.saturating_add(1)..)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?
        {
            parity ^= left > right;
        }
    }
    Ok(parity)
}

/// Lexicographic permutation iteration has no recursive stack or unmetered search.
fn next(order: &mut [u32], budget: &mut Budget) -> Result<bool, Fault> {
    budget.work(size(order.len(), AT)?.saturating_mul(3), AT)?;
    let Some(pivot) = order
        .windows(2)
        .rposition(|pair| pair.first() < pair.get(1))
    else {
        return Ok(false);
    };
    let value = *order
        .get(pivot)
        .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
    let successor = order
        .iter()
        .rposition(|entry| *entry > value)
        .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
    order.swap(pivot, successor);
    order
        .get_mut(pivot.saturating_add(1)..)
        .ok_or_else(|| invalid(InvalidCode::ReadingShape))?
        .reverse();
    Ok(true)
}
