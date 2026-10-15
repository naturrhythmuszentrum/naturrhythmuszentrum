// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit admission and contraction limits.

use super::JointError;

/// Per-operation source, stored-entry and scalar-work capacities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JointLimits {
    /// Maximum ordered source count.
    pub(crate) sources: usize,
    /// Maximum stored entries in each admitted representation.
    pub(crate) entries: usize,
    /// Maximum counted scalar terms or metadata comparisons.
    pub(crate) work: usize,
}

impl JointLimits {
    /// Set explicit capacities, including zero refusal policies.
    #[must_use]
    pub const fn new(sources: usize, entries: usize, work: usize) -> Self {
        Self {
            sources,
            entries,
            work,
        }
    }
    /// Check dimensions and multiplication before allocating storage.
    pub(crate) fn matrix(self, rows: usize, columns: usize) -> Result<usize, JointError> {
        let entries = rows.checked_mul(columns).ok_or(JointError::ResourceLimit)?;
        if rows > self.sources || entries > self.entries {
            return Err(JointError::ResourceLimit);
        }
        Ok(entries)
    }
    /// Admit a fixed work count before numerical loops.
    pub(crate) const fn check_work(self, work: usize) -> Result<(), JointError> {
        if work > self.work {
            Err(JointError::ResourceLimit)
        } else {
            Ok(())
        }
    }
}
impl Default for JointLimits {
    fn default() -> Self {
        Self::new(1024, 1_048_576, 16_777_216)
    }
}
