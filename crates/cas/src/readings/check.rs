// SPDX-License-Identifier: AGPL-3.0-only
//! Reconstruct complete ordered affine forms over one exact coordinate family.

use super::{AT, AffineMap, invalid, reserve};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{LogicalId, State, size};
use crate::model::Operator;
use crate::number::Rational;

/// A constant-size syntax dispatch does not traverse or charge older scalar paths.
pub(crate) fn applicable(state: &State, left: LogicalId, right: LogicalId) -> Result<bool, Fault> {
    for root in [left, right] {
        if matches!(
            state.expression(root)?.operator,
            Operator::Select(_)
                | Operator::AffineReading(_)
                | Operator::RealPart
                | Operator::ImagPart
                | Operator::Conjugate
                | Operator::Transpose
        ) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Compare coefficient vectors only after rebuilding every original source.
/// Every admitted operator in this fragment is total on the declared coordinates.
pub(crate) fn equal(
    state: &State,
    left: LogicalId,
    right: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let (family, first) = normalize(state, left, budget)?;
    let retained = first.validate(budget)?;
    // The complete original chain has been checked once in this fixed application frame.
    // Identical canonical source nodes share that derivation, including unsupported-source refusals.
    if left == right {
        return Ok(true);
    }
    let previous = budget.reserve_temporary(retained, AT)?;
    let second = normalize(state, right, budget);
    budget.restore_limits(previous);
    let (other_family, second) = second?;
    second.validate(budget)?;
    if family != other_family {
        return Err(invalid(InvalidCode::ReadingUnrelated));
    }
    if first != second {
        return Err(invalid(InvalidCode::ReadingMismatch));
    }
    Ok(true)
}

/// Walk the one semantic source per definition iteratively, never recursively.
fn normalize(
    state: &State,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<(LogicalId, AffineMap), Fault> {
    let (family, chain) = source_chain(state, root, budget)?;
    let count = super::syntax::coordinates(&state.expression(family)?.sort)?;
    budget.check(BudgetKind::FamilyParameters, u64::from(count), AT)?;
    let mut map = AffineMap::identity(count, budget)?;
    let chain_bytes = size(chain.len(), AT)?
        .checked_mul(64)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    for node in chain.into_iter().rev() {
        budget.work(1, AT)?;
        let previous = budget.reserve_temporary(chain_bytes, AT)?;
        let result = apply(state, node, &map, budget);
        budget.restore_limits(previous);
        map = result?;
    }
    Ok((family, map))
}

/// No zero coefficient can skip an unsupported or invalid original source.
fn source_chain(
    state: &State,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<(LogicalId, Vec<LogicalId>), Fault> {
    let mut current = root;
    let mut chain = Vec::new();
    loop {
        budget.work(1, AT)?;
        let value = state.expression(current)?;
        if !value.scope.is_empty() || value.space.is_some() {
            return Err(invalid(InvalidCode::ReadingDefinition));
        }
        if matches!(value.operator, Operator::Free(_)) {
            return Ok((current, chain));
        }
        let operand = match value.operator {
            Operator::AffineReading(_) => 1,
            Operator::Select(_)
            | Operator::RealPart
            | Operator::ImagPart
            | Operator::Conjugate
            | Operator::Transpose => 0,
            _ => return Err(invalid(InvalidCode::ReadingDefinition)),
        };
        let next = value
            .operands
            .get(operand)
            .copied()
            .ok_or_else(|| invalid(InvalidCode::ReadingDefinition))?;
        let count = size(chain.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        budget.check(BudgetKind::Depth, count, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count
                .checked_mul(64)
                .ok_or_else(|| invalid(InvalidCode::ReadingShape))?,
            AT,
        )?;
        chain.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        chain.push(current);
        current = next;
    }
}

/// Every constructor preserves the already reconstructed ordered source coordinates.
pub(super) fn apply(
    state: &State,
    node: LogicalId,
    source: &AffineMap,
    budget: &mut Budget,
) -> Result<AffineMap, Fault> {
    let value = state.expression(node)?;
    match &value.operator {
        Operator::AffineReading(map) => map.compose(source, budget),
        Operator::Select(selection) => selection.compose(source, budget),
        Operator::RealPart => source.project(&[0], budget),
        Operator::ImagPart => source.project(&[1], budget),
        Operator::Conjugate => conjugation(budget)?.compose(source, budget),
        Operator::Transpose => transpose(state, node, source, budget),
        _ => Err(invalid(InvalidCode::ReadingDefinition)),
    }
}

/// Conjugation is the exact ordered Real map `(a,b) -> (a,-b)`.
fn conjugation(budget: &mut Budget) -> Result<AffineMap, Fault> {
    let negative = Rational::one().negated(budget)?;
    AffineMap::new(
        2,
        2,
        vec![
            Rational::one(),
            Rational::zero(),
            Rational::zero(),
            negative,
        ],
        vec![Rational::zero(), Rational::zero()],
        budget,
    )
}

/// Matrix transposition permutes logical coordinates; scalar product rules never apply.
fn transpose(
    state: &State,
    node: LogicalId,
    source: &AffineMap,
    budget: &mut Budget,
) -> Result<AffineMap, Fault> {
    let value = state.expression(node)?;
    let input = value
        .operands
        .first()
        .copied()
        .ok_or_else(|| invalid(InvalidCode::ReadingDefinition))?;
    let (_, rows, columns) = super::selection::rectangle(&state.expression(input)?.sort)?;
    let mut order = reserve(super::affine::length(rows, columns)?, budget)?;
    for column in 0..rows {
        for row in 0..columns {
            order.push(
                row.checked_mul(rows)
                    .and_then(|value| value.checked_add(column))
                    .ok_or_else(|| invalid(InvalidCode::ReadingShape))?,
            );
        }
    }
    source.project(&order, budget)
}
