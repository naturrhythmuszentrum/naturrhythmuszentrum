// SPDX-License-Identifier: AGPL-3.0-only
//! Entry-set judgments keep ordered reading identity separate from set equality.

use super::{AT, EntryIndex, bases, invalid, reserve};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, size};

/// One shared entry and its zero-based position in each original selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedEntry {
    /// Original base entry.
    pub entry: EntryIndex,
    /// Position in the left selection.
    pub left_position: u32,
    /// Position in the right selection.
    pub right_position: u32,
}

/// Exact relations over one base, with intersection in the left selection's order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntrySetRelation {
    /// Shared entries retaining both original positions.
    pub intersection: Vec<SharedEntry>,
    /// Number of distinct entries on the left.
    left_count: usize,
    /// Number of distinct entries on the right.
    right_count: usize,
    /// Equality of the complete ordered entry sequences, independently of result shape.
    same_order: bool,
}

impl EntrySetRelation {
    /// Whether every right entry occurs on the left.
    #[must_use]
    pub const fn left_contains_right(&self) -> bool {
        self.intersection.len() == self.right_count
    }

    /// Whether every left entry occurs on the right.
    #[must_use]
    pub const fn right_contains_left(&self) -> bool {
        self.intersection.len() == self.left_count
    }

    /// Whether both operands have the same entries, independently of order and shape.
    #[must_use]
    pub const fn equal_sets(&self) -> bool {
        self.left_contains_right() && self.right_contains_left()
    }

    /// Whether the operands have no shared entry.
    #[must_use]
    pub const fn disjoint(&self) -> bool {
        self.intersection.is_empty()
    }

    /// Whether the original ordered entry sequences agree.
    #[must_use]
    pub const fn same_order(&self) -> bool {
        self.same_order
    }
}

impl Cas {
    /// Decide intersection, both containments, set equality, disjointness and order.
    ///
    /// Intersection follows the left operand's order and names both operand positions.
    /// No equality of reading maps follows from equality of entry sets.
    /// # Errors
    /// Refuses foreign or nonselection handles, different symbols, changed declaration
    /// snapshots, different definitions of one symbol and exhausted caller resources.
    pub fn entry_set_relation(
        &self,
        left: &Handle,
        right: &Handle,
        budget: &mut Budget,
    ) -> Result<EntrySetRelation, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.compare_entries(left, right, budget);
        budget.restore_limits(previous);
        result
    }

    /// Prepay comparisons and the bounded result before traversing either index list.
    fn compare_entries(
        &self,
        left: &Handle,
        right: &Handle,
        budget: &mut Budget,
    ) -> Result<EntrySetRelation, Fault> {
        let (first_base, first) = bases::selection(self, left, budget)?;
        let (second_base, second) = bases::selection(self, right, budget)?;
        bases::same(self, first_base, second_base, budget)?;
        let first = first.indices();
        let second = second.indices();
        let count = first.len().min(second.len());
        let bytes = size(count, AT)?.saturating_mul(size(std::mem::size_of::<SharedEntry>(), AT)?);
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.work(
            size(first.len(), AT)?.saturating_mul(size(second.len(), AT)?),
            AT,
        )?;
        budget.work(size(first.len().max(second.len()), AT)?, AT)?;
        let mut intersection = reserve(count, budget)?;
        for (left_position, entry) in first.iter().enumerate() {
            if let Some(right_position) = second.iter().position(|other| other == entry) {
                intersection.push(SharedEntry {
                    entry: *entry,
                    left_position: u32::try_from(left_position)
                        .map_err(|_| invalid(InvalidCode::SelectionShape))?,
                    right_position: u32::try_from(right_position)
                        .map_err(|_| invalid(InvalidCode::SelectionShape))?,
                });
            }
        }
        Ok(EntrySetRelation {
            intersection,
            left_count: first.len(),
            right_count: second.len(),
            same_order: first == second,
        })
    }
}
