// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic raw matrix recipes through the matrix-owned feature surface.

use crate::{
    CaseRequest, Operation, Scope, Seed,
    recipe::{Numerators, RecipeError, input},
};
use nrz_matrix::benchmark::{RawKernel, RawOperation, RawShape};

/// Compute owns recipes and timing scope; faer state remains behind matrix's opaque handle.
#[derive(Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the crate-visible kernel enum retains this prepared state"
)]
pub(crate) struct Matrix {
    /// Exact recipe selection retained for preparation-inclusive repetitions.
    case: CaseRequest,
    /// Full deterministic input identity.
    seed: Seed,
    /// Explicit payload allowance for this serial kernel.
    byte_cap: u64,
    /// Matrix-owned prepared binary64 kernel.
    handle: Option<RawKernel>,
}

/// Decode explicitly requested dimensions and operation, without default shapes.
fn shape(case: &CaseRequest) -> Result<(RawOperation, RawShape), RecipeError> {
    match (case.operation, case.shape.as_slice()) {
        (Operation::Gemm, [rows, contracted, columns]) => Ok((
            RawOperation::Gemm,
            RawShape {
                rows: usize::try_from(*rows).map_err(|_| RecipeError::Overflow)?,
                contracted: usize::try_from(*contracted).map_err(|_| RecipeError::Overflow)?,
                columns: usize::try_from(*columns).map_err(|_| RecipeError::Overflow)?,
            },
        )),
        (Operation::Gemv, [rows, contracted]) => Ok((
            RawOperation::Gemv,
            RawShape {
                rows: usize::try_from(*rows).map_err(|_| RecipeError::Overflow)?,
                contracted: usize::try_from(*contracted).map_err(|_| RecipeError::Overflow)?,
                columns: 1,
            },
        )),
        (Operation::Factor, [order]) => Ok((
            RawOperation::Factor,
            RawShape {
                rows: usize::try_from(*order).map_err(|_| RecipeError::Overflow)?,
                contracted: usize::try_from(*order).map_err(|_| RecipeError::Overflow)?,
                columns: usize::try_from(*order).map_err(|_| RecipeError::Overflow)?,
            },
        )),
        (Operation::Solve, [order, columns]) => Ok((
            RawOperation::Solve,
            RawShape {
                rows: usize::try_from(*order).map_err(|_| RecipeError::Overflow)?,
                contracted: usize::try_from(*order).map_err(|_| RecipeError::Overflow)?,
                columns: usize::try_from(*columns).map_err(|_| RecipeError::Overflow)?,
            },
        )),
        _ => Err(RecipeError::Domain),
    }
}

/// Shape-only capability check precedes numerical generation and every owned allocation.
pub(super) fn packing_supported(case: &CaseRequest) -> bool {
    shape(case).is_ok_and(|(operation, shape)| RawKernel::packing_bytes(operation, shape).is_some())
}

/// Column-major seeded entries, followed by the declared ascending row construction.
fn inputs(
    case: &CaseRequest,
    seed: Seed,
    shape: RawShape,
    byte_cap: u64,
) -> Result<(Vec<f64>, Vec<f64>), RecipeError> {
    let left_len = shape
        .rows
        .checked_mul(shape.contracted)
        .ok_or(RecipeError::Overflow)?;
    let right_len = if case.operation == Operation::Factor {
        0
    } else {
        shape
            .contracted
            .checked_mul(shape.columns)
            .ok_or(RecipeError::Overflow)?
    };
    let mut budget = crate::memory::Budget::new(byte_cap);
    let mut generate = |length: usize, offset: usize| -> Result<Vec<f64>, RecipeError> {
        let mut values = budget.reserve(length)?;
        for index in 0..length {
            values.push(
                input(
                    seed,
                    u64::try_from(index.checked_add(offset).ok_or(RecipeError::Overflow)?)
                        .map_err(|_| RecipeError::Overflow)?,
                )? - 1.5,
            );
        }
        Ok(values)
    };
    let mut left = generate(left_len, 0)?;
    let right = generate(right_len, left_len)?;
    if matches!(case.operation, Operation::Factor | Operation::Solve) {
        let lambda = f64::from_bits(case.diagonal_margin.ok_or(RecipeError::Domain)?.0);
        for row in 0..shape.rows {
            let mut sum = 0.0;
            for column in 0..shape.rows {
                if row != column {
                    sum += left[column * shape.rows + row].abs();
                }
            }
            let diagonal = sum + lambda;
            if !diagonal.is_finite() || diagonal <= sum {
                return Err(RecipeError::Domain);
            }
            left[row * shape.rows + row] = diagonal;
        }
    }
    Ok((left, right))
}

/// Prepare all recipe-owned buffers and the opaque matrix handle under checked caps.
fn prepare(case: &CaseRequest, seed: Seed, byte_cap: u64) -> Result<RawKernel, RecipeError> {
    let (operation, shape) = shape(case)?;
    let entries = shape
        .rows
        .checked_mul(shape.contracted)
        .and_then(|left| {
            shape
                .contracted
                .checked_mul(shape.columns)
                .and_then(|right| left.checked_add(right))
        })
        .ok_or(RecipeError::Overflow)?;
    if u64::try_from(entries)
        .map_err(|_| RecipeError::Overflow)?
        .checked_mul(8)
        .ok_or(RecipeError::Overflow)?
        > byte_cap
    {
        return Err(RecipeError::Allocation);
    }
    let (left, right) = inputs(case, seed, shape, byte_cap)?;
    RawKernel::prepare(
        operation,
        shape,
        left,
        right,
        usize::try_from(byte_cap).map_err(|_| RecipeError::Overflow)?,
    )
    .map_err(|error| match error {
        nrz_matrix::benchmark::RawError::Capacity => RecipeError::Allocation,
        nrz_matrix::benchmark::RawError::Domain
        | nrz_matrix::benchmark::RawError::Candidate
        | nrz_matrix::benchmark::RawError::UnaccountedPacking => RecipeError::Domain,
    })
}

