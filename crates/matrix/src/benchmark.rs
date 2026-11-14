// SPDX-License-Identifier: AGPL-3.0-only
//! Opt-in raw serial binary64 benchmark handles, isolated from certified storage.
//!
//! This surface accepts only owned ordinary buffers. No descriptor, companion,
//! ledger, certified region or numerical guarantee crosses this boundary.

use faer::dyn_stack::{MemBuffer, MemStack};
use faer::linalg::lu::partial_pivoting::factor::{lu_in_place, lu_in_place_scratch};
use faer::linalg::lu::partial_pivoting::solve::{solve_in_place, solve_in_place_scratch};
use std::hint::black_box;

/// Raw backend operation, never an engine-certified kernel claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawOperation {
    /// Dense matrix-matrix product.
    Gemm,
    /// Dense matrix-vector product with one output column.
    Gemv,
    /// Partial-pivot LU construction.
    Factor,
    /// Solve using a separately prepared partial-pivot LU factor.
    Solve,
}

/// Dense column-major shape and leading strides of the owned raw operands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawShape {
    /// Rows of the left matrix and output.
    pub rows: usize,
    /// Contracted dimension, equal to rows for factor and solve.
    pub contracted: usize,
    /// Output/right-hand-side columns, one for GEMV.
    pub columns: usize,
}

/// Checked raw adapter refusal, without an engine numerical certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RawError {
    /// Shape, input length or finite domain does not match the selected operation.
    #[error("invalid raw benchmark shape or domain")]
    Domain,
    /// Checked capacity arithmetic or declared payload allowance was exceeded.
    #[error("raw benchmark capacity exceeded")]
    Capacity,
    /// The pinned backend exposes no public requirement for possible internal packing.
    #[error("raw backend packing has no declared capacity requirement")]
    UnaccountedPacking,
    /// Raw backend candidate was nonfinite or singular.
    #[error("invalid raw backend candidate")]
    Candidate,
}

/// Actual owned buffer evidence with no backend types or exported addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawBufferLayout {
    /// Actual payload capacity in bytes.
    pub capacity_bytes: usize,
    /// Logical live payload in bytes.
    pub live_bytes: usize,
    /// Base address modulo 64.
    pub address_modulo_64: usize,
    /// Row stride in bytes.
    pub row_stride_bytes: usize,
    /// Column stride in bytes; zero for scratch and index vectors.
    pub column_stride_bytes: usize,
}

/// Opaque serial faer state; no faer type occurs in its public API.
pub struct RawKernel {
    /// Named raw operation.
    operation: RawOperation,
    /// Checked logical shape.
    shape: RawShape,
    /// Immutable prepared left operand.
    left: Vec<f64>,
    /// Immutable prepared right operand.
    right: Vec<f64>,
    /// Mutable output/reset storage.
    output: Vec<f64>,
    /// Immutable owned input for explicit factor restoration.
    source_left: Vec<f64>,
    /// Owned right input, kept detached from all certified storage.
    source_right: Vec<f64>,
    /// Preallocated partial-pivot LU permutation.
    permutation: Vec<usize>,
    /// Preallocated inverse permutation.
    inverse_permutation: Vec<usize>,
    /// Backend-declared serial factor scratch, allocated during preparation.
    factor_scratch: MemBuffer,
    /// Fixed serial backend configuration shared by preparation and execution.
    parallelism: faer::Par,
    /// Actual owned buffers and declared factor/solve scratch, with opaque-packed paths refused.
    reserved_bytes: usize,
}

impl std::fmt::Debug for RawKernel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RawKernel")
            .field("operation", &self.operation)
            .field("shape", &self.shape)
            .field("reserved_bytes", &self.reserved_bytes)
            .finish_non_exhaustive()
    }
}

