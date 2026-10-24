// SPDX-License-Identifier: AGPL-3.0-only
//! Closed statistical meanings distinguish original selections, contractions and publication roles.

/// Mathematical role of one private statistical matrix under its original model and law.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticsAllocationRole {
    /// Original selected sensitivity rows, including repeated source selections.
    InputSensitivity,
    /// Explicit selected input means; nominal centers cannot supply this role.
    InputMeans,
    /// Original represented operating point and complete deterministic enclosure.
    OperatingPoint,
    /// Original source covariance used to authenticate structural zeros.
    StructureCovariance,
    /// Original source covariance used to classify a fixed operand.
    FixedCovariance,
    /// Input sensitivities aligned to the common original source-column order.
    Inputs,
    /// Original model covariance in the complete selected source order.
    Model,
    /// Concatenated declared input means under an affine statistical law.
    Means,
    /// Local derivative in original concatenated operand order.
    Jacobian,
    /// Checked affine offsets; these are never inferred input means.
    Offsets,
    /// Local derivative contracted with original input source sensitivities.
    Sensitivity,
    /// Sensitivity contracted with the original model covariance.
    CovarianceProduct,
    /// Complete covariance, or ordered cross-covariance of both original targets.
    Covariance,
    /// Explicit output expectation, permitted only by the retained affine law.
    Mean,
    /// Transposed sensitivity retaining the same original selected source order.
    Transpose,
    /// One standalone cross operand aligned to the common source order.
    Aligned,
    /// Separate covariance parameter copies prepared for atomic publication.
    PublishedCovariance,
    /// Separate source sensitivity parameter copies prepared for atomic publication.
    PublishedSensitivity,
    /// Separate local derivative parameter copies prepared for atomic publication.
    PublishedJacobian,
    /// Separate explicit mean parameter copies prepared for atomic publication.
    PublishedMean,
}
/// Numerical representation of a statistical allocation, independent of its mathematical role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticsAllocationChannel {
    /// Finite candidate scalar coordinates.
    Centers,
    /// Two independent enclosure endpoints per logical coordinate.
    Truth,
    /// Numerical error on the published parameter itself.
    Errors,
    /// Opaque packed left backend operand.
    BackendLeft,
    /// Opaque packed right backend operand.
    BackendRight,
    /// Opaque backend contraction result.
    BackendResult,
}
impl StatisticsAllocationChannel {
    /// Backend logical rectangles do not expose allocator capacities.
    pub(crate) const fn backend(self) -> bool {
        matches!(
            self,
            Self::BackendLeft | Self::BackendRight | Self::BackendResult
        )
    }
    /// Enclosures retain both endpoints without treating their width as a logical shape.
    pub(crate) const fn components(self) -> usize {
        if matches!(self, Self::Truth) { 2 } else { 1 }
    }
}