impl Matrix {
    /// Matrix-owned storage view contains only extents, strides and address residues.
    pub(super) fn layout(&self) -> Result<[Option<crate::layout::BufferLayout>; 8], RecipeError> {
        use crate::layout::{BufferLayout, BufferRole};
        let handle = self.handle.as_ref().ok_or(RecipeError::Domain)?;
        let roles = [
            BufferRole::Input,
            BufferRole::Input,
            BufferRole::Output,
            BufferRole::Input,
            BufferRole::Input,
            BufferRole::Permutation,
            BufferRole::Permutation,
            BufferRole::Scratch,
        ];
        let mut out = [None; 8];
        let count = |n| u64::try_from(n).map_err(|_| RecipeError::Overflow);
        for ((slot, value), role) in out.iter_mut().zip(handle.buffer_layouts()).zip(roles) {
            if value.capacity_bytes != 0 {
                *slot = Some(BufferLayout {
                    role,
                    capacity_bytes: count(value.capacity_bytes)?,
                    live_bytes: count(value.live_bytes)?,
                    address_modulo_64: count(value.address_modulo_64)?,
                    row_stride_bytes: count(value.row_stride_bytes)?,
                    column_stride_bytes: count(value.column_stride_bytes)?,
                });
            }
        }
        Ok(out)
    }

    /// Prepare and independently dry-check a deterministic finite kernel.
    pub(super) fn prepare(
        case: &CaseRequest,
        seed: Seed,
        byte_cap: u64,
    ) -> Result<Self, RecipeError> {
        let mut budget = crate::memory::Budget::new(byte_cap);
        budget.charge(size_of::<Self>())?;
        let case = crate::memory::clone_case(case, &mut budget)?;
        let byte_cap = budget.remaining();
        let mut handle = prepare(&case, seed, byte_cap)?;
        handle.validate().map_err(|_| RecipeError::Domain)?;
        Ok(Self {
            case,
            seed,
            byte_cap,
            handle: Some(handle),
        })
    }
    /// Restore mutable backend output outside the timed operation.
    pub(super) fn reset(&mut self) {
        if let Some(handle) = &mut self.handle {
            handle.reset();
        }
    }
    /// Execute each selected scope and consume every backend output before reuse.
    pub(super) fn execute(&mut self, length: u64) -> Result<(), RecipeError> {
        if self.case.operation == Operation::Factor
            && self.case.scope != Scope::RestorationInclusiveRepeated
            && length != 1
        {
            return Err(RecipeError::Domain);
        }
        for repetition in 0..length {
            let repeated_solve = repetition != 0 && self.case.operation == Operation::Solve;
            let repeated_factor = self.case.scope == Scope::RestorationInclusiveRepeated;
            if self.case.scope == Scope::PreparationInclusive {
                drop(self.handle.take());
                self.handle = Some(prepare(&self.case, self.seed, self.byte_cap)?);
            } else if repeated_solve || repeated_factor {
                self.reset();
            }
            self.handle
                .as_mut()
                .ok_or(RecipeError::Domain)?
                .execute()
                .map_err(|_| RecipeError::Domain)?;
        }
        Ok(())
    }
    /// Check the complete requested item outside measurement and restore state.
    pub(super) fn validate(&mut self, length: u64) -> Result<(), RecipeError> {
        self.execute(length)?;
        self.reset();
        self.handle
            .as_mut()
            .ok_or(RecipeError::Domain)?
            .validate()
            .map_err(|_| RecipeError::Domain)
    }
}

/// Matrix numerator ledger: factor/solve intentionally have no invented FLOP count.
pub(super) fn counts(case: &CaseRequest, length: u64) -> Result<Numerators, RecipeError> {
    let (_, shape) = shape(case)?;
    let rows = u64::try_from(shape.rows).map_err(|_| RecipeError::Overflow)?;
    let contracted = u64::try_from(shape.contracted).map_err(|_| RecipeError::Overflow)?;
    let columns = u64::try_from(shape.columns).map_err(|_| RecipeError::Overflow)?;
    let elements = rows
        .checked_mul(columns)
        .and_then(|n| n.checked_mul(length))
        .ok_or(RecipeError::Overflow)?;
    let floating = if matches!(case.operation, Operation::Gemm | Operation::Gemv) {
        Some(
            elements
                .checked_mul(contracted)
                .and_then(|n| n.checked_mul(2))
                .ok_or(RecipeError::Overflow)?,
        )
    } else {
        None
    };
    if [Some(elements), floating]
        .into_iter()
        .flatten()
        .any(|n| n > crate::decimal::MAX_INTEGER)
    {
        return Err(RecipeError::Overflow);
    }
    Ok(Numerators {
        items: 1,
        elements,
        vector_operations: None,
        floating_operations: floating,
        logical_bytes: None,
    })
}
