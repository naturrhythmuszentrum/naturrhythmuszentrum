// SPDX-License-Identifier: AGPL-3.0-only
//! Complete coverage and duplicate coverage are distinct exact judgments.

use super::{AT, EntryIndex, bases, invalid, reserve, selection::rectangle};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, size};

/// An entry covered by more than one member of the supplied selection list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryMultiplicity {
    /// Original base entry.
    pub entry: EntryIndex,
    /// Number of selection-list members containing this entry, including repeated members.
    pub count: u64,
}

/// Coverage defects in logical column order; an empty base is covered by an empty list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryCoverage {
    /// Base entries absent from every selection.
    pub uncovered: Vec<EntryIndex>,
    /// Base entries present in at least two selection-list members.
    pub multiple: Vec<EntryMultiplicity>,
}

impl EntryCoverage {
    /// Whether every base entry occurs in at least one selection.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.uncovered.is_empty()
    }

    /// Whether every base entry occurs in exactly one selection.
    #[must_use]
    pub const fn exactly_once(&self) -> bool {
        self.uncovered.is_empty() && self.multiple.is_empty()
    }
}

impl Cas {
    /// Decide total coverage and report holes and multiplicities over an explicit base.
    /// # Errors
    /// Refuses foreign handles, nonselections, mixed symbols, changed declarations or
    /// definitions, nonliteral base rectangles and exhausted caller resources.
    pub fn entry_coverage(
        &self,
        base: &Handle,
        selections: &[Handle],
        budget: &mut Budget,
    ) -> Result<EntryCoverage, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.cover_entries(base, selections, budget);
        budget.restore_limits(previous);
        result
    }

    /// Validate every list member even when the base has no entries.
    fn cover_entries(
        &self,
        base: &Handle,
        selections: &[Handle],
        budget: &mut Budget,
    ) -> Result<EntryCoverage, Fault> {
        budget.work(1, AT)?;
        let base = self.checked(base)?;
        bases::declaration(self, base)?;
        let (_, rows, columns) = rectangle(&self.state.expression(base)?.sort)?;
        let count = super::affine::length(rows, columns)?;
        let bytes = size(count, AT)?.saturating_mul(32);
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(
            BudgetKind::OutputBytes,
            size(count, AT)?.saturating_mul(24),
            AT,
        )?;
        let mut counts = reserve(count, budget)?;
        counts.resize(count, 0_u64);
        for selected in selections {
            budget.work(1, AT)?;
            let (source, selection) = bases::selection(self, selected, budget)?;
            bases::same(self, base, source, budget)?;
            budget.work(size(selection.indices().len(), AT)?, AT)?;
            for entry in selection.indices() {
                let position = u64::from(entry.column) * u64::from(rows) + u64::from(entry.row);
                let position =
                    usize::try_from(position).map_err(|_| invalid(InvalidCode::SelectionShape))?;
                let slot = counts
                    .get_mut(position)
                    .ok_or_else(|| invalid(InvalidCode::SelectionOutOfRange))?;
                *slot = slot
                    .checked_add(1)
                    .ok_or_else(|| invalid(InvalidCode::SelectionShape))?;
            }
        }
        coverage(rows, &counts, budget)
    }
}

/// Reserve both result lists together with the still-live count array before expansion.
fn coverage(rows: u32, counts: &[u64], budget: &mut Budget) -> Result<EntryCoverage, Fault> {
    let mut uncovered = reserve(counts.len(), budget)?;
    let mut multiple = reserve(counts.len(), budget)?;
    budget.work(size(counts.len(), AT)?, AT)?;
    for (position, count) in counts.iter().copied().enumerate() {
        let position = size(position, AT)?;
        let row = position
            .checked_rem(u64::from(rows))
            .ok_or_else(|| invalid(InvalidCode::SelectionShape))?;
        let column = position
            .checked_div(u64::from(rows))
            .ok_or_else(|| invalid(InvalidCode::SelectionShape))?;
        let entry = EntryIndex {
            row: u32::try_from(row).map_err(|_| invalid(InvalidCode::SelectionShape))?,
            column: u32::try_from(column).map_err(|_| invalid(InvalidCode::SelectionShape))?,
        };
        if count == 0 {
            uncovered.push(entry);
        } else if count > 1 {
            multiple.push(EntryMultiplicity { entry, count });
        }
    }
    Ok(EntryCoverage {
        uncovered,
        multiple,
    })
}
