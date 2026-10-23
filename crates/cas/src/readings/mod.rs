// SPDX-License-Identifier: AGPL-3.0-only
//! Checked finite selections and exact ordered affine reading definitions.
//!
//! Shaped values use logical column order. A Complex scalar has the ordered Real
//! coordinates `(real, imaginary)`. Reading definitions are transparent expressions:
//! their first operand retains the declared symbol, and their second operand is the
//! exact source. A bare declaration is never equated to a definition by its name.

pub(crate) mod affine;
mod bases;
pub(crate) mod check;
mod coverage;
mod determinant;
mod entry_sets;
mod exact_expand;
mod exact_map;
mod inventory;
mod inventory_budget;
mod inventory_build;
mod inventory_model;
mod inventory_walk;
mod inventory_write;
mod inverse;
mod proof;
mod rational_inverse;
mod selection;
mod storage;
pub(crate) mod syntax;
pub(crate) mod wire;

pub use affine::AffineMap;
pub use coverage::{EntryCoverage, EntryMultiplicity};
pub use entry_sets::{EntrySetRelation, SharedEntry};
pub use exact_map::ExactAffineMap;
pub use inventory_model::{
    DependencyBase, DependencyContents, DependencyDeclaration, DependencyDefinition,
    DependencyInventory, DependencyPremise, DependencyPremiseKind, DependencySelection,
};
pub use inverse::CheckedMapInverse;
pub use rational_inverse::AffineInverse;
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
