// SPDX-License-Identifier: AGPL-3.0-only
//! Admitted immutable covariance witnesses share no unchecked dense constructor.

use super::{DenseCovariance, JointError, JointLimits, SourceFactor};
use nrz_scalar::{ArithmeticContext, Enclosure, Scalar};

/// Mathematical covariance representation admitted by its own checked constructor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CovarianceRepresentation<P: Scalar> {
    /// Exact PSD source factor, including singular rectangular cases.
    Factor(SourceFactor<P>),
    /// Symmetric exact or enclosed family with a sufficient PSD certificate.
    Dense(DenseCovariance<P>),
}
impl<P: Scalar> CovarianceRepresentation<P> {
    /// Declared source order.
    #[must_use]
    pub const fn order(&self) -> usize {
        match self {
            Self::Factor(value) => value.rows(),
            Self::Dense(value) => value.order(),
        }
    }
    /// Admission work per source pair, before complete block validation.
    pub(crate) fn terms(&self) -> usize {
        match self {
            Self::Factor(value) => value.columns().max(1),
            Self::Dense(_) => 1,
        }
    }
    /// Sufficient exact zero proof for a declared zero block.
    pub(crate) fn zero(&self, row: usize, column: usize) -> bool {
        match self {
            Self::Factor(value) => value.zero(row, column),
            Self::Dense(value) => value.zero(row, column),
        }
    }
    /// Enclose the mathematical parameter with its retained witness.
    pub(crate) fn entry(
        &self,
        row: usize,
        column: usize,
        context: &ArithmeticContext,
        limits: JointLimits,
    ) -> Result<Enclosure<P>, JointError> {
        match self {
            Self::Factor(value) => value.entry(row, column, context, limits),
            Self::Dense(value) => value.entry(row, column),
        }
    }
}
