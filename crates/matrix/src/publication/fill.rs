// SPDX-License-Identifier: AGPL-3.0-only
//! Fill reserved private faer slabs; every check runs before the no-fail commit.

use super::Payload;
use crate::binding::buffer;
use crate::storage::PreparedStorage;
use crate::{HeapError, OperationError};
use nrz_scalar::Scalar;
use std::any::Any;

/// Copy validated candidate bytes into private storage reserved by the binding.
pub(super) fn fill<P: Scalar + 'static>(
    storage: &mut PreparedStorage,
    outputs: &[Payload<P>],
) -> Result<(), OperationError> {
    let mut slices = buffer(storage.keys().len())?;
    for output in outputs {
        slices.push(output.centers.as_slice());
        if let Some(bounds) = &output.bounds {
            slices.push(bounds.as_slice());
        }
    }
    if slices.len() != storage.keys().len() {
        return Err(OperationError::CandidateContract);
    }
    match storage {
        PreparedStorage::Single { slots, .. } => {
            for (matrix, values) in slots.values.iter_mut().zip(slices) {
                fill_single(matrix, values)?;
            }
        }
        PreparedStorage::Double { slots, .. } => {
            for (matrix, values) in slots.values.iter_mut().zip(slices) {
                fill_double(matrix, values)?;
            }
        }
    }
    Ok(())
}

/// Each sealed backend format uses safe indexing after validating its private allocation.
macro_rules! fill_format {
    ($function:ident, $ty:ty) => {
        /// Write private scratch without borrowing any published numerical allocation.
        fn $function<P: Scalar + 'static>(
            matrix: &mut faer::Mat<$ty>,
            values: &[P],
        ) -> Result<(), HeapError> {
            if matrix.nrows() != values.len() || matrix.ncols() != 1 {
                return Err(HeapError::LengthMismatch);
            }
            for (index, value) in values.iter().enumerate() {
                let entry = (value as &dyn Any)
                    .downcast_ref::<$ty>()
                    .ok_or(HeapError::PrecisionMismatch)?;
                matrix[(index, 0)] = *entry;
            }
            Ok(())
        }
    };
}

fill_format!(fill_single, f32);
fill_format!(fill_double, f64);
