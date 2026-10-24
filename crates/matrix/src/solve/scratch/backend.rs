// SPDX-License-Identifier: AGPL-3.0-only
//! Opaque backend owners retain non-cloneable pre-allocation logical request permits.

use super::Permit;

/// The wrapper never exposes replacement or detachment of the original numerical owner.
#[derive(Debug)]
pub(crate) struct Backend<T> {
    /// Actual opaque backend matrix, accessible only by shared borrow.
    pub(super) value: T,
    /// Original pre-allocation authority that cannot be detached or cloned.
    pub(super) permit: Permit,
}
impl<T> Backend<T> {
    /// Borrow the safe backend object while keeping its allocating interpretation attached.
    pub(crate) const fn get(&self) -> &T {
        &self.value
    }
}
impl<T> Drop for Backend<T> {
    fn drop(&mut self) {
        self.permit.release(self.permit.entries());
    }
}

/// Public matrix dimensions authenticate the logical request while opaque capacities remain absent.
pub(super) fn validate<T: 'static>(
    value: &T,
    extent: crate::Extent,
) -> Result<(), crate::OperationError> {
    let value = value as &dyn std::any::Any;
    let shape = if let Some(matrix) = value.downcast_ref::<faer::Mat<f32>>() {
        [matrix.nrows(), matrix.ncols()]
    } else if let Some(matrix) = value.downcast_ref::<faer::Mat<f64>>() {
        [matrix.nrows(), matrix.ncols()]
    } else {
        return Err(crate::PrivateAllocationError::Representation.into());
    };
    if shape != [extent.rows(), extent.columns()] {
        return Err(crate::PrivateAllocationError::IncompleteCoverage.into());
    }
    Ok(())
}
