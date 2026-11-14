// SPDX-License-Identifier: AGPL-3.0-only
//! Runtime coefficient pairs and boundary-only consumption.

use crate::{
    CaseRequest, Dependency, Operation, Seed, Width,
    recipe::{RecipeError, input},
};
use std::{
    hint::black_box,
    simd::{Simd, StdFloat},
};

/// Prepared scalar lane state and two coefficient tuples per state.
#[derive(Clone, Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the crate-visible kernel enum retains this prepared state"
)]
pub(crate) struct Arithmetic {
    /// Scalar or logical SIMD width.
    width: Width,
    /// Named operation variant.
    operation: Operation,
    /// One or eight active states per lane.
    states: usize,
    /// Runtime-distinct lane seeds, accumulator-major then lane-minor.
    seeds: [[f64; 8]; 8],
    /// First coefficient in the alternating pair.
    a: [[f64; 8]; 8],
    /// Rounded reciprocal of the first factor.
    inverse: [[f64; 8]; 8],
    /// Signed-add/FMA coefficients or first square-root offsets.
    b: [[f64; 8]; 8],
    /// Runtime addition/FMA coefficient from its distinct generator position.
    increment: [[f64; 8]; 8],
    /// Prepared multiplicand-dependent addend; scaling stays outside timing.
    scaled_b: [[f64; 8]; 8],
    /// Second distinct square-root offsets.
    c: [[f64; 8]; 8],
}

impl Arithmetic {
    /// Fixed storage evidence captured after allocation and before any item timer.
    pub(super) fn layout(&self) -> Result<[Option<crate::layout::BufferLayout>; 8], RecipeError> {
        use crate::layout::BufferRole;
        let mut out = [None; 8];
        for (slot, values) in out.iter_mut().zip([
            &self.seeds,
            &self.a,
            &self.inverse,
            &self.b,
            &self.increment,
            &self.scaled_b,
            &self.c,
        ]) {
            *slot = Some(crate::layout::BufferLayout {
                role: BufferRole::State,
                capacity_bytes: 512,
                live_bytes: u64::try_from(self.states).map_err(|_| RecipeError::Overflow)?
                    * crate::recipe::lanes(self.width)
                    * 8,
                address_modulo_64: u64::try_from(values.as_ptr().addr() % 64)
                    .map_err(|_| RecipeError::Overflow)?,
                row_stride_bytes: 8,
                column_stride_bytes: 64,
            });
        }
        Ok(out)
    }

    /// Prepare distinct runtime inputs without timing or machine assumptions.
    pub(super) fn prepare(case: &CaseRequest, seed: Seed) -> Result<Self, RecipeError> {
        let states = match case.dependency {
            Dependency::Dependent => 1,
            Dependency::Independent => 8,
            Dependency::MultiAccumulatorReduction
            | Dependency::IndependentElements
            | Dependency::BackendKernel
            | Dependency::PreparedTraversal => return Err(RecipeError::Domain),
        };
        let mut out = Self {
            width: case.width,
            operation: case.operation,
            states,
            seeds: [[0.0; 8]; 8],
            a: [[0.0; 8]; 8],
            inverse: [[0.0; 8]; 8],
            b: [[0.0; 8]; 8],
            increment: [[0.0; 8]; 8],
            scaled_b: [[0.0; 8]; 8],
            c: [[0.0; 8]; 8],
        };
        let width =
            usize::try_from(crate::recipe::lanes(case.width)).map_err(|_| RecipeError::Overflow)?;
        for state in 0..states {
            for lane in 0..width {
                let position =
                    u64::try_from((state * width + lane) * 4).map_err(|_| RecipeError::Overflow)?;
                out.seeds[state][lane] = input(seed, position)?;
                let coefficient = input(seed, position + 1)?;
                out.increment[state][lane] = coefficient / 16_777_216.0;
                out.a[state][lane] = 1.0 + coefficient / 1_048_576.0;
                out.inverse[state][lane] = 1.0 / out.a[state][lane];
                out.b[state][lane] = input(seed, position + 2)?;
                out.c[state][lane] = input(seed, position + 3)?;
                out.scaled_b[state][lane] = out.b[state][lane] / 16_777_216.0;
            }
        }
        Ok(out)
    }

