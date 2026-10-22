// SPDX-License-Identifier: AGPL-3.0-only
//! Checked cumulative limits shared across execution and precision retry.
//!
//! # Cumulative reservation contract
//!
//! `Limits` sets finite cumulative attempt and work allowances; zero means no allowance, with no unlimited sentinel. Its `WorkBudget` reserves an attempt and its known work atomically before execution and charges additional work before an operation. Failed reservations change neither counter; successful reservations are never refunded after failure. Checked integer addition prevents wraparound. The caller defines a reproducible work unit and includes numerical verification in its charges.

use core::fmt;

/// The resource whose finite limit prevented further work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    /// No further candidate or refinement attempt can be reserved.
    Attempts,
    /// The requested work would exceed the cumulative work limit.
    Work,
}

impl fmt::Display for ResourceLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Attempts => "attempt limit exhausted",
            Self::Work => "work limit exhausted",
        })
    }
}

impl std::error::Error for ResourceLimit {}

/// Finite cumulative attempt and work allowances; zero is a valid refusing policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum total number of candidate and refinement attempts.
    max_attempts: u32,
    /// Maximum charged work in the caller's documented unit.
    max_work: u64,
}

impl Limits {
    /// Set finite cumulative limits without an implicit unlimited sentinel.
    #[must_use]
    pub const fn new(max_attempts: u32, max_work: u64) -> Self {
        Self {
            max_attempts,
            max_work,
        }
    }

    /// Maximum number of attempts, including initial and promoted executions.
    #[must_use]
    pub const fn max_attempts(self) -> u32 {
        self.max_attempts
    }

    /// Maximum cumulative work in the caller's documented unit.
    #[must_use]
    pub const fn max_work(self) -> u64 {
        self.max_work
    }

    /// Start empty counters for this allowance.
    #[must_use]
    pub const fn start(self) -> WorkBudget {
        WorkBudget {
            limits: self,
            attempts: 0,
            work: 0,
        }
    }
}

/// Cumulative counters, charged before work; failed reservations change nothing.
///
/// Work units are defined by the numerical caller and must cover all work it
/// performs, including verification. Reservations are never refunded on failure.
#[derive(Debug)]
pub struct WorkBudget {
    /// Finite allowances captured at construction.
    limits: Limits,
    /// Total reserved attempts, including failed attempts.
    attempts: u32,
    /// Total reserved work, including failed operations.
    work: u64,
}

impl WorkBudget {
    /// Limits captured when these counters were created.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// Total reserved attempts.
    #[must_use]
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Total charged work.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }

    /// Reserve one attempt and its known work atomically; return its one-based ordinal.
    ///
    /// # Errors
    /// Refuses exhausted attempts, excessive work or counter overflow without
    /// changing either counter. Arithmetic overflow is resource exhaustion.
    pub fn reserve_attempt(&mut self, work: u64) -> Result<u32, ResourceLimit> {
        let attempts = self
            .attempts
            .checked_add(1)
            .filter(|&value| value <= self.limits.max_attempts)
            .ok_or(ResourceLimit::Attempts)?;
        let total = self.checked_work(work)?;
        self.attempts = attempts;
        self.work = total;
        Ok(attempts)
    }

    /// Reserve additional work before a bounded operation.
    ///
    /// # Errors
    /// Refuses excessive work or overflow without changing the counter.
    pub fn charge(&mut self, work: u64) -> Result<(), ResourceLimit> {
        self.work = self.checked_work(work)?;
        Ok(())
    }

    /// Check addition and the finite work allowance without mutating counters.
    fn checked_work(&self, work: u64) -> Result<u64, ResourceLimit> {
        self.work
            .checked_add(work)
            .filter(|&value| value <= self.limits.max_work)
            .ok_or(ResourceLimit::Work)
    }
}

#[cfg(test)]
mod tests {
    use super::{Limits, ResourceLimit, WorkBudget};

    /// An inaccessible exhausted counter cannot wrap into an available attempt.
    /// ´test:unit:attempt-counter-does-not-wrap´
    #[test]
    fn attempt_counter_does_not_wrap() {
        let mut work = WorkBudget {
            limits: Limits::new(u32::MAX, 10),
            attempts: u32::MAX,
            work: 3,
        };
        assert_eq!(work.reserve_attempt(1), Err(ResourceLimit::Attempts));
        assert_eq!((work.attempts(), work.work()), (u32::MAX, 3));
    }
}
