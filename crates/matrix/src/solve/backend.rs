// SPDX-License-Identifier: AGPL-3.0-only
//! Safe faer candidates stay opaque and carry no accuracy claim.

use super::allocations::{Backend, Buffer, FactorAllocationStage as Stage, Run};
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
}

/// Concrete adapters keep safe matrix indexing behind checked dimensions and precision gates.
macro_rules! adapter {
    ($pack:ident, $check:ident, $format:ty) => {
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
    };
}
adapter!(pack_single, check_single, f32);
adapter!(pack_double, check_double, f64);

impl Lu {
    /// Reuse the same factors for finite private differential or correction columns.
    pub(crate) fn solve_entries<P: Scalar + 'static>(
        &self,
        extent: crate::Extent,
        entries: &[P],
        attempt: super::scratch::Attempt<'_>,
    ) -> Result<crate::private::Buffer<P>, OperationError> {
        use super::scratch::Stage;
        if entries.len() != extent.entries() || extent.columns() == 0 {
            return Err(SolveError::Contract.into());
        }
        match self {
            Self::Single(factor) => {
                check_single(factor.get(), extent.rows())?;
                let packed = attempt.run.backend(
                    Stage::SensitivityPackedRightHandSide,
                    attempt.index,
                    || entries_single(extent, entries),
                )?;
                let result =
                    attempt
                        .run
                        .backend(Stage::SensitivityBackendResult, attempt.index, || {
                            Ok(factor.get().solve(packed.get()))
                        })?;
                planned_copy_single(
                    result.get(),
                    attempt.run,
                    attempt.index,
                    Stage::SensitivityCenters,
                )
            }
            Self::Double(factor) => {
                check_double(factor.get(), extent.rows())?;
                let packed = attempt.run.backend(
                    Stage::SensitivityPackedRightHandSide,
                    attempt.index,
                    || entries_double(extent, entries),
                )?;
                let result =
                    attempt
                        .run
                        .backend(Stage::SensitivityBackendResult, attempt.index, || {
                            Ok(factor.get().solve(packed.get()))
                        })?;
                planned_copy_double(
                    result.get(),
                    attempt.run,
                    attempt.index,
                    Stage::SensitivityCenters,
                )
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

/// The actual RHS owner retains its pre-allocation permit until the backend result is copied.
macro_rules! planned_solve {
    ($name:ident, $copy:ident, $pack:ident, $format:ty) => {
        /// Ticket both backend rectangles and the matrix-owned scalar copy.
        fn $name<P: Scalar + 'static>(
            factor: &faer::linalg::solvers::PartialPivLu<$format>,
            rhs: &super::RightHandSide<'_, '_, P>,
            run: &super::scratch::Run,
        ) -> Result<crate::private::Buffer<P>, OperationError> {
            use super::scratch::Stage;
            let packed = run.backend(Stage::PackedRightHandSide, 0, || match rhs {
                super::RightHandSide::Physical(view) => $pack(view),
                super::RightHandSide::Identity(shape) => Ok(faer::Mat::<$format>::identity(
                    shape.extent().rows(),
                    shape.extent().columns(),
                )),
            })?;
            let result = run.backend(Stage::BackendResult, 0, || Ok(factor.solve(packed.get())))?;
            $copy(result.get(), run, 0, Stage::Centers)
        }
        /// Copy finite values without detaching allocation authority or allowing unplanned growth.
        fn $copy<P: Scalar + 'static>(
            matrix: &faer::Mat<$format>,
            run: &super::scratch::Run,
            iteration: usize,
            stage: super::scratch::Stage,
        ) -> Result<crate::private::Buffer<P>, OperationError> {
            let mut result = run.buffer(stage, Some(iteration))?;
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
planned_solve!(planned_single, planned_copy_single, pack_single, f32);
planned_solve!(planned_double, planned_copy_double, pack_double, f64);

impl Lu {
    /// Physical and typed-identity solves use the same predeclared normalized coordinate schedule.
    pub(crate) fn solve_planned<P: Scalar + 'static>(
        &self,
        rhs: &super::RightHandSide<'_, '_, P>,
        run: &super::scratch::Run,
    ) -> Result<crate::private::Buffer<P>, OperationError> {
        match self {
            Self::Single(factor) => {
                check_single(factor.get(), rhs.extent().rows())?;
                planned_single(factor.get(), rhs, run)
            }
            Self::Double(factor) => {
                check_double(factor.get(), rhs.extent().rows())?;
                planned_double(factor.get(), rhs, run)
            }
        }
    }
    /// A finite correction has distinct planned RHS, result and copied-coordinate owners.
    pub(crate) fn correction_planned<P: Scalar + 'static>(
        &self,
        extent: crate::Extent,
        entries: &[P],
        run: &super::scratch::Run,
        iteration: usize,
    ) -> Result<crate::private::Buffer<P>, OperationError> {
        use super::scratch::Stage;
        if entries.len() != extent.entries() || extent.columns() == 0 {
            return Err(SolveError::Contract.into());
        }
        match self {
            Self::Single(factor) => {
                check_single(factor.get(), extent.rows())?;
                let packed = run.backend(Stage::PackedRightHandSide, iteration, || {
                    entries_single(extent, entries)
                })?;
                let result = run.backend(Stage::BackendResult, iteration, || {
                    Ok(factor.get().solve(packed.get()))
                })?;
                planned_copy_single(result.get(), run, iteration, Stage::Centers)
            }
            Self::Double(factor) => {
                check_double(factor.get(), extent.rows())?;
                let packed = run.backend(Stage::PackedRightHandSide, iteration, || {
                    entries_double(extent, entries)
                })?;
                let result = run.backend(Stage::BackendResult, iteration, || {
                    Ok(factor.get().solve(packed.get()))
                })?;
                planned_copy_double(result.get(), run, iteration, Stage::Centers)
            }
        }
    }
}
