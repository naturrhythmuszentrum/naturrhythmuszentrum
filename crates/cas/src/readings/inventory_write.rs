// SPDX-License-Identifier: AGPL-3.0-only
//! Exact writes intersect only the inventory's authenticated ordered base-entry unions.

use super::inventory_model::DependencyInventory;
use super::{AT, EntryIndex, bases, invalid, reserve};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{AtomicSort, Operator, Sort};

impl Cas {
    /// Return touched base entries for an admitted write selection, in logical column order.
    ///
    /// A distinct declared base is disjoint. A reused symbol with a changed snapshot or
    /// definition refuses. This judgment carries no heap versions or fallback policy.
    /// # Errors
    /// Refuses foreign inventories or handles, nonselections, changed bases and exhausted
    /// cumulative work or result payload resources.
    pub fn dependency_selection_intersection(
        &self,
        inventory: &DependencyInventory,
        write: &Handle,
        budget: &mut Budget,
    ) -> Result<Vec<EntryIndex>, Fault> {
        self.inventory_limits(budget, |budget| {
            self.checked(&inventory.owner)?;
            let (base, selected) = bases::selection(self, write, budget)?;
            self.intersect_dependencies(inventory, base, selected.indices(), budget)
        })
    }

    /// Return touched entries for a distinct, in-range write list over one exact base.
    ///
    /// The input order is unrestricted; the result is in logical column order. Empty
    /// writes are valid. A scalar has the single entry `(0,0)`, including Complex scalars.
    /// # Errors
    /// Refuses foreign inventories or bases, duplicates, out-of-range entries, nonliteral
    /// rectangles, changed declarations or definitions, and exhausted caller resources.
    pub fn dependency_entry_intersection(
        &self,
        inventory: &DependencyInventory,
        base: &Handle,
        entries: &[EntryIndex],
        budget: &mut Budget,
    ) -> Result<Vec<EntryIndex>, Fault> {
        self.inventory_limits(budget, |budget| {
            self.checked(&inventory.owner)?;
            let base = self.checked(base)?;
            bases::declaration(self, base)?;
            validate_entries(&self.state.expression(base)?.sort, entries, budget)?;
            self.intersect_dependencies(inventory, base, entries, budget)
        })
    }

    /// Matching uses authenticated base identity; public inventory data cannot supply authority.
    fn intersect_dependencies(
        &self,
        inventory: &DependencyInventory,
        base: LogicalId,
        entries: &[EntryIndex],
        budget: &mut Budget,
    ) -> Result<Vec<EntryIndex>, Fault> {
        budget.work(size(inventory.bases.len(), AT)?, AT)?;
        let Some(position) = inventory
            .bases
            .iter()
            .position(|original| *original == base)
        else {
            self.reject_changed_base(inventory, base, budget)?;
            return Ok(Vec::new());
        };
        let reads = &inventory
            .contents
            .bases
            .get(position)
            .ok_or_else(|| invalid(InvalidCode::Reference))?
            .entries;
        budget.work(
            size(reads.len(), AT)?.saturating_mul(size(entries.len(), AT)?),
            AT,
        )?;
        let count = reads.len().min(entries.len());
        budget.check(
            BudgetKind::OutputBytes,
            size(count, AT)?.saturating_mul(8),
            AT,
        )?;
        let mut touched = reserve(count, budget)?;
        for entry in reads {
            if entries.contains(entry) {
                touched.push(*entry);
            }
        }
        Ok(touched)
    }

    /// Same-key snapshots cannot masquerade as unrelated disjoint bases.
    fn reject_changed_base(
        &self,
        inventory: &DependencyInventory,
        base: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let declared = bases::declaration(self, base)?;
        let Operator::Free(key) = &declared.operator else {
            return Err(invalid(InvalidCode::ReadingDefinition));
        };
        for original in &inventory.bases {
            budget.work(1, AT)?;
            let other = bases::declaration(self, *original)?;
            let Operator::Free(other_key) = &other.operator else {
                return Err(invalid(InvalidCode::ReadingDefinition));
            };
            budget.work(size(key.len().max(other_key.len()), AT)?, AT)?;
            if key == other_key {
                return bases::same(self, *original, base, budget);
            }
        }
        Ok(())
    }
}

/// Validate untrusted writes even when the inventory has no matching base or demanded entries.
fn validate_entries(
    source: &Sort,
    entries: &[EntryIndex],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (rows, columns) = if matches!(source, Sort::Value(AtomicSort::Scalar(_))) {
        (1, 1)
    } else {
        let (_, rows, columns) = super::selection::rectangle(source)?;
        (rows, columns)
    };
    let count = size(entries.len(), AT)?;
    budget.check(BudgetKind::MatrixEntries, count, AT)?;
    budget.work(count.saturating_mul(count.saturating_add(1)), AT)?;
    for (position, entry) in entries.iter().enumerate() {
        if entry.row >= rows || entry.column >= columns {
            return Err(invalid(InvalidCode::SelectionOutOfRange));
        }
        if entries
            .get(..position)
            .is_some_and(|prior| prior.contains(entry))
        {
            return Err(invalid(InvalidCode::SelectionDuplicate));
        }
    }
    Ok(())
}