/// Validate every dense dimension and finite source before owned backend allocation.
fn validate_inputs(
    operation: RawOperation,
    shape: RawShape,
    left: &[f64],
    right: &[f64],
) -> Result<(usize, usize, usize), RawError> {
    let a = shape
        .rows
        .checked_mul(shape.contracted)
        .ok_or(RawError::Capacity)?;
    let b = shape
        .contracted
        .checked_mul(shape.columns)
        .ok_or(RawError::Capacity)?;
    let c = shape
        .rows
        .checked_mul(shape.columns)
        .ok_or(RawError::Capacity)?;
    if shape.rows == 0 || shape.contracted == 0 || shape.columns == 0 || left.len() != a {
        return Err(RawError::Domain);
    }
    let square = matches!(operation, RawOperation::Factor | RawOperation::Solve);
    if square && shape.rows != shape.contracted {
        return Err(RawError::Domain);
    }
    if operation == RawOperation::Factor && shape.columns != shape.rows {
        return Err(RawError::Domain);
    }
    a.checked_mul(shape.columns).ok_or(RawError::Capacity)?;
    if operation == RawOperation::Gemv && shape.columns != 1 {
        return Err(RawError::Domain);
    }
    let right_valid = if operation == RawOperation::Factor {
        right.is_empty()
    } else {
        right.len() == b
    };
    if !right_valid || left.iter().chain(right).any(|value| !value.is_finite()) {
        return Err(RawError::Domain);
    }
    Ok((a, b, c))
}

/// Reserve an exact requested population and charge its actual retained capacity.
fn buffer<T: Copy + Default>(length: usize, remaining: &mut usize) -> Result<Vec<T>, RawError> {
    let requested = length
        .checked_mul(size_of::<T>())
        .ok_or(RawError::Capacity)?;
    if requested > *remaining {
        return Err(RawError::Capacity);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| RawError::Capacity)?;
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(RawError::Capacity)?;
    *remaining = remaining.checked_sub(actual).ok_or(RawError::Capacity)?;
    values.resize(length, T::default());
    Ok(values)
}

/// The exact backend operands shared by factor preparation and timed dispatch.
struct Operands<'a> {
    /// Mutable left storage, read-only for multiplication and solve.
    left: faer::MatMut<'a, f64>,
    /// Immutable right operand.
    right: faer::MatRef<'a, f64>,
    /// Mutable destination or solve right-hand sides.
    output: faer::MatMut<'a, f64>,
}

/// Construct all backend views in one place so layout tests cover timed dispatch.
fn operands<'a>(
    shape: RawShape,
    left: &'a mut [f64],
    right: &'a [f64],
    output: &'a mut [f64],
) -> Operands<'a> {
    let right_columns = if right.is_empty() { 0 } else { shape.columns };
    Operands {
        left: faer::MatMut::from_column_major_slice_mut(left, shape.rows, shape.contracted),
        right: faer::MatRef::from_column_major_slice(right, shape.contracted, right_columns),
        output: faer::MatMut::from_column_major_slice_mut(output, shape.rows, shape.columns),
    }
}

impl RawKernel {
    /// Fixed owned storage slots: left, right, output, source left/right, permutations, scratch.
    #[must_use]
    pub fn buffer_layouts(&self) -> [RawBufferLayout; 8] {
        let vector = |capacity, length, address, element, rows| RawBufferLayout {
            capacity_bytes: capacity * element,
            live_bytes: length * element,
            address_modulo_64: address % 64,
            row_stride_bytes: element,
            column_stride_bytes: rows * element,
        };
        [
            vector(
                self.left.capacity(),
                self.left.len(),
                self.left.as_ptr().addr(),
                8,
                self.shape.rows,
            ),
            vector(
                self.right.capacity(),
                self.right.len(),
                self.right.as_ptr().addr(),
                8,
                self.shape.contracted,
            ),
            vector(
                self.output.capacity(),
                self.output.len(),
                self.output.as_ptr().addr(),
                8,
                self.shape.rows,
            ),
            vector(
                self.source_left.capacity(),
                self.source_left.len(),
                self.source_left.as_ptr().addr(),
                8,
                self.shape.rows,
            ),
            vector(
                self.source_right.capacity(),
                self.source_right.len(),
                self.source_right.as_ptr().addr(),
                8,
                self.shape.contracted,
            ),
            vector(
                self.permutation.capacity(),
                self.permutation.len(),
                self.permutation.as_ptr().addr(),
                size_of::<usize>(),
                0,
            ),
            vector(
                self.inverse_permutation.capacity(),
                self.inverse_permutation.len(),
                self.inverse_permutation.as_ptr().addr(),
                size_of::<usize>(),
                0,
            ),
            vector(
                self.factor_scratch.len(),
                self.factor_scratch.len(),
                self.factor_scratch.as_ptr().addr(),
                1,
                0,
            ),
        ]
    }

