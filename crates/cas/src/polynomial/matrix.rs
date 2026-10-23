// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered native matrix contraction over the existing exact scalar coefficient field.

use super::{AT, Fraction, Polynomial, convert_scalar, fragment};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{LogicalId, State, size};
use crate::model::{AtomicSort, Extent, Field, Operator, Sort};
use crate::number::Rational;
use std::collections::{BTreeMap, BTreeSet};

/// A dispatch restricted to explicit matrices and their ordered products.
pub(crate) fn applicable(state: &State, left: LogicalId, right: LogicalId) -> Result<bool, Fault> {
    for id in [left, right] {
        if matches!(
            state.expression(id)?.operator,
            Operator::Matrix { .. } | Operator::MatrixMultiply
        ) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Native map endpoints must retain explicit coefficient definitions at every matrix leaf.
/// Bare declared matrices keep the existing reading-definition refusal.
pub(crate) fn definition(state: &State, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        let count = size(
            visited
                .len()
                .saturating_add(pending.len())
                .saturating_add(1),
            AT,
        )?;
        budget.work(count, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count.saturating_mul(128),
            AT,
        )?;
        if !visited.insert(id) {
            continue;
        }
        let term = state.expression(id)?;
        match term.operator {
            Operator::Matrix { .. } => {}
            Operator::MatrixMultiply => {
                pending
                    .try_reserve(term.operands.len())
                    .map_err(|_| Fault::allocation(AT))?;
                pending.extend(term.operands.iter().copied());
            }
            _ => return Err(Fault::invalid(InvalidCode::ReadingDefinition, AT)),
        }
    }
    Ok(())
}

/// Shape arithmetic is checked before allocating; no unused empty axis is traversed.
pub(crate) fn shape(state: &State, id: LogicalId) -> Result<(u32, u32), Fault> {
    match &state.expression(id)?.sort {
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(columns),
        }) => Ok((*rows, *columns)),
        _ => Err(fragment()),
    }
}

/// Rebuild both complete operands, retaining their exact scalar domains independently.
pub(crate) fn equal(
    state: &State,
    left: LogicalId,
    right: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    if shape(state, left)? != shape(state, right)? {
        return Err(Fault::invalid(InvalidCode::ReadingShape, AT));
    }
    let first = expand(state, left, budget)?;
    let retained = bytes(&first, budget)?;
    let previous = budget.reserve_temporary(retained, AT)?;
    let result = compare(state, right, &first, budget);
    budget.restore_limits(previous);
    if result? {
        Ok(true)
    } else {
        Err(Fault::invalid(InvalidCode::ReadingMismatch, AT))
    }
}

/// Compare every ordered scalar fraction without discarding an invalid denominator.
fn compare(
    state: &State,
    right: LogicalId,
    first: &[Fraction],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let second = expand(state, right, budget)?;
    let retained = bytes(&second, budget)?;
    let previous = budget.reserve_temporary(retained, AT)?;
    let result = first.iter().zip(&second).try_fold(true, |same, (a, b)| {
        if a.denominator.terms.is_empty() || b.denominator.terms.is_empty() {
            return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
        }
        let left = a.numerator.mul(&b.denominator, budget)?;
        let right = b.numerator.mul(&a.denominator, budget)?;
        left.prepay(budget)?;
        right.prepay(budget)?;
        Ok(same && left.terms == right.terms)
    });
    budget.restore_limits(previous);
    result
}

/// Charge all live coefficient maps before the next nested conversion or contraction.
pub(super) fn bytes(values: &[Fraction], budget: &mut Budget) -> Result<u64, Fault> {
    let mut total = size(values.len(), AT)?.saturating_mul(64);
    for value in values {
        total = total.saturating_add(value.numerator.bytes(budget)?);
        total = total.saturating_add(value.denominator.bytes(budget)?);
    }
    budget.check(BudgetKind::StagedPayloadBytes, total, AT)?;
    Ok(total)
}

