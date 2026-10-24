// SPDX-License-Identifier: AGPL-3.0-only
//! Safe faer candidates stay opaque and carry no accuracy claim.

use super::allocations::{Backend, Buffer, FactorAllocationStage as Stage, Run};
use crate::binding::buffer;
use crate::{NormalizedView, OperationError, SolveError};
use faer::prelude::*;
use nrz_scalar::{Precision, Scalar};

/// Concrete backend factorizations remain distinct from ordinary heap values.
#[derive(Debug)]
pub(crate) enum Lu {
    /// Partial-pivoted binary32 factors and their permutation.
    Single(Backend<faer::linalg::solvers::PartialPivLu<f32>>),
    /// Partial-pivoted binary64 factors and their permutation.
    Double(Backend<faer::linalg::solvers::PartialPivLu<f64>>),
}

impl Lu {
    /// Pack only a validated positive square normalized operator.
    pub(crate) fn factor<P: Scalar + 'static>(
        view: &NormalizedView<'_, P>,
        run: &Run,
    ) -> Result<Self, OperationError> {
        let order = view.extent().rows();
        if order == 0 || order != view.extent().columns() {
            return Err(SolveError::Contract.into());
        }
        match P::PRECISION {
            Precision::F32 => {
                let packed = run.backend(Stage::PackedOperator, || pack_single(view))?;
                let factor = run.backend(Stage::Factors, || Ok(packed.get().partial_piv_lu()))?;
                check_single(factor.get(), order)?;
                Ok(Self::Single(factor))
            }
            Precision::F64 => {
                let packed = run.backend(Stage::PackedOperator, || pack_double(view))?;
                let factor = run.backend(Stage::Factors, || Ok(packed.get().partial_piv_lu()))?;
                check_double(factor.get(), order)?;
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
                check_single(factor.get(), rhs.extent().rows())?;
                copy_single(&factor.get().solve(&pack_single(rhs)?))
            }
            Self::Double(factor) => {
                check_double(factor.get(), rhs.extent().rows())?;
                copy_double(&factor.get().solve(&pack_double(rhs)?))
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
                check_single(factor.get(), order)?;
                copy_single(
                    &factor
                        .get()
                        .solve(&faer::Mat::<f32>::identity(order, order)),
                )
            }
            Self::Double(factor) => {
                check_double(factor.get(), order)?;
                copy_double(
                    &factor
                        .get()
                        .solve(&faer::Mat::<f64>::identity(order, order)),
                )
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

impl Lu {
    /// Reuse the same factors for finite private differential or correction columns.
    pub(crate) fn solve_entries<P: Scalar + 'static>(
        &self,
        extent: crate::Extent,
        entries: &[P],
    ) -> Result<Vec<P>, OperationError> {
        if entries.len() != extent.entries() || extent.columns() == 0 {
            return Err(SolveError::Contract.into());
        }
        match self {
            Self::Single(factor) => {
                check_single(factor.get(), extent.rows())?;
                copy_single(&factor.get().solve(&entries_single(extent, entries)?))
            }
            Self::Double(factor) => {
                check_double(factor.get(), extent.rows())?;
                copy_double(&factor.get().solve(&entries_double(extent, entries)?))
            }
        }
    }
}
/// Pack private finite entries without allowing a backend assertion to validate their shape.
macro_rules! entries_adapter {
    ($name:ident, $format:ty) => {
        /// Preserve the admitted scalar format through checked column-order indexing.
        fn $name<P: Scalar + 'static>(
            extent: crate::Extent,
            entries: &[P],
        ) -> Result<faer::Mat<$format>, OperationError> {
            let mut result = faer::Mat::<$format>::zeros(extent.rows(), extent.columns());
            for column in 0..extent.columns() {
                for row in 0..extent.rows() {
                    let value = *entries
                        .get(extent.index(row, column)?)
                        .ok_or(SolveError::Contract)?;
                    if !value.is_finite() {
                        return Err(SolveError::CandidateFailure.into());
                    }
                    result[(row, column)] = crate::conversion::same_format(value)?;
                }
            }
            Ok(result)
        }
    };
}
entries_adapter!(entries_single, f32);
entries_adapter!(entries_double, f64);

impl Lu {
    /// The factor's actual owner keeps its pre-allocation ticket through every later reuse.
    pub(crate) const fn ticket(&self) -> crate::PrivateAllocationTicket {
        match self {
            Self::Single(factor) => factor.ticket(),
            Self::Double(factor) => factor.ticket(),
        }
    }
    /// Inline backend object storage excludes all opaque heap storage and workspace.
    pub(crate) const fn inline_bytes(&self) -> usize {
        match self {
            Self::Single(factor) => size_of_val(factor.get()),
            Self::Double(factor) => size_of_val(factor.get()),
        }
    }
    /// Factor construction inventories the identity solve independently of later physical RHS solves.
    pub(crate) fn inverse_witness<P: Scalar + 'static>(
        &self,
        order: usize,
        run: &Run,
    ) -> Result<Buffer<P>, OperationError> {
        match self {
            Self::Single(factor) => {
                check_single(factor.get(), order)?;
                let identity = run.backend(Stage::Identity, || {
                    Ok(faer::Mat::<f32>::identity(order, order))
                })?;
                let result = run.backend(Stage::InverseResult, || {
                    Ok(factor.get().solve(identity.get()))
                })?;
                copy_factor_single(result.get(), run)
            }
            Self::Double(factor) => {
                check_double(factor.get(), order)?;
                let identity = run.backend(Stage::Identity, || {
                    Ok(faer::Mat::<f64>::identity(order, order))
                })?;
                let result = run.backend(Stage::InverseResult, || {
                    Ok(factor.get().solve(identity.get()))
                })?;
                copy_factor_double(result.get(), run)
            }
        }
    }
}
/// Copy backend inverse candidates into independently owned, pre-ticketed scalar buffers.
macro_rules! factor_copy {
    ($name:ident, $format:ty) => {
        /// Preserve the exact admitted scalar format and refuse nonfinite candidates before proof.
        fn $name<P: Scalar + 'static>(
            matrix: &faer::Mat<$format>,
            run: &Run,
        ) -> Result<Buffer<P>, OperationError> {
            let mut result = run.buffer(Stage::InverseCandidate)?;
            for column in 0..matrix.ncols() {
                for row in 0..matrix.nrows() {
                    let value = matrix[(row, column)];
                    if !value.is_finite() {
                        return Err(SolveError::CandidateFailure.into());
                    }
                    result.push(crate::conversion::same_format(value)?)?;
                }
            }
            Ok(result)
        }
    };
}
factor_copy!(copy_factor_single, f32);
factor_copy!(copy_factor_double, f64);
