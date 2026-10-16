// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit bounds for a copy execution's input scans and independent scratch.

use super::OperationError;

/// Per-execution policy, in logical scalar entries including carried companions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    /// Maximum operation-ordered inputs and outputs.
    slots: usize,
    /// Maximum scalar entries inspected in one complete scan or copy phase.
    pub(crate) work: usize,
    /// Maximum scalar entries in each complete private payload set.
    pub(crate) scratch: usize,
}

impl ExecutionLimits {
    /// Set explicit limits; zero admits empty payloads but no nonempty slot table.
    #[must_use]
    pub const fn new(slots: usize, work: usize, scratch: usize) -> Self {
        Self {
            slots,
            work,
            scratch,
        }
    }

    /// Refuse before scanning entries or allocating numerical scratch.
    pub(crate) const fn check(self, slots: usize, entries: usize) -> Result<(), OperationError> {
        if slots > self.slots || entries > self.work || entries > self.scratch {
            Err(OperationError::ResourceLimit)
        } else {
            Ok(())
        }
    }
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self::new(4096, 1_048_576, 1_048_576)
    }
}