/// Reconstruct a native matrix DAG using a charged explicit postorder stack.
pub(super) fn expand(
    state: &State,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<Vec<Fraction>, Fault> {
    let mut values = BTreeMap::new();
    let mut pending = vec![(root, false)];
    let mut retained = 0_u64;
    while let Some((id, expanded)) = pending.pop() {
        budget.work(size(values.len(), AT)?.saturating_add(1), AT)?;
        if values.contains_key(&id) {
            continue;
        }
        let term = state.expression(id)?;
        let composite = matches!(term.operator, Operator::MatrixMultiply);
        if composite && !expanded {
            budget.check(
                BudgetKind::StagedPayloadBytes,
                retained.saturating_add(
                    size(pending.len(), AT)?
                        .saturating_add(3)
                        .saturating_mul(64),
                ),
                AT,
            )?;
            pending.try_reserve(3).map_err(|_| Fault::allocation(AT))?;
            pending.push((id, true));
            pending.extend(term.operands.iter().rev().map(|child| (*child, false)));
            continue;
        }
        let previous = budget.reserve_temporary(retained, AT)?;
        let value = expand_node(state, id, &values, budget);
        budget.restore_limits(previous);
        let value = value?;
        retained = retained
            .saturating_add(bytes(&value, budget)?)
            .saturating_add(64);
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        values.insert(id, value);
    }
    values.remove(&root).ok_or_else(fragment)
}

/// Build one literal or product while retaining every earlier coefficient.
fn expand_node(
    state: &State,
    id: LogicalId,
    values: &BTreeMap<LogicalId, Vec<Fraction>>,
    budget: &mut Budget,
) -> Result<Vec<Fraction>, Fault> {
    let term = state.expression(id)?;
    let (rows, columns) = shape(state, id)?;
    let count = crate::readings::affine::length(rows, columns)?;
    budget.check(BudgetKind::MatrixEntries, size(count, AT)?, AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        size(count, AT)?.saturating_mul(64),
        AT,
    )?;
    budget.work(size(count, AT)?, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    match term.operator {
        Operator::Matrix { .. } => {
            for child in &term.operands {
                let retained = bytes(&result, budget)?;
                let previous = budget.reserve_temporary(retained, AT)?;
                let value = convert_scalar(state, *child, budget);
                budget.restore_limits(previous);
                result.push(value?);
            }
        }
        Operator::MatrixMultiply => {
            let [left, right] = term.operands.as_slice() else {
                return Err(fragment());
            };
            let (_, inner) = shape(state, *left)?;
            let first = values.get(left).ok_or_else(fragment)?;
            let second = values.get(right).ok_or_else(fragment)?;
            for position in 0..count {
                let position = u32::try_from(position).map_err(|_| fragment())?;
                let retained = bytes(&result, budget)?;
                let previous = budget.reserve_temporary(retained, AT)?;
                let value = contract(
                    first,
                    second,
                    (position / columns, position % columns, inner, columns),
                    budget,
                );
                budget.restore_limits(previous);
                result.push(value?);
            }
        }
        _ => return Err(fragment()),
    }
    Ok(result)
}

/// Multiply row by column in order; scalar commutation never reorders matrices.
fn contract(
    left: &[Fraction],
    right: &[Fraction],
    dimensions: (u32, u32, u32, u32),
    budget: &mut Budget,
) -> Result<Fraction, Fault> {
    let (row, column, inner, columns) = dimensions;
    let mut sum = Fraction::polynomial(Polynomial::constant(&Rational::zero(), budget)?, budget)?;
    for coordinate in 0..inner {
        budget.work(1, AT)?;
        let first = crate::readings::affine::length(row, inner)?
            .saturating_add(crate::readings::affine::length(coordinate, 1)?);
        let second = crate::readings::affine::length(coordinate, columns)?
            .saturating_add(crate::readings::affine::length(column, 1)?);
        let product = left.get(first).ok_or_else(fragment)?.binary(
            right.get(second).ok_or_else(fragment)?,
            &Operator::Mul,
            budget,
        )?;
        sum = sum.binary(&product, &Operator::Add, budget)?;
    }
    Ok(sum)
}
