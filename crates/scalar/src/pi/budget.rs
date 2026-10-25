// SPDX-License-Identifier: AGPL-3.0-only
//! Caller-owned cumulative constant work and logical storage allowances.

use super::PiError;

/// Explicit finite limits; zero is a valid refusing policy and there are no defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PiLimits {
    /// Cumulative admitted numerical work units.
    pub work: u64,
    /// Cumulative paired Machin-series refinements.
    pub steps: u64,
    /// Maximum simultaneous logical algorithm state plus returned result bytes.
    pub staged_bytes: usize,
    /// Cumulative logical returned result bytes.
    pub output_bytes: usize,
}

/// Committed usage; every refused request leaves all four fields unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PiUsage {
    /// Numerical work from successful calls only.
    pub work: u64,
    /// Paired series refinements from successful calls only.
    pub steps: u64,
    /// High-water logical algorithm state plus returned result bytes.
    pub staged_bytes: usize,
    /// Sum of the logical result sizes returned by successful calls.
    pub output_bytes: usize,
}

/// Cumulative pi ledger, updated only when a complete enclosure is returned.
///
/// Storage counts fixed logical state and results, not stack frames, compiler
/// temporaries, the caller's ledger/context or allocator overhead. There is no
/// heap allocation and no cache. Dropping a result does not refund output usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PiBudget {
    /// Immutable explicit caller policy.
    limits: PiLimits,
    /// All-or-none admitted usage.
    usage: PiUsage,
}

impl PiBudget {
    /// Create an unused ledger under explicit limits.
    #[must_use]
    pub const fn new(limits: PiLimits) -> Self {
        Self {
            limits,
            usage: PiUsage {
                work: 0,
                steps: 0,
                staged_bytes: 0,
                output_bytes: 0,
            },
        }
    }

    /// Immutable caller policy.
    #[must_use]
    pub const fn limits(&self) -> PiLimits {
        self.limits
    }

    /// Current committed usage.
    #[must_use]
    pub const fn usage(&self) -> PiUsage {
        self.usage
    }

    /// Prepay a fixed bounded arithmetic schedule on the private transaction.
    pub(super) fn work(&mut self, amount: u64) -> Result<(), PiError> {
        let total = self
            .usage
            .work
            .checked_add(amount)
            .ok_or(PiError::WorkLimit)?;
        if total > self.limits.work {
            return Err(PiError::WorkLimit);
        }
        self.usage.work = total;
        Ok(())
    }

    /// Reserve one paired series refinement before touching either series.
    pub(super) fn step(&mut self) -> Result<(), PiError> {
        let total = self.usage.steps.checked_add(1).ok_or(PiError::StepLimit)?;
        if total > self.limits.steps {
            return Err(PiError::StepLimit);
        }
        self.usage.steps = total;
        self.work(26)
    }

    /// Reserve the fixed logical scratch and result before numerical evaluation.
    pub(super) fn storage(&mut self, state: usize, output: usize) -> Result<(), PiError> {
        let staged = state.checked_add(output).ok_or(PiError::StagedBytesLimit)?;
        if staged > self.limits.staged_bytes {
            return Err(PiError::StagedBytesLimit);
        }
        let total = self
            .usage
            .output_bytes
            .checked_add(output)
            .ok_or(PiError::OutputBytesLimit)?;
        if total > self.limits.output_bytes {
            return Err(PiError::OutputBytesLimit);
        }
        self.usage.staged_bytes = self.usage.staged_bytes.max(staged);
        self.usage.output_bytes = total;
        Ok(())
    }
}
