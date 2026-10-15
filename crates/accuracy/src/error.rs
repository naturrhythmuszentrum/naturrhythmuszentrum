// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit refusals of bound admission and finite bound arithmetic.

use core::fmt;
use nrz_scalar::ArithmeticFailure;

/// A failed numerical policy or bound operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccuracyFailure {
    /// A bound entry is negative or nonfinite.
    InvalidBound,
    /// A contraction's two bound vectors have different lengths.
    LengthMismatch,
    /// The scalar substrate could not enclose an operation.
    Arithmetic(ArithmeticFailure),
}

impl fmt::Display for AccuracyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBound => formatter.write_str("a bound must be finite and nonnegative"),
            Self::LengthMismatch => formatter.write_str("bound vector lengths differ"),
            Self::Arithmetic(cause) => write!(formatter, "bound arithmetic failed: {cause}"),
        }
    }
}

impl std::error::Error for AccuracyFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Arithmetic(cause) => Some(cause),
            Self::InvalidBound | Self::LengthMismatch => None,
        }
    }
}

impl From<ArithmeticFailure> for AccuracyFailure {
    fn from(cause: ArithmeticFailure) -> Self {
        Self::Arithmetic(cause)
    }
}
