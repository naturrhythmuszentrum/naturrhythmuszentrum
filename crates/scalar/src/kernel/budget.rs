// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit monotone Work for fixed-size kernel certificate construction and export.

use super::KernelError;

/// Cumulative charges and successful fixed-size records; refusal never refunds Work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KernelUsage {
    /// Prepaid bounded numerical, encoding and copy schedules.
    pub work: u64,
    /// Complete numerical evaluations with a mathematical record.
    pub constructed: u64,
    /// Complete mathematical records exported to the caller.
    pub exported: u64,
}

/// Explicit caller allowance with no default or hidden reset.
#[derive(Debug)]
pub struct KernelBudget {
    /// Inclusive cumulative Work ceiling.
    limit: u64,
    /// Monotone charges and successful results.
    usage: KernelUsage,
}

impl KernelBudget {
    /// Start an unused ledger; zero is a valid refusing allowance.
    #[must_use]
    pub const fn new(limit: u64) -> Self {
        Self {
            limit,
            usage: KernelUsage {
                work: 0,
                constructed: 0,
                exported: 0,
            },
        }
    }

    /// Current cumulative usage, including Work spent by refused operations.
    #[must_use]
    pub const fn usage(&self) -> KernelUsage {
        self.usage
    }

    /// Immutable inclusive cumulative Work ceiling.
    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    /// Prepay a bounded schedule, including work belonging to the caller.
    /// # Errors
    /// Refuses an exhausted ceiling without changing prior usage.
    pub fn charge(&mut self, amount: u64) -> Result<(), KernelError> {
        let total = self
            .usage
            .work
            .checked_add(amount)
            .ok_or(KernelError::WorkLimit)?;
        if total > self.limit {
            return Err(KernelError::WorkLimit);
        }
        self.usage.work = total;
        Ok(())
    }

    /// Record a completed construction after all fallible operations.
    pub(super) const fn constructed(&mut self) {
        // Every construction prepays at least one unit, bounding this count by Work.
        self.usage.constructed += 1;
    }

    /// Record an exported certificate after its fixed copy has been prepaid.
    pub(super) const fn exported(&mut self) {
        // Every export prepays at least one unit, bounding this count by Work.
        self.usage.exported += 1;
    }
}
