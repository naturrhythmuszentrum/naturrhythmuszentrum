// SPDX-License-Identifier: AGPL-3.0-only
//! Physical question metadata is bounded before traversal or result publication.

use super::{DecisionError, LatticeDecisionSpec, LatticeIndexKind};
use crate::{Kind, decision::storage};
use nrz_cas::limits::{Budget, BudgetKind};

/// Check ordered dimensions and prepay only newly owned names, vectors and index descriptors.
pub(super) fn specification(
    value: &LatticeDecisionSpec,
    budget: &mut Budget,
) -> Result<u64, DecisionError> {
    let dimension = u64::from(value.basis.dimension());
    let count = usize::try_from(dimension).map_err(|_| DecisionError::Capacity)?;
    budget.work(5, storage::AT)?;
    if count == 0
        || value.axes.len() != count
        || value.indices.len() != count
        || value.frame.entries().len() != count
        || value.index.question().families.len() != count
    {
        return Err(DecisionError::LatticeShape);
    }
    let entries = dimension
        .checked_mul(dimension)
        .ok_or(DecisionError::Capacity)?;
    budget.check(BudgetKind::MatrixEntries, entries, storage::AT)?;
    let mut bytes = entries
        .checked_mul(64)
        .and_then(|size| size.checked_add(dimension.saturating_mul(256)))
        .and_then(|size| size.checked_add(512))
        .ok_or(DecisionError::Capacity)?;
    for (row, axis) in value.axes.iter().enumerate() {
        budget.work(1, storage::AT)?;
        if axis.basis.len() != count || axis.name.trim().is_empty() {
            return Err(DecisionError::LatticeShape);
        }
        let name = length(&axis.name)?;
        budget.work(
            name.saturating_mul(u64::try_from(row + 1).map_err(|_| DecisionError::Capacity)?),
            storage::AT,
        )?;
        if value.axes[..row]
            .iter()
            .any(|earlier| earlier.name == axis.name)
        {
            return Err(DecisionError::LatticeShape);
        }
        bytes = bytes.checked_add(name).ok_or(DecisionError::Capacity)?;
    }
    for kind in &value.indices {
        let name = length(kind.name())?;
        budget.work(name.saturating_add(1), storage::AT)?;
        if kind.name().trim().is_empty() {
            return Err(DecisionError::LatticeIndexKind);
        }
        if let LatticeIndexKind::Count(descriptor) = kind {
            let Kind::Count(_, domain) = descriptor.kind() else {
                return Err(DecisionError::LatticeIndexKind);
            };
            if !domain.is_nonnegative() {
                return Err(DecisionError::LatticeIndexKind);
            }
            bytes = bytes.checked_add(2_048).ok_or(DecisionError::Capacity)?;
            bytes = bytes
                .checked_add(length(descriptor.measure().name())?)
                .ok_or(DecisionError::Capacity)?;
        }
        bytes = bytes.checked_add(name).ok_or(DecisionError::Capacity)?;
    }
    storage::retained(bytes, budget)?;
    budget.work(bytes, storage::AT)?;
    Ok(bytes)
}

/// Checked metadata lengths never wrap or acquire an unbounded comparison allowance.
pub(super) fn length(value: &str) -> Result<u64, DecisionError> {
    u64::try_from(value.len()).map_err(|_| DecisionError::Capacity)
}
