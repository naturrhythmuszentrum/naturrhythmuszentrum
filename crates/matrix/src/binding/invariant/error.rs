// SPDX-License-Identifier: AGPL-3.0-only
//! Materialization refusals preserve every independently completed operation proof stage.

use crate::{Failure, HeapError, OperationError};
use nrz_scalar::Scalar;

/// Admission refuses without an attempt; execution refuses with its complete failure record.
#[derive(Debug, thiserror::Error)]
pub enum InvariantError<P: Scalar> {
    /// No reciprocal execution began; the original admission or resource cause remains available.
    #[error(transparent)]
    Admission(#[from] OperationError),
    /// The refused execution retains all completed proof stages and the original checked recipe.
    #[error(transparent)]
    Failure(#[from] Failure<P>),
}

impl<P: Scalar> From<HeapError> for InvariantError<P> {
    fn from(cause: HeapError) -> Self {
        Self::Admission(cause.into())
    }
}

impl<P: Scalar> From<nrz_bindings::readings::algebra::Fault> for InvariantError<P> {
    fn from(cause: nrz_bindings::readings::algebra::Fault) -> Self {
        Self::Admission(cause.into())
    }
}
