// SPDX-License-Identifier: AGPL-3.0-only
//! Checked finite selections and exact ordered affine reading definitions.
//!
//! Shaped values use logical column order. A Complex scalar has the ordered Real
//! coordinates `(real, imaginary)`. Reading definitions are transparent expressions:
//! their first operand retains the declared symbol, and their second operand is the
//! exact source. A bare declaration is never equated to a definition by its name.

mod affine;
pub(crate) mod check;
mod proof;
mod selection;
pub(crate) mod syntax;
pub(crate) mod wire;

pub use affine::AffineMap;
pub use selection::{EntryIndex, EntryOrder, EntrySelection, SelectionSpec};

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::size;

/// Shared location for finite reading-map failures and resource charges.
const AT: Location = Location::new("ordered reading map");

/// Every finite temporary array is bounded before its allocation or traversal.
fn reserve<T>(count: usize, budget: &mut Budget) -> Result<Vec<T>, Fault> {
    let count64 = size(count, AT)?;
    budget.check(BudgetKind::MatrixEntries, count64, AT)?;
    let bytes = count64
        .checked_mul(size(std::mem::size_of::<T>(), AT)?)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(count64, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(result)
}

/// Unsupported extents and arithmetic overflow have a named finite-shape refusal.
const fn invalid(code: InvalidCode) -> Fault {
    Fault::invalid(code, AT)
}
