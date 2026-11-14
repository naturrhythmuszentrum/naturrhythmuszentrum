// SPDX-License-Identifier: AGPL-3.0-only
//! Contiguous streaming with explicit reduction and output-consumption order.

use crate::{
    CaseRequest, Operation, Seed, Width,
    recipe::{RecipeError, input},
};
use std::{hint::black_box, simd::Simd};

/// Disjoint owned buffers; their full capacities contribute to resource accounting.
#[derive(Clone, Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the crate-visible kernel enum retains this prepared state"
)]
pub(crate) struct Stream {
    /// Explicit stream variant.
    operation: Operation,
    /// Logical read width; other streams use scalar source identity.
    width: Width,
    /// Runtime fill or fused coefficient.
    coefficient: f64,
    /// First input buffer, empty when unused.
    x: Vec<f64>,
    /// Second fused input buffer, empty when unused.
    y: Vec<f64>,
    /// Full consumed destination, empty for read.
    output: Vec<f64>,
}

/// Allocate and first-touch a bounded input buffer from the revisioned generator.
fn buffer(
    length: usize,
    seed: Seed,
    offset: u64,
    budget: &mut crate::memory::Budget,
) -> Result<Vec<f64>, RecipeError> {
    let mut values = budget.reserve(length)?;
    for index in 0..length {
        let position = u64::try_from(index)
            .map_err(|_| RecipeError::Overflow)?
            .checked_add(offset)
            .ok_or(RecipeError::Overflow)?;
        values.push(input(seed, position)? - 1.5);
    }
    Ok(values)
}

impl Stream {
    /// Fixed storage evidence captured after allocation and before any item timer.
    pub(super) fn layout(&self) -> Result<[Option<crate::layout::BufferLayout>; 8], RecipeError> {
        use crate::layout::{BufferRole, buffer};
        let mut out = [None; 8];
        out[0] = buffer(&self.x, BufferRole::Input, 0)?;
        out[1] = buffer(&self.y, BufferRole::Input, 0)?;
        out[2] = buffer(&self.output, BufferRole::Output, 0)?;
        Ok(out)
    }

    /// Prepare only the selected stream's owned operand buffers.
    pub(super) fn prepare(
        case: &CaseRequest,
        seed: Seed,
        byte_cap: u64,
    ) -> Result<Self, RecipeError> {
        let n = *case.shape.first().ok_or(RecipeError::Domain)?;
        let buffers = match case.operation {
            Operation::Read | Operation::Write => 1,
            Operation::Copy => 2,
            Operation::FusedStream => 3,
            _ => return Err(RecipeError::Domain),
        };
        let bytes = n
            .checked_mul(8)
            .and_then(|v| v.checked_mul(buffers))
            .and_then(|n| {
                u64::try_from(size_of::<Self>())
                    .ok()
                    .and_then(|header| n.checked_add(header))
            })
            .ok_or(RecipeError::Overflow)?;
        if bytes > byte_cap {
            return Err(RecipeError::Allocation);
        }
        let mut budget = crate::memory::Budget::new(byte_cap);
        budget.charge(size_of::<Self>())?;
        let length = usize::try_from(n).map_err(|_| RecipeError::Overflow)?;
        let coefficient = case
            .inputs
            .first()
            .and_then(|array| array.first())
            .map(|bits| f64::from_bits(bits.0));
        let coefficient = match case.operation {
            Operation::Write | Operation::FusedStream => coefficient
                .filter(|value| value.is_normal())
                .ok_or(RecipeError::Domain)?,
            _ => 1.0,
        };
        let x = if case.operation == Operation::Write {
            Vec::new()
        } else {
            buffer(length, seed, 0, &mut budget)?
        };
        let y = if case.operation == Operation::FusedStream {
            buffer(length, seed, n, &mut budget)?
        } else {
            Vec::new()
        };
        let output = if case.operation == Operation::Read {
            Vec::new()
        } else {
            buffer(
                length,
                seed,
                n.checked_mul(2).ok_or(RecipeError::Overflow)?,
                &mut budget,
            )?
        };
        Ok(Self {
            operation: case.operation,
            width: case.width,
            coefficient,
            x,
            y,
            output,
        })
    }

