// SPDX-License-Identifier: AGPL-3.0-only
//! Evaluation error remains attached to a covariance parameter, never a physical source.

use super::CompanionRef;
use crate::RegionView;
use nrz_scalar::Scalar;

/// Current deterministic numerical error on the named source-covariance parameter region.
///
/// This view cannot satisfy an API requiring a physical-value bound companion.
#[derive(Debug)]
pub struct ParameterErrorView<'h, P: Scalar> {
    /// Independent heap region of outward upper errors.
    pub(crate) region: &'h RegionView<'h, P>,
    /// Statistical parameter owner; never a value handle.
    pub(crate) target: CompanionRef<P>,
    /// Full source-covariance product descriptors in column order.
    pub(crate) descriptors: &'h [nrz_units::EvaluationDescriptor],
}
impl<P: Scalar> ParameterErrorView<'_, P> {
    /// The immutable companion parameter owner; version one for this occupation.
    #[must_use]
    pub const fn target(&self) -> CompanionRef<P> {
        self.target
    }
    /// Error descriptors retain both covariance factors and the evaluation-radius role.
    #[must_use]
    pub const fn descriptors(&self) -> &[nrz_units::EvaluationDescriptor] {
        self.descriptors
    }
}

impl<P: Scalar + 'static> ParameterErrorView<'_, P> {
    /// Shape of the independently stored parameter-error region.
    #[must_use]
    pub const fn extent(&self) -> crate::Extent {
        self.region.extent()
    }
    /// Read an outward numerical error on a covariance parameter.
    ///
    /// # Errors
    /// Refuses out-of-range parameter indices.
    pub fn get(&self, row: usize, column: usize) -> Result<P, crate::HeapError> {
        self.region.get(row, column)
    }
    /// Export parameter errors without transferring any physical assurance.
    ///
    /// # Errors
    /// Refuses packing limits or allocation failure.
    pub fn export(&self) -> Result<Vec<P>, crate::HeapError> {
        self.region.export()
    }
}
/// Dense source-covariance region with independently paired product descriptors.
#[derive(Debug)]
pub struct CovarianceParameterView<'h, P: Scalar> {
    /// Numerical storage is private; no homogeneous ratio schema replaces physical products.
    pub(crate) region: &'h RegionView<'h, P>,
    /// Each entry retains both source-side descriptor factors.
    pub(crate) descriptors: &'h [nrz_units::EvaluationDescriptor],
}
impl<P: Scalar + 'static> CovarianceParameterView<'_, P> {
    /// Shape of the full source covariance.
    #[must_use]
    pub const fn extent(&self) -> crate::Extent {
        self.region.extent()
    }
    /// Read one displayed covariance parameter.
    ///
    /// # Errors
    /// Refuses out-of-range parameter indices.
    pub fn get(&self, row: usize, column: usize) -> Result<P, crate::HeapError> {
        self.region.get(row, column)
    }
    /// Paired product meaning of one covariance entry.
    ///
    /// # Errors
    /// Refuses out-of-range parameter indices.
    pub fn descriptor(
        &self,
        row: usize,
        column: usize,
    ) -> Result<&nrz_units::PairingWitness, crate::HeapError> {
        let index = self.region.extent().index(row, column)?;
        self.descriptors
            .get(index)
            .map(nrz_units::EvaluationDescriptor::target)
            .ok_or(crate::HeapError::InvalidLayout)
    }
    /// Export parameter centers without granting independent-source semantics.
    ///
    /// # Errors
    /// Refuses packing limits or allocation failure.
    pub fn export(&self) -> Result<Vec<P>, crate::HeapError> {
        self.region.export()
    }
}
