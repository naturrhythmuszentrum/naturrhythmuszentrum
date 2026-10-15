// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit syntax and floating-evaluation resource limits.

use crate::ArithmeticFailure;

/// Bounds on literal significand digits, decimal exponents and evaluation work.
///
/// Eight work units per input byte prepay bounded grammar and candidate scans;
/// each enclosure add, multiply, divide or displacement evaluation costs one.
/// Limits of zero are meaningful and refuse any operation that needs that resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralLimits {
    /// Maximum combined significand digit count.
    digits: usize,
    /// Maximum absolute written and effective decimal exponent.
    exponent: u32,
    /// Maximum charged work units.
    work: u64,
}

impl LiteralLimits {
    /// Set all limits explicitly; zero budgets are accepted as refusing policies.
    #[must_use]
    pub const fn new(max_digits: usize, max_exponent: u32, max_work: u64) -> Self {
        Self {
            digits: max_digits,
            exponent: max_exponent,
            work: max_work,
        }
    }
    /// Maximum combined significand digits.
    #[must_use]
    pub const fn max_digits(self) -> usize {
        self.digits
    }
    /// Maximum absolute written and effective decimal exponent.
    #[must_use]
    pub const fn max_exponent(self) -> u32 {
        self.exponent
    }
    /// Maximum charged work.
    #[must_use]
    pub const fn max_work(self) -> u64 {
        self.work
    }
}

impl Default for LiteralLimits {
    fn default() -> Self {
        Self::new(1024, 4096, 100_000)
    }
}

/// A checked counter; every charge precedes its floating work.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Work {
    /// Configured work ceiling.
    limit: u64,
    /// Units charged so far, including prepaid text scanning.
    used: u64,
}

impl Work {
    /// Prepay all bounded scans before looking for separators or parsing a candidate.
    pub(crate) fn new(bytes: usize, limit: u64) -> Result<Self, ArithmeticFailure> {
        let used = u64::try_from(bytes)
            .ok()
            .and_then(|value| value.checked_mul(8))
            .ok_or(ArithmeticFailure::ResourceLimit)?;
        if used > limit {
            return Err(ArithmeticFailure::ResourceLimit);
        }
        Ok(Self { limit, used })
    }

    /// Reserve arithmetic work without wrapping the counter.
    pub(crate) fn charge(&mut self, amount: u64) -> Result<(), ArithmeticFailure> {
        let total = self
            .used
            .checked_add(amount)
            .ok_or(ArithmeticFailure::ResourceLimit)?;
        if total > self.limit {
            return Err(ArithmeticFailure::ResourceLimit);
        }
        self.used = total;
        Ok(())
    }

    /// Total paid work units.
    pub(crate) const fn used(self) -> u64 {
        self.used
    }
}
