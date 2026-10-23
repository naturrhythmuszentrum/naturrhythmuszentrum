// SPDX-License-Identifier: AGPL-3.0-only
//! Shared reservations keep caller-owned result buffers live across nested proof operations.

use super::AT;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::size;

/// Prepay traversal and simultaneous conservative storage for finite result slots.
pub(super) fn reserve<T>(count: usize, budget: &mut Budget) -> Result<Vec<T>, Fault> {
    let length = size(count, AT)?;
    budget.check(BudgetKind::MatrixEntries, length, AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        length.saturating_mul(128),
        AT,
    )?;
    budget.work(length, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(result)
}

/// Retained buffers reduce the allowance of every nested operation.
pub(super) fn retaining<T>(
    bytes: u64,
    budget: &mut Budget,
    action: impl FnOnce(&mut Budget) -> Result<T, Fault>,
) -> Result<T, Fault> {
    let previous = budget.reserve_temporary(bytes, AT)?;
    let result = action(budget);
    budget.restore_limits(previous);
    result
}
