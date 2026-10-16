// SPDX-License-Identifier: AGPL-3.0-only
//! Safe faer candidate multiplication, independent of the outward verification contraction.

use crate::binding::buffer;
use crate::{NormalizedView, OperationError};
use nrz_scalar::Scalar;

/// Select only the two sealed backend formats; no caller-defined scalar enters faer.
pub(crate) fn product<P: Scalar + 'static>(
    left: &NormalizedView<'_, P>,
    right: &NormalizedView<'_, P>,
) -> Result<Vec<P>, OperationError> {
    match P::PRECISION {
        nrz_scalar::Precision::F32 => single(left, right),
        nrz_scalar::Precision::F64 => double(left, right),
    }
}

/// Concrete safe backend adapters avoid unsupported generic faer scalar assumptions.
macro_rules! backend {
    ($name:ident, $format:ty) => {
        /// Pack checked immutable operands, compute a private candidate, and copy its entries.
        fn $name<P: Scalar + 'static>(
            left: &NormalizedView<'_, P>,
            right: &NormalizedView<'_, P>,
        ) -> Result<Vec<P>, OperationError> {
            let rows = left.extent().rows();
            let contracted = left.extent().columns();
            let columns = right.extent().columns();
            if contracted != right.extent().rows() {
                return Err(crate::KernelError::Contract.into());
            }
            let extent = crate::Extent::new(rows, columns)?;
            let mut first = faer::Mat::<$format>::zeros(rows, contracted);
            let mut second = faer::Mat::<$format>::zeros(contracted, columns);
            for column in 0..contracted {
                for row in 0..rows {
                    first[(row, column)] = crate::conversion::same_format(left.get(row, column)?)?;
                }
            }
            for column in 0..columns {
                for row in 0..contracted {
                    second[(row, column)] =
                        crate::conversion::same_format(right.get(row, column)?)?;
                }
            }
            let candidate = &first * &second;
            let mut result = buffer(extent.entries())?;
            for column in 0..columns {
                for row in 0..rows {
                    result.push(crate::conversion::same_format(candidate[(row, column)])?);
                }
            }
            Ok(result)
        }
    };
}
backend!(single, f32);
backend!(double, f64);