    /// Declared packing payload on the source-proven allocation-free dispatch paths.
    ///
    /// faer 0.24.4 `src/linalg/matmul/mod.rs`: n=1 selects column-major
    /// matvec at 1214–1229; otherwise m=1 transposes at 1247–1249 and selects column-major
    /// matvec at 1252–1265 for k=1, or row-major matvec at 1267–1280 otherwise.
    /// Otherwise k=1 selects rank update at 1283–1298. These dispatches require
    /// the `is_simd()` guard at 1201. Our operands
    /// have row stride 1 and column stride equal to their row count. With
    /// `Par::Seq`, matvec uses 775–844 or 894–975 (the allocation at 980 is
    /// Rayon-only); rank update uses 1147–1150 and 1038–1136.
    /// LU order 1 uses `src/linalg/lu/partial_pivoting/factor.rs` 81–83,
    /// 19–66, with caller stack at 263–273; its empty trailing update exits
    /// `matmul/mod.rs` 1190–1192. Solve uses `src/linalg/lu/partial_pivoting/solve.rs`
    /// 39–52, caller-stack permutation in `src/perm/mod.rs` 320–339, and
    /// `src/linalg/triangular_solve.rs` 454–469, 535–550, 28 and 111–117; upper solve reverses at 592–604.
    /// Multiple RHS columns split at 430–448 and 511–529, with serial
    /// `src/utils/mod.rs` 21–23 calling both branches without spawning.
    /// These pinned source citations establish the absence of heap allocation;
    /// no allocation measurement is claimed. A faer version change invalidates
    /// this proof. Other shapes and measured allocation evidence remain open
    /// under ``open:nrz:compute-backend-packing-requirement``.
    #[must_use]
    pub const fn packing_bytes(operation: RawOperation, shape: RawShape) -> Option<usize> {
        let direct = shape.rows == 1 || shape.columns == 1 || shape.contracted == 1;
        match operation {
            RawOperation::Gemm | RawOperation::Gemv if direct => Some(0),
            RawOperation::Factor | RawOperation::Solve if shape.rows == 1 => Some(0),
            RawOperation::Gemm
            | RawOperation::Gemv
            | RawOperation::Factor
            | RawOperation::Solve => None,
        }
    }

