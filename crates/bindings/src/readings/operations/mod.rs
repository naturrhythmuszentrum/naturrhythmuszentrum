// SPDX-License-Identifier: AGPL-3.0-only
//! Closed exact operation relations retain ordered operands and physical coordinate contracts.

mod affine;
mod operator;
mod product;

pub use affine::AffineOperation;
pub use operator::{LinearOperator, OperatorCoefficient};
pub use product::OperatorApplication;

/// Physical operation refusal, separate from numerical kernel admission.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ReadingOperationError {
    /// A required exact operation-domain condition is not discharged.
    #[error("operation domain remains unproved")]
    Unproved,
    /// Ordered axes differ, even if dimensions agree.
    #[error("operation ordered axis mismatch")]
    Axis,
    /// Original physical input coordinates do not match the declared domain.
    #[error("operation physical input mismatch")]
    Input,
    /// A proposed result or equation names another actual expression.
    #[error("operation result identity mismatch")]
    Result,
    /// A multiplicative operation received an affine point coordinate.
    #[error("linear operation requires difference coordinates")]
    Point,
    /// Formal physical quotient construction refused.
    #[error("operation pairing: {0}")]
    Pairing(#[from] crate::PairingError),
    /// Original reading or shape admission refused.
    #[error("operation reading: {0}")]
    Reading(#[from] super::ReadingError),
    /// Exact expression, proof or finite resource admission refused.
    #[error("operation algebra: {0}")]
    Algebra(#[from] nrz_cas::Fault),
}