    /// Retain every active seed and both exact coefficient tuples without source rounding.
    fn inputs(&self) -> Result<crate::recipe::ArithmeticInputs, RecipeError> {
        let width =
            usize::try_from(crate::recipe::lanes(self.width)).map_err(|_| RecipeError::Overflow)?;
        let fields = if matches!(
            self.operation,
            Operation::FmaAddend | Operation::FmaMultiplicand | Operation::MultiplyAdd
        ) {
            2
        } else {
            1
        };
        let count = self
            .states
            .checked_mul(width)
            .ok_or(RecipeError::Overflow)?;
        let bytes = count
            .checked_mul(1 + 2 * fields)
            .and_then(|n| n.checked_mul(size_of::<crate::Binary64>()))
            .ok_or(RecipeError::Overflow)?;
        let mut budget =
            crate::memory::Budget::new(u64::try_from(bytes).map_err(|_| RecipeError::Overflow)?);
        let mut states = budget.reserve(count)?;
        let mut pair = [
            budget.reserve(count * fields)?,
            budget.reserve(count * fields)?,
        ];
        for state in 0..self.states {
            for lane in 0..width {
                states.push(crate::Binary64(self.seeds[state][lane].to_bits()));
                let tuples = self.tuples(state, lane)?;
                for (values, tuple) in pair.iter_mut().zip(tuples) {
                    values.extend(
                        tuple[..fields]
                            .iter()
                            .map(|value| crate::Binary64(value.to_bits())),
                    );
                }
            }
        }
        Ok(crate::recipe::ArithmeticInputs {
            states,
            pair,
            tuple_fields: u64::try_from(fields).map_err(|_| RecipeError::Overflow)?,
        })
    }

    /// Operation-specific tuple order is the same order used by the timed recurrence.
    fn tuples(&self, state: usize, lane: usize) -> Result<[[f64; 2]; 2], RecipeError> {
        let a = self.a[state][lane];
        let inverse = self.inverse[state][lane];
        let b = self.b[state][lane];
        let increment = self.increment[state][lane];
        let scaled = self.scaled_b[state][lane];
        Ok(match self.operation {
            Operation::Add => [[increment, 0.0], [-increment, 0.0]],
            Operation::Multiply | Operation::Divide => [[a, 0.0], [inverse, 0.0]],
            Operation::FmaAddend => [[increment, b], [increment, -b]],
            Operation::FmaMultiplicand | Operation::MultiplyAdd => {
                [[a, scaled], [inverse, -scaled]]
            }
            Operation::SquareRoot => [[b, 0.0], [self.c[state][lane], 0.0]],
            _ => return Err(RecipeError::Domain),
        })
    }

    /// Dry-check the complete finite item outside timing.
    pub(super) fn validate(&self, length: u64) -> Result<(), RecipeError> {
        self.dispatch::<true>(length)
    }

    /// Execute a fixed-length item and consume its complete final state.
    pub(super) fn execute(&self, length: u64) -> Result<(), RecipeError> {
        self.dispatch::<false>(length)
    }

    /// Static operation specialization keeps enum dispatch outside the item loop.
    fn dispatch<const CHECK: bool>(&self, length: u64) -> Result<(), RecipeError> {
        match self.operation {
            Operation::Add => self.width::<0, CHECK>(length),
            Operation::Multiply => self.width::<1, CHECK>(length),
            Operation::Divide => self.width::<2, CHECK>(length),
            Operation::FmaAddend => self.width::<3, CHECK>(length),
            Operation::FmaMultiplicand => self.width::<4, CHECK>(length),
            Operation::MultiplyAdd => self.width::<5, CHECK>(length),
            Operation::SquareRoot => self.width::<6, CHECK>(length),
            _ => Err(RecipeError::Domain),
        }
    }

    /// Static width specializations retain separate scalar and SIMD identities.
    fn width<const OP: u8, const CHECK: bool>(&self, length: u64) -> Result<(), RecipeError> {
        match self.width {
            Width::Scalar => self.scalar::<OP, CHECK>(length),
            Width::Simd1 => self.vector::<1, OP, CHECK>(length),
            Width::Simd2 => self.vector::<2, OP, CHECK>(length),
            Width::Simd4 => self.vector::<4, OP, CHECK>(length),
            Width::Simd8 => self.vector::<8, OP, CHECK>(length),
            Width::BackendSelectedBinary64 => Err(RecipeError::Domain),
        }
    }