    /// Prepare checked owned buffers with serial backend configuration.
    /// # Errors
    /// Rejects zero/incompatible shapes, nonfinite inputs and insufficient payload allowance.
    pub fn prepare(
        operation: RawOperation,
        shape: RawShape,
        left: Vec<f64>,
        right: Vec<f64>,
        byte_cap: usize,
    ) -> Result<Self, RawError> {
        if Self::packing_bytes(operation, shape).is_none() {
            return Err(RawError::UnaccountedPacking);
        }
        let (left_len, _, output_len) = validate_inputs(operation, shape, &left, &right)?;
        let source_bytes = left
            .capacity()
            .checked_add(right.capacity())
            .and_then(|n| n.checked_mul(size_of::<f64>()))
            .and_then(|n| n.checked_add(size_of::<Self>()))
            .ok_or(RawError::Capacity)?;
        let mut remaining = byte_cap
            .checked_sub(source_bytes)
            .ok_or(RawError::Capacity)?;
        let source_left = left;
        let source_right = right;
        let mut left = buffer(left_len, &mut remaining)?;
        left.copy_from_slice(&source_left);
        let mut right = buffer(source_right.len(), &mut remaining)?;
        right.copy_from_slice(&source_right);
        let output = buffer(output_len, &mut remaining)?;
        let order = if matches!(operation, RawOperation::Factor | RawOperation::Solve) {
            shape.rows
        } else {
            0
        };
        let permutation = buffer(order, &mut remaining)?;
        let inverse_permutation = buffer(order, &mut remaining)?;
        let requirement =
            lu_in_place_scratch::<usize, f64>(order, order, faer::Par::Seq, faer::Spec::default());
        let requirement = if operation == RawOperation::Solve {
            requirement.or(solve_in_place_scratch::<usize, f64>(
                shape.rows,
                shape.columns,
                faer::Par::Seq,
            ))
        } else {
            requirement
        };
        let scratch_bytes = requirement.layout().map_err(|_| RawError::Capacity)?.size();
        if scratch_bytes > remaining {
            return Err(RawError::Capacity);
        }
        let factor_scratch = MemBuffer::try_new(requirement).map_err(|_| RawError::Capacity)?;
        remaining = remaining
            .checked_sub(factor_scratch.len())
            .ok_or(RawError::Capacity)?;
        let reserved_bytes = byte_cap - remaining;
        let mut result = Self {
            operation,
            parallelism: faer::Par::Seq,
            shape,
            left,
            right,
            output,
            source_left,
            source_right,
            permutation,
            inverse_permutation,
            factor_scratch,
            reserved_bytes,
        };
        if operation == RawOperation::Solve {
            result.factorize();
        }
        result.reset();
        Ok(result)
    }

    /// Construct packed LU with preallocated permutation and serial backend scratch.
    fn factorize(&mut self) {
        let views = operands(self.shape, &mut self.left, &self.right, &mut self.output);
        let result = lu_in_place(
            views.left,
            &mut self.permutation,
            &mut self.inverse_permutation,
            self.parallelism,
            MemStack::new(&mut self.factor_scratch),
            faer::Spec::default(),
        );
        black_box(result);
    }

    /// Logical shape and leading strides remain inspectable without backend types.
    #[must_use]
    pub const fn shape(&self) -> RawShape {
        self.shape
    }

    /// Actual owned payload and declared scratch; opaque-packed paths are refused.
    #[must_use]
    pub const fn reserved_bytes(&self) -> usize {
        self.reserved_bytes
    }

    /// Declared backend worker count used by every execute dispatch.
    #[must_use]
    pub fn backend_workers(&self) -> usize {
        self.parallelism.degree()
    }

