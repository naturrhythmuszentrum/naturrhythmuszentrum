// SPDX-License-Identifier: AGPL-3.0-only
//! Original denominator obligations must follow from exactly the runtime positive input boxes.

use super::{CoordinateNormalization, ReadingError, ReciprocalMapDefinition, support};
use nrz_cas::{Cas, Handle, limits::Budget, syntax::Operator};

/// Traverse original syntax before reduction, including coefficients that later become zero.
pub(super) fn check(
    cas: &Cas,
    frame: &CoordinateNormalization,
    proposed: &ReciprocalMapDefinition,
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    let mut pending = support::reserve(1, budget)?;
    pending.push(proposed.original.expression().clone());
    let mut cursor = 0;
    while cursor < pending.len() {
        let node = &pending[cursor];
        let retained = u64::try_from(
            pending
                .len()
                .checked_mul(std::mem::size_of::<Handle>())
                .ok_or(ReadingError::Shape)?,
        )
        .map_err(|_| ReadingError::Shape)?;
        let (_, operator, operands, header) =
            budget.with_retained_payload(retained, support::AT, |budget| {
                support::inspect(cas, node, budget)
            })?;
        if header != cas.context_header(frame.context())? {
            return Err(ReadingError::Definition);
        }
        admitted(cas, proposed, node, &operator, &operands, budget)?;
        for operand in operands {
            budget.work(
                u64::try_from(pending.len()).map_err(|_| ReadingError::Shape)?,
                support::AT,
            )?;
            if !pending.contains(&operand) {
                let count = pending.len().checked_add(1).ok_or(ReadingError::Shape)?;
                support::capacity::<Handle>(count, budget)?;
                pending
                    .try_reserve_exact(1)
                    .map_err(|_| nrz_cas::Fault::allocation(support::AT))?;
                pending.push(operand);
            }
        }
        cursor += 1;
    }
    Ok(())
}

/// Only total arithmetic and reciprocals covered by the executable box guards may occur.
fn admitted(
    cas: &Cas,
    proposed: &ReciprocalMapDefinition,
    node: &Handle,
    operator: &Operator,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    budget.work(
        u64::try_from(proposed.inputs.len()).map_err(|_| ReadingError::Shape)?,
        support::AT,
    )?;
    match operator {
        Operator::Rational(_)
        | Operator::Constant(_)
        | Operator::Neg
        | Operator::Add
        | Operator::Sub
        | Operator::Mul
        | Operator::Matrix { .. }
        | Operator::MatrixMultiply
        | Operator::Transpose => Ok(()),
        Operator::IntegerPower(power) if *power >= 0 => Ok(()),
        Operator::Free(_) if proposed.inputs.contains(node) => Ok(()),
        Operator::Div => denominator(
            cas,
            proposed,
            operands.get(1).ok_or(ReadingError::Definition)?,
            budget,
        ),
        _ => Err(ReadingError::Definition),
    }
}

/// A cancelled or zero-weighted original denominator cannot hide an additional domain condition.
fn denominator(
    cas: &Cas,
    proposed: &ReciprocalMapDefinition,
    value: &Handle,
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    budget.work(
        u64::try_from(proposed.inputs.len()).map_err(|_| ReadingError::Shape)?,
        support::AT,
    )?;
    if proposed.inputs.contains(value) {
        return Ok(());
    }
    let (_, operator, _, _) = support::inspect(cas, value, budget)?;
    if matches!(operator, Operator::Rational(value) if !value.is_zero()) {
        Ok(())
    } else {
        Err(ReadingError::Definition)
    }
}
