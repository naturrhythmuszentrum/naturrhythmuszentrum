// SPDX-License-Identifier: AGPL-3.0-only
//! Backend owners keep their original typed permits even though allocator capacities are opaque.

use super::{
    Channel, Role, Run,
    plan::{Permit, State},
};
use crate::{OperationError, PrivateAllocationError as Error};
use std::any::{Any, TypeId};

/// One opaque backend rectangle whose extent and scalar type were planned before allocation.
#[derive(Debug)]
pub(crate) struct Backend<T> {
    /// Actual backend allocation owner.
    value: T,
    /// Its original non-cloneable allocation authority.
    permit: Permit,
}
impl<T> Backend<T> {
    /// Borrowing the backend never transfers or duplicates its permit.
    pub(crate) const fn get(&self) -> &T {
        &self.value
    }
}
impl<T> Drop for Backend<T> {
    fn drop(&mut self) {
        self.permit.release(self.permit.entries());
    }
}
impl Run {
    /// Check concrete type and logical rectangle before issuing an opaque backend allocation.
    pub(crate) fn backend<T: 'static>(
        &self,
        role: Role,
        index: usize,
        channel: Channel,
        allocate: impl FnOnce() -> Result<T, OperationError>,
    ) -> Result<Backend<T>, OperationError> {
        let slot = self.slot(role, index, channel)?;
        let entry = &self.0.specifications[slot];
        let kind = match entry.precision {
            nrz_scalar::Precision::F32 => TypeId::of::<faer::Mat<f32>>(),
            nrz_scalar::Precision::F64 => TypeId::of::<faer::Mat<f64>>(),
        };
        if !channel.backend() || TypeId::of::<T>() != kind {
            return Err(Error::Representation.into());
        }
        let permit = self.grant(slot)?;
        let value = allocate()?;
        let dimensions = shape(&value).ok_or(Error::Representation)?;
        if dimensions != (entry.extent.rows(), entry.extent.columns()) {
            return Err(Error::IncompleteCoverage.into());
        }
        entry.state.set(State::Live);
        Ok(Backend { value, permit })
    }
}
/// Public faer dimensions authenticate a request without pretending to expose hidden capacities.
fn shape(value: &dyn Any) -> Option<(usize, usize)> {
    value.downcast_ref::<faer::Mat<f32>>().map_or_else(
        || {
            value
                .downcast_ref::<faer::Mat<f64>>()
                .map(|matrix| (matrix.nrows(), matrix.ncols()))
        },
        |matrix| Some((matrix.nrows(), matrix.ncols())),
    )
}