    /// Actual runtime ISA selection is not exposed by this backend adapter.
    #[must_use]
    pub const fn selected_path(&self) -> Option<&'static str> {
        None
    }

    /// Restore mutable outputs outside the selected kernel timer.
    pub fn reset(&mut self) {
        if self.operation == RawOperation::Solve {
            for column in 0..self.shape.columns {
                for row in 0..self.shape.rows {
                    self.output[column * self.shape.rows + row] =
                        self.source_right[column * self.shape.rows + row];
                }
            }
        } else {
            self.output.fill(0.0);
            if self.operation == RawOperation::Factor {
                for column in 0..self.shape.contracted {
                    for row in 0..self.shape.rows {
                        self.left[column * self.shape.rows + row] =
                            self.source_left[column * self.shape.rows + row];
                    }
                }
            }
        }
    }

    /// Execute the serial raw backend and consume its complete output boundary.
    /// # Errors
    /// Rejects an unavailable prepared factor or invalid raw candidate.
    pub fn execute(&mut self) -> Result<(), RawError> {
        match self.operation {
            RawOperation::Gemm | RawOperation::Gemv => {
                let views = operands(self.shape, &mut self.left, &self.right, &mut self.output);
                faer::linalg::matmul::matmul(
                    views.output,
                    faer::Accum::Replace,
                    black_box(views.left.as_ref()),
                    black_box(views.right),
                    1.0,
                    self.parallelism,
                );
                black_box(self.output.as_slice());
            }
            RawOperation::Factor => {
                self.factorize();
                black_box(self.left.as_slice());
            }
            RawOperation::Solve => {
                let permutation = faer::perm::PermRef::new_checked(
                    &self.permutation,
                    &self.inverse_permutation,
                    self.shape.rows,
                );
                let views = operands(self.shape, &mut self.left, &self.right, &mut self.output);
                solve_in_place(
                    views.left.as_ref(),
                    views.left.as_ref(),
                    permutation,
                    views.output,
                    self.parallelism,
                    MemStack::new(&mut self.factor_scratch),
                );
                black_box(self.output.as_slice());
            }
        }
        Ok(())
    }

    /// Dry validation of finite centers and nonzero LU pivots, outside timing.
    /// # Errors
    /// Rejects nonfinite outputs or a zero/nonfinite LU pivot.
    pub fn validate(&mut self) -> Result<(), RawError> {
        self.execute()?;
        if matches!(self.operation, RawOperation::Factor | RawOperation::Solve) {
            if self.left.iter().any(|value| !value.is_finite()) {
                return Err(RawError::Candidate);
            }
            for index in 0..self.shape.rows {
                let pivot = self.left[index * self.shape.rows + index];
                if pivot == 0.0 || !pivot.is_finite() {
                    return Err(RawError::Candidate);
                }
            }
        }
        for column in 0..self.shape.columns {
            for row in 0..self.shape.rows {
                if !self.output[column * self.shape.rows + row].is_finite() {
                    return Err(RawError::Candidate);
                }
            }
        }
        self.reset();
        Ok(())
    }
}

