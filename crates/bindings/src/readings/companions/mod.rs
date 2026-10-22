// SPDX-License-Identifier: AGPL-3.0-only
//! Bound, error and covariance symbols retain exact targets and closed reading formulas.

mod evaluation;
mod parameters;
mod relation;
mod value;

pub use evaluation::ParameterErrorReading;
pub use parameters::{CompanionParameters, CompanionReadingKind};
pub use relation::CompanionRelation;
pub use value::CompanionReading;

/// Refusals specific to physical companion declarations and transports.
#[derive(Clone, Debug, thiserror::Error)]
pub enum CompanionReadingError {
    /// A bound/error or ordered covariance target was substituted.
    #[error("companion reading target mismatch")]
    Target,
    /// A source or output parameter has the wrong difference/product unit.
    #[error("companion reading unit mismatch")]
    Unit,
    /// Distinct bound, numerical-error and covariance interpretations cannot substitute.
    #[error("companion reading kind mismatch")]
    Kind,
    /// Physical reading or exact graph admission refused.
    #[error("companion reading: {0}")]
    Reading(#[from] super::ReadingError),
    /// Scalar pairing construction refused.
    #[error("companion pairing: {0}")]
    Pairing(#[from] crate::PairingError),
    /// CAS checking or resource limits refused.
    #[error("companion algebra: {0}")]
    Algebra(#[from] nrz_cas::Fault),
}
