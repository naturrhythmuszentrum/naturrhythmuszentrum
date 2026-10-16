// SPDX-License-Identifier: AGPL-3.0-only
//! Safe faer candidates stay opaque and carry no accuracy claim.

use crate::binding::buffer;
use crate::{NormalizedView, OperationError, SolveError};
use faer::prelude::*;
use nrz_scalar::{Precision, Scalar};

/// Concrete backend factorizations remain distinct from ordinary heap values.
#[derive(Debug)]
pub(crate) enum Lu {
    /// Partial-pivoted binary32 factors and their permutation.
    Single(faer::linalg::solvers::PartialPivLu<f32>),
    /// Partial-pivoted binary64 factors and their permutation.
    Double(faer::linalg::solvers::PartialPivLu<f64>),
}

impl Lu {
    /// Pack only a validated positive square normalized operator.
    pub(crate) fn factor<P: Scalar + 'static>(
        view: &NormalizedView<'_, P>,
    ) -> Result<Self, OperationError> {
        let order = view.extent().rows();
        if order == 0 || order != view.extent().columns() {
            return Err(SolveError::Contract.into());
        }
        match P::PRECISION {
            Precision::F32 => {
                let factor = pack_single(view)?.partial_piv_lu();
                check_single(&factor, order)?;
                Ok(Self::Single(factor))
            }
            Precision::F64 => {
                let factor = pack_double(view)?.partial_piv_lu();
                check_double(&factor, order)?;
                Ok(Self::Double(factor))
            }
        }
    }

    /// Solve physical RHS columns separately from the witness identity columns.
    pub(crate) fn solve<P: Scalar + 'static>(
        &self,
        rhs: &NormalizedView<'_, P>,
    ) -> Result<Vec<P>, OperationError> {
        match self {
            Self::Single(factor) => {
                check_single(factor, rhs.extent().rows())?;
                copy_single(&factor.solve(&pack_single(rhs)?))
            }
            Self::Double(factor) => {
                check_double(factor, rhs.extent().rows())?;
                copy_double(&factor.solve(&pack_double(rhs)?))
            }
        }
    }

    /// Obtain a private inverse candidate by a separate normalized identity solve.
    pub(crate) fn inverse<P: Scalar + 'static>(
        &self,
        order: usize,
    ) -> Result<Vec<P>, OperationError> {
        match self {
            Self::Single(factor) => {
                check_single(factor, order)?;
                copy_single(&factor.solve(&faer::Mat::<f32>::identity(order, order)))
            }
            Self::Double(factor) => {
                check_double(factor, order)?;
                copy_double(&factor.solve(&faer::Mat::<f64>::identity(order, order)))
            }
        }
    }
}

/// Concrete adapters keep safe matrix indexing behind checked dimensions and precision gates.
macro_rules! adapter {
    ($pack:ident, $copy:ident, $check:ident, $format:ty) => {
        /// Discharge every backend solve assertion and reject zero or nonfinite pivots.
        fn $check(
            factor: &faer::linalg::solvers::PartialPivLu<$format>,
            rows: usize,
        ) -> Result<(), OperationError> {
            let upper = factor.U();
            if rows == 0 || upper.nrows() != rows || upper.ncols() != rows {
                return Err(SolveError::Contract.into());
            }
            for index in 0..rows {
                let pivot = upper[(index, index)];
                if pivot == 0.0 || !pivot.is_finite() {
                    return Err(SolveError::CandidateFailure.into());
                }
            }
            Ok(())
        }

        /// Copy a checked logical view into one finite backend rectangle.
        fn $pack<P: Scalar + 'static>(
            view: &NormalizedView<'_, P>,
        ) -> Result<faer::Mat<$format>, OperationError> {
            let extent = view.extent();
            let mut result = faer::Mat::<$format>::zeros(extent.rows(), extent.columns());
            for column in 0..extent.columns() {
                for row in 0..extent.rows() {
                    result[(row, column)] = crate::conversion::same_format(view.get(row, column)?)?;
                }
            }
            Ok(result)
        }

        /// Nonfinite backend output is a candidate failure before certification or publication.
        fn $copy<P: Scalar + 'static>(
            matrix: &faer::Mat<$format>,
        ) -> Result<Vec<P>, OperationError> {
            let extent = crate::Extent::new(matrix.nrows(), matrix.ncols())?;
            let mut values = buffer(extent.entries())?;
            for column in 0..extent.columns() {
                for row in 0..extent.rows() {
                    let value = matrix[(row, column)];
                    if !value.is_finite() {
                        return Err(SolveError::CandidateFailure.into());
                    }
                    values.push(crate::conversion::same_format(value)?);
                }
            }
            Ok(values)
        }
    };
}
adapter!(pack_single, copy_single, check_single, f32);
adapter!(pack_double, copy_double, check_double, f64);