impl RawKernel {
    /// Copy explicitly raw output centers outside the benchmark timer.
    ///
    /// A factor is represented by its strict-lower multipliers and upper triangle;
    /// this is raw backend state with no descriptor, companion or certificate.
    /// # Errors
    /// Rejects unavailable factor output or failed bounded output reservation.
    pub fn raw_centers(&self) -> Result<Vec<f64>, RawError> {
        let count = self
            .shape
            .rows
            .checked_mul(self.shape.columns)
            .ok_or(RawError::Capacity)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| RawError::Capacity)?;
        for column in 0..self.shape.columns {
            for row in 0..self.shape.rows {
                let value = if self.operation == RawOperation::Factor {
                    self.left[column * self.shape.rows + row]
                } else {
                    self.output[column * self.shape.rows + row]
                };
                output.push(value);
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    //! Hand-derived raw matrix oracles; no elapsed-time expectations.

    use super::{RawError, RawKernel, RawOperation, RawShape};

    #[test]
    fn backend_operands_have_dispatch_layouts() -> Result<(), RawError> {
        for (operation, rows, contracted, columns) in [
            (RawOperation::Gemm, 1, 17, 23),
            (RawOperation::Gemm, 1, 1, 23),
            (RawOperation::Gemm, 19, 1, 23),
            (RawOperation::Gemm, 19, 17, 1),
            (RawOperation::Gemv, 19, 17, 1),
            (RawOperation::Factor, 1, 1, 1),
            (RawOperation::Solve, 1, 1, 23),
        ] {
            let shape = RawShape {
                rows,
                contracted,
                columns,
            };
            let right_columns = if operation == RawOperation::Factor {
                0
            } else {
                columns
            };
            let mut kernel = RawKernel::prepare(
                operation,
                shape,
                vec![2.0; rows * contracted],
                vec![1.0; contracted * right_columns],
                1_048_576,
            )?;
            let views = super::operands(shape, &mut kernel.left, &kernel.right, &mut kernel.output);
            for (view, expected_rows, expected_columns) in [
                (views.left.as_ref(), rows, contracted),
                (views.right, contracted, right_columns),
                (views.output.as_ref(), rows, columns),
            ] {
                assert_eq!(view.nrows(), expected_rows);
                assert_eq!(view.ncols(), expected_columns);
                assert_eq!(view.row_stride(), 1);
                assert_eq!(usize::try_from(view.col_stride()), Ok(view.nrows()));
            }
            assert_eq!(views.left.row_stride(), 1);
            assert_eq!(
                usize::try_from(views.left.col_stride()),
                Ok(views.left.nrows())
            );
            assert_eq!(views.output.row_stride(), 1);
            assert_eq!(
                usize::try_from(views.output.col_stride()),
                Ok(views.output.nrows())
            );
            kernel.execute()?;
        }
        Ok(())
    }

    #[test]
    fn raw_product_has_explicit_column_major_output() -> Result<(), RawError> {
        let shape = RawShape {
            rows: 2,
            contracted: 1,
            columns: 2,
        };
        let mut kernel = RawKernel::prepare(
            RawOperation::Gemm,
            shape,
            vec![1.0, 3.0],
            vec![5.0, 6.0],
            4096,
        )?;
        kernel.execute()?;
        assert_eq!(kernel.raw_centers()?, [5.0, 15.0, 6.0, 18.0]);
        Ok(())
    }

    #[test]
    fn prepared_solve_resets_right_hand_side() -> Result<(), RawError> {
        let shape = RawShape {
            rows: 1,
            contracted: 1,
            columns: 1,
        };
        let mut kernel =
            RawKernel::prepare(RawOperation::Solve, shape, vec![4.0], vec![8.0], 4096)?;
        kernel.execute()?;
        assert_eq!(kernel.raw_centers()?, [2.0]);
        kernel.reset();
        kernel.execute()?;
        assert_eq!(kernel.raw_centers()?, [2.0]);
        Ok(())
    }
    #[test]
    fn raw_factor_restoration_has_hand_derived_packed_output() -> Result<(), RawError> {
        let shape = RawShape {
            rows: 1,
            contracted: 1,
            columns: 1,
        };
        let mut kernel =
            RawKernel::prepare(RawOperation::Factor, shape, vec![4.0], Vec::new(), 4096)?;
        for _ in 0..2 {
            kernel.execute()?;
            assert_eq!(kernel.raw_centers()?, [4.0]);
            kernel.reset();
        }
        Ok(())
    }

    #[test]
    fn nonsquare_raw_vector_product_and_invalid_factor_shape() -> Result<(), RawError> {
        let shape = RawShape {
            rows: 3,
            contracted: 2,
            columns: 1,
        };
        let mut kernel = RawKernel::prepare(
            RawOperation::Gemv,
            shape,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            vec![7.0, 8.0],
            4096,
        )?;
        kernel.execute()?;
        assert_eq!(kernel.raw_centers()?, [39.0, 54.0, 69.0]);
        let shape = RawShape {
            rows: 1,
            contracted: 1,
            columns: 3,
        };
        assert!(matches!(
            RawKernel::prepare(RawOperation::Factor, shape, vec![4.0], Vec::new(), 4096),
            Err(RawError::Domain)
        ));
        Ok(())
    }
    #[test]
    fn opaque_packing_refuses_before_input_access_or_owned_reservation() {
        // Even absent inputs and a zero byte cap return the capability refusal,
        // before shape/input validation or any fallible reservation is reached.
        // ``open:nrz:compute-backend-packing-requirement``
        for operation in [
            RawOperation::Gemm,
            RawOperation::Factor,
            RawOperation::Solve,
        ] {
            let shape = RawShape {
                rows: 1024,
                contracted: 1024,
                columns: 1024,
            };
            assert_eq!(RawKernel::packing_bytes(operation, shape), None);
            assert!(matches!(
                RawKernel::prepare(operation, shape, Vec::new(), Vec::new(), 0),
                Err(RawError::UnaccountedPacking)
            ));
        }
    }
}