    /// Scalar item with an unrolled alternating coefficient pair.
    #[expect(
        clippy::suboptimal_flops,
        reason = "the separate multiply/add recipe requires separate rounding"
    )]
    fn scalar<const OP: u8, const CHECK: bool>(&self, length: u64) -> Result<(), RecipeError> {
        let mut states = black_box(self.seeds.map(|row| row[0]));
        let a = black_box(self.a.map(|row| row[0]));
        let inverse = black_box(self.inverse.map(|row| row[0]));
        let b = black_box(self.b.map(|row| row[0]));
        let scaled_b = black_box(self.scaled_b.map(|row| row[0]));
        let c = black_box(self.c.map(|row| row[0]));
        let increment = black_box(self.increment.map(|row| row[0]));
        for _ in 0..length / 2 {
            for state in 0..self.states {
                let before = states[state];
                let first = match OP {
                    0 => before + increment[state],
                    1 => before * a[state],
                    2 => before / a[state],
                    3 => increment[state].mul_add(b[state], before),
                    4 => a[state].mul_add(before, scaled_b[state]),
                    5 => a[state] * before + scaled_b[state],
                    6 => (before + b[state]).sqrt(),
                    _ => return Err(RecipeError::Domain),
                };
                let second = match OP {
                    0 => first + -increment[state],
                    1 => first * inverse[state],
                    2 => first / inverse[state],
                    3 => increment[state].mul_add(-b[state], first),
                    4 => inverse[state].mul_add(first, -scaled_b[state]),
                    5 => inverse[state] * first + -scaled_b[state],
                    6 => (first + c[state]).sqrt(),
                    _ => return Err(RecipeError::Domain),
                };
                if CHECK {
                    validate_cycle(before, first, second)?;
                }
                states[state] = second;
            }
        }
        if length % 2 != 0 {
            for state in 0..self.states {
                let x = states[state];
                states[state] = match OP {
                    0 => x + increment[state],
                    1 => x * a[state],
                    2 => x / a[state],
                    3 => increment[state].mul_add(b[state], x),
                    4 => a[state].mul_add(x, scaled_b[state]),
                    5 => a[state] * x + scaled_b[state],
                    6 => (x + b[state]).sqrt(),
                    _ => return Err(RecipeError::Domain),
                };
                if CHECK && !states[state].is_normal() {
                    return Err(RecipeError::Domain);
                }
            }
        }
        black_box(states);
        Ok(())
    }
    /// Explicit logical SIMD item with an unrolled alternating coefficient pair.
    fn vector<const N: usize, const OP: u8, const CHECK: bool>(
        &self,
        length: u64,
    ) -> Result<(), RecipeError> {
        let load = |rows: [[f64; 8]; 8]| {
            rows.map(|row| Simd::<f64, N>::from_array(std::array::from_fn(|i| row[i])))
        };
        let mut states = black_box(load(self.seeds));
        let a = black_box(load(self.a));
        let inverse = black_box(load(self.inverse));
        let b = black_box(load(self.b));
        let scaled_b = black_box(load(self.scaled_b));
        let c = black_box(load(self.c));
        let increment = black_box(load(self.increment));
        for _ in 0..length / 2 {
            for state in 0..self.states {
                let before = states[state];
                let first = match OP {
                    0 => before + increment[state],
                    1 => before * a[state],
                    2 => before / a[state],
                    3 => increment[state].mul_add(b[state], before),
                    4 => a[state].mul_add(before, scaled_b[state]),
                    5 => a[state] * before + scaled_b[state],
                    6 => (before + b[state]).sqrt(),
                    _ => return Err(RecipeError::Domain),
                };
                let second = match OP {
                    0 => first + -increment[state],
                    1 => first * inverse[state],
                    2 => first / inverse[state],
                    3 => increment[state].mul_add(-b[state], first),
                    4 => inverse[state].mul_add(first, -scaled_b[state]),
                    5 => inverse[state] * first + -scaled_b[state],
                    6 => (first + c[state]).sqrt(),
                    _ => return Err(RecipeError::Domain),
                };
                if CHECK {
                    for ((before, first), second) in before
                        .to_array()
                        .into_iter()
                        .zip(first.to_array())
                        .zip(second.to_array())
                    {
                        validate_cycle(before, first, second)?;
                    }
                }
                states[state] = second;
            }
        }
        if length % 2 != 0 {
            for state in 0..self.states {
                let x = states[state];
                states[state] = match OP {
                    0 => x + increment[state],
                    1 => x * a[state],
                    2 => x / a[state],
                    3 => increment[state].mul_add(b[state], x),
                    4 => a[state].mul_add(x, scaled_b[state]),
                    5 => a[state] * x + scaled_b[state],
                    6 => (x + b[state]).sqrt(),
                    _ => return Err(RecipeError::Domain),
                };
                if CHECK
                    && states[state]
                        .to_array()
                        .iter()
                        .any(|value| !value.is_normal())
                {
                    return Err(RecipeError::Domain);
                }
            }
        }
        black_box(states);
        Ok(())
    }
}

/// A finite-normal alternating cycle may be periodic but may not be permanently fixed.
const fn validate_cycle(before: f64, first: f64, second: f64) -> Result<(), RecipeError> {
    let finite = first.is_normal() && second.is_normal();
    let fixed = before.to_bits() == first.to_bits() && first.to_bits() == second.to_bits();
    if !finite || fixed {
        return Err(RecipeError::Domain);
    }
    Ok(())
}

/// Reconstruct the revisioned prepared coefficients for raw evidence outside timing.
pub(super) fn inputs(
    case: &CaseRequest,
    seed: Seed,
) -> Result<Option<crate::recipe::ArithmeticInputs>, RecipeError> {
    if !matches!(
        case.dependency,
        Dependency::Dependent | Dependency::Independent
    ) {
        return Ok(None);
    }
    Arithmetic::prepare(case, seed)?.inputs().map(Some)
}

#[cfg(test)]
mod tests;
