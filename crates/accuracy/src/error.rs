// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit refusals of bound admission and finite bound arithmetic.

use crate::ResourceLimit;
use core::fmt;
use nrz_scalar::ArithmeticFailure;

/// A failed numerical policy or bound operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccuracyFailure {
    /// A bound entry is negative or nonfinite.
    InvalidBound,
    /// A contraction's two bound vectors have different lengths.
    LengthMismatch,
    /// Tolerances or the positive reference scale fail their numerical domain.
    InvalidBudget,
    /// A refinement policy has an invalid stagnation window.
    InvalidPolicy,
    /// An operation does not match the current recovery phase or retry permission.
    InvalidRecoveryState,
    /// A cumulative attempt or work allowance was exhausted.
    ResourceLimit(ResourceLimit),
    /// The scalar substrate could not enclose an operation.
    Arithmetic(ArithmeticFailure),
}

impl fmt::Display for AccuracyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBound => formatter.write_str("a bound must be finite and nonnegative"),
            Self::LengthMismatch => formatter.write_str("bound vector lengths differ"),
            Self::InvalidBudget => formatter.write_str("invalid tolerance or reference scale"),
            Self::InvalidPolicy => formatter.write_str("the stagnation window must be positive"),
            Self::InvalidRecoveryState => formatter.write_str("invalid recovery transition"),
            Self::ResourceLimit(limit) => write!(formatter, "numerical resource limit: {limit}"),
            Self::Arithmetic(cause) => write!(formatter, "bound arithmetic failed: {cause}"),
        }
    }
}

impl std::error::Error for AccuracyFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Arithmetic(cause) => Some(cause),
            Self::ResourceLimit(limit) => Some(limit),
            Self::InvalidBound
            | Self::LengthMismatch
            | Self::InvalidBudget
            | Self::InvalidPolicy
            | Self::InvalidRecoveryState => None,
        }
    }
}

impl From<ArithmeticFailure> for AccuracyFailure {
    fn from(cause: ArithmeticFailure) -> Self {
        Self::Arithmetic(cause)
    }
}

impl From<ResourceLimit> for AccuracyFailure {
    fn from(limit: ResourceLimit) -> Self {
        Self::ResourceLimit(limit)
    }
}