    /// Restore mutable destinations outside timed kernel execution.
    pub(super) fn reset(&mut self) {
        self.output.fill(0.0);
    }

    /// Consume every pass before reusing output, with no per-element barriers.
    pub(super) fn execute(&mut self, passes: u64) -> Result<(), RecipeError> {
        let coefficient = black_box(self.coefficient);
        let x = black_box(self.x.as_slice());
        let y = black_box(self.y.as_slice());
        for _ in 0..passes {
            match self.operation {
                Operation::Read => {
                    let checksum = match self.width {
                        Width::Simd1 => read::<1>(x)?,
                        Width::Simd2 => read::<2>(x)?,
                        Width::Simd4 => read::<4>(x)?,
                        Width::Simd8 => read::<8>(x)?,
                        Width::Scalar | Width::BackendSelectedBinary64 => {
                            return Err(RecipeError::Domain);
                        }
                    };
                    if !checksum.is_finite() {
                        return Err(RecipeError::Domain);
                    }
                    black_box(checksum);
                }
                Operation::Write => {
                    self.output.fill(coefficient);
                    black_box(self.output.as_slice());
                }
                Operation::Copy => {
                    self.output.copy_from_slice(x);
                    black_box(self.output.as_slice());
                }
                Operation::FusedStream => {
                    for ((output, x), y) in self.output.iter_mut().zip(x).zip(y) {
                        *output = coefficient.mul_add(*x, *y);
                    }
                    black_box(self.output.as_slice());
                }
                _ => return Err(RecipeError::Domain),
            }
        }
        Ok(())
    }

    /// Full finite dry evaluation outside sampling.
    pub(super) fn validate(&mut self, passes: u64) -> Result<(), RecipeError> {
        self.execute(passes)?;
        if self.output.iter().any(|value| !value.is_finite()) {
            return Err(RecipeError::Domain);
        }
        self.reset();
        Ok(())
    }
}

/// Eight SIMD accumulators, scalar tail positions, then fixed accumulator-major reduction.
fn read<const N: usize>(values: &[f64]) -> Result<f64, RecipeError> {
    let width = 8 * N;
    if values.len() < width {
        return Err(RecipeError::Domain);
    }
    let mut partial = [Simd::<f64, N>::splat(0.0); 8];
    let mut chunks = values.chunks_exact(width);
    for chunk in &mut chunks {
        for (index, accumulator) in partial.iter_mut().enumerate() {
            *accumulator += Simd::from_array(std::array::from_fn(|lane| chunk[index * N + lane]));
        }
    }
    let mut partial = partial.map(Simd::to_array);
    for (position, value) in chunks.remainder().iter().enumerate() {
        partial[position / N][position % N] += *value;
    }
    let mut checksum = partial[0][0];
    for value in partial.into_iter().flatten().skip(1) {
        checksum += value;
    }
    Ok(checksum)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cancellation pairs occupy the first and third partial positions, independently of width.
    fn reduction<const N: usize>() -> Result<(), RecipeError> {
        let mut values = vec![1.0; 8 * N];
        values[0] = 1.0e16;
        values[2] = -1.0e16;
        values.extend([-1.0e16, 2.0, 1.0e16]);
        // Tail updates make partials 0,3,0 followed by K-3 ones, hence exactly K.
        let expected = f64::from(u32::try_from(8 * N).map_err(|_| RecipeError::Overflow)?);
        assert_eq!(read::<N>(&values)?.to_bits(), expected.to_bits());
        assert_eq!(read::<N>(&values[..8 * N - 1]), Err(RecipeError::Domain));
        Ok(())
    }

    #[test]
    fn accumulator_major_lane_minor_tail_reduction_is_fixed() -> Result<(), RecipeError> {
        reduction::<1>()?;
        reduction::<2>()?;
        reduction::<4>()?;
        reduction::<8>()
    }
}
