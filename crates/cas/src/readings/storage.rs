// SPDX-License-Identifier: AGPL-3.0-only
//! Simultaneous map buffers reserve capacity while nested graph operations use their own scratch.

use super::AT;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::size;

/// Reserve conservative handle, draft and index storage before growing another map buffer.
pub(super) fn slots<T>(
    count: usize,
    budget: &mut Budget,
    action: impl FnOnce(&mut Budget) -> Result<T, Fault>,
) -> Result<T, Fault> {
    let previous = budget.reserve_temporary(size(count, AT)?.saturating_mul(128), AT)?;
    let result = action(budget);
    budget.restore_limits(previous);
    result
}
