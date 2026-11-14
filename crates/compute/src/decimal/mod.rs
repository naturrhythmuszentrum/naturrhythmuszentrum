// SPDX-License-Identifier: AGPL-3.0-only
//! Exact source arithmetic and declared-grid decimal projection.
//!
//! Implements ``alg:nrz:compute-design-decimal-rounding`` with 54 fixed limbs.

mod integer;
mod parse;
mod rational;
mod render;

use integer::Integer;
pub use parse::{decimal_source, validate_fixed, validate_significant};
pub use rational::Exact;
pub use render::{fixed, significant};

/// Refusal of malformed sources or arithmetic beyond the fixed representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DecimalError {
    /// A source is nonfinite or has malformed syntax.
    #[error("invalid exact source")]
    Source,
    /// A checked fixed-capacity integer operation overflowed.
    #[error("exact integer capacity exceeded")]
    Capacity,
    /// A denominator is zero.
    #[error("zero exact denominator")]
    Denominator,
    /// A grid is outside the designed range.
    #[error("invalid decimal grid")]
    Grid,
}

/// Rounding role of one field, independent of serializer behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rounding {
    /// Nearest coefficient; exact ties go away from zero.
    Nearest,
    /// Lower endpoint, toward negative infinity.
    Lower,
    /// Upper endpoint, toward positive infinity.
    Upper,
}

/// Maximum exact JSON count, duration or signed counter magnitude.
pub const MAX_INTEGER: u64 = (1_u64 << 53) - 1;

#[cfg(test)]
mod tests;
