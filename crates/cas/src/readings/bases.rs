// SPDX-License-Identifier: AGPL-3.0-only
//! Exact base identity includes its declaration snapshot and defining expression.

use super::{AT, EntrySelection, invalid};
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Expression, Handle, LogicalId, size};
use crate::model::Operator;

/// Resolve one declared base without accepting a printed name as authority.
pub(super) fn declaration(cas: &Cas, base: LogicalId) -> Result<&Expression, Fault> {
    let value = cas.state.expression(base)?;
    let declared = match value.operator {
        Operator::Free(_) => value,
        Operator::AffineReading(_) => {
            let id = value
                .operands
                .first()
                .copied()
                .ok_or_else(|| invalid(InvalidCode::ReadingDefinition))?;
            cas.state.expression(id)?
        }
        _ => return Err(invalid(InvalidCode::ReadingDefinition)),
    };
    if !matches!(declared.operator, Operator::Free(_))
        || !declared.scope.is_empty()
        || declared.space.is_some()
    {
        return Err(invalid(InvalidCode::ReadingDefinition));
    }
    Ok(declared)
}

/// Diagnose symbol, snapshot and defining-source disagreements independently.
pub(super) fn same(
    cas: &Cas,
    left: LogicalId,
    right: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(2, AT)?;
    let first = declaration(cas, left)?;
    let second = declaration(cas, right)?;
    let (Operator::Free(first_key), Operator::Free(second_key)) =
        (&first.operator, &second.operator)
    else {
        return Err(invalid(InvalidCode::ReadingDefinition));
    };
    budget.work(size(first_key.len().max(second_key.len()), AT)?, AT)?;
    if first_key != second_key {
        return Err(invalid(InvalidCode::SelectionBase));
    }
    if first.header != second.header {
        return Err(invalid(InvalidCode::SelectionDeclaration));
    }
    if left != right {
        return Err(invalid(InvalidCode::SelectionDefinition));
    }
    Ok(())
}

/// Only admitted selection expressions carry an authenticated base identity.
pub(super) fn selection<'a>(
    cas: &'a Cas,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<(LogicalId, &'a EntrySelection), Fault> {
    budget.work(1, AT)?;
    let value = cas.state.expression(cas.checked(handle)?)?;
    let Operator::Select(selected) = &value.operator else {
        return Err(invalid(InvalidCode::SelectionExpected));
    };
    let base = value
        .operands
        .first()
        .copied()
        .ok_or_else(|| invalid(InvalidCode::ReadingDefinition))?;
    declaration(cas, base)?;
    selected.validate(&cas.state.expression(base)?.sort, budget)?;
    Ok((base, selected))
}
