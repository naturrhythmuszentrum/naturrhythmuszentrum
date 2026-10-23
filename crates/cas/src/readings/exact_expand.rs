// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded producer expansion; the independent matrix checker supplies all equality authority.

use super::{AT, affine::length, invalid, reserve};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId, size};
use crate::model::Operator;
use std::collections::BTreeMap;

/// Expand only native matrices and ordered products, retaining every original operand.
pub(crate) fn expand(
    cas: &mut Cas,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    let mut values = BTreeMap::new();
    let mut pending = vec![(root, false)];
    let mut retained = 0_u64;
    while let Some((id, expanded)) = pending.pop() {
        budget.work(size(values.len(), AT)?.saturating_add(1), AT)?;
        if values.contains_key(&id) {
            continue;
        }
        let term = cas.state.expression(id)?;
        if matches!(term.operator, Operator::MatrixMultiply) && !expanded {
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
        let result = node(cas, id, &values, budget);
        budget.restore_limits(previous);
        let entries = result?;
        retained = retained
            .saturating_add(size(entries.len(), AT)?.saturating_mul(64))
            .saturating_add(64);
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        values.insert(id, entries);
    }
    values
        .remove(&root)
        .ok_or_else(|| invalid(InvalidCode::ReadingDefinition))
}

/// Materialize one native node as an ordered scalar coefficient array.
fn node(
    cas: &mut Cas,
    id: LogicalId,
    values: &BTreeMap<LogicalId, Vec<LogicalId>>,
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    let (rows, columns) = crate::polynomial::matrix::shape(&cas.state, id)?;
    super::storage::slots(length(rows, columns)?, budget, |budget| {
        let mut result = reserve(length(rows, columns)?, budget)?;
        let term = cas.state.expression(id)?;
        match term.operator {
            Operator::Matrix { .. } => result.extend_from_slice(&term.operands),
            Operator::MatrixMultiply => {
                let [left, right] = term.operands.as_slice() else {
                    return Err(invalid(InvalidCode::ReadingDefinition));
                };
                let header = term.header;
                let (_, inner) = crate::polynomial::matrix::shape(&cas.state, *left)?;
                let first = values
                    .get(left)
                    .ok_or_else(|| invalid(InvalidCode::Reference))?;
                let second = values
                    .get(right)
                    .ok_or_else(|| invalid(InvalidCode::Reference))?;
                for position in 0..length(rows, columns)? {
                    let position =
                        u32::try_from(position).map_err(|_| invalid(InvalidCode::ReadingShape))?;
                    result.push(contract(
                        cas,
                        header,
                        first,
                        second,
                        (position / columns, position % columns, inner, columns),
                        budget,
                    )?);
                }
            }
            _ => return Err(invalid(InvalidCode::ReadingDefinition)),
        }
        Ok(result)
    })
}

/// Propose an ordered scalar dot product without skipping zero operands.
fn contract(
    cas: &mut Cas,
    header: LogicalId,
    left: &[LogicalId],
    right: &[LogicalId],
    dimensions: (u32, u32, u32, u32),
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let (row, column, inner, columns) = dimensions;
    let mut sum = cas.map_scalar(
        header,
        Operator::Rational(crate::Rational::zero()),
        &[],
        budget,
    )?;
    for coordinate in 0..inner {
        budget.work(1, AT)?;
        let first = *left
            .get(length(row, inner)?.saturating_add(length(coordinate, 1)?))
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        let second = *right
            .get(length(coordinate, columns)?.saturating_add(length(column, 1)?))
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        let product = cas.map_scalar(header, Operator::Mul, &[first, second], budget)?;
        sum = cas.map_scalar(header, Operator::Add, &[sum, product], budget)?;
    }
    Ok(sum)
}
