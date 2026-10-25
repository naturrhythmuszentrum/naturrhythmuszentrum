// SPDX-License-Identifier: AGPL-3.0-only
//! Integer-directed alternating sums, with a one-sided next-term remainder.

use super::{PiBounds, PiBudget, PiError};

/// Floor of the next scaled power and an enclosure of the finite partial sum.
#[derive(Clone, Copy, Debug)]
struct Series {
    /// Exact floor of `2^96 / q^(2n+1)`.
    power: i128,
    /// Lower partial-sum numerator over `2^96`.
    lower: i128,
    /// Upper partial-sum numerator over `2^96`.
    upper: i128,
    /// Exact positive squared divisor.
    square: i128,
}

impl Series {
    /// Only the admitted Machin divisors five and 239 are constructed below.
    const fn new(divisor: i128) -> Self {
        Self {
            power: (1_i128 << 96) / divisor,
            lower: 0,
            upper: 0,
            square: divisor * divisor,
        }
    }

    /// Nested integer division preserves the floor; adding one is a strict upper bound.
    fn advance(&mut self, index: u32) -> (i128, i128) {
        let term = self.power / i128::from(2 * index + 1);
        let positive = index.is_multiple_of(2);
        if positive {
            self.lower += term;
            self.upper += term + 1;
        } else {
            self.lower -= term + 1;
            self.upper -= term;
        }
        self.power /= self.square;
        let tail = self.power / i128::from(2 * index + 3) + 1;
        if positive {
            (self.lower - tail, self.upper)
        } else {
            (self.lower, self.upper + tail)
        }
    }
}

/// Fixed-size state; intersections retain every preceding certified bound.
#[derive(Debug)]
pub(super) struct State {
    /// The two independent arctangent partial sums.
    series: [Series; 2],
    /// Nested positive pi enclosure over the fixed dyadic denominator.
    bounds: PiBounds,
    /// Number of completed paired refinements.
    steps: u32,
}

impl State {
    /// The geometric seed `3 < pi < 4` requires no series step.
    pub(super) const fn new() -> Self {
        Self {
            series: [Series::new(5), Series::new(239)],
            bounds: PiBounds {
                lower: 3_u128 << 96,
                upper: 4_u128 << 96,
            },
            steps: 0,
        }
    }

    /// Both powers vanish within 21 steps; all signed numerators remain below `2^101`.
    pub(super) fn refine(&mut self, budget: &mut PiBudget) -> Result<(), PiError> {
        if self.series.iter().all(|value| value.power == 0) {
            return Err(PiError::ToleranceUnreachable);
        }
        budget.step()?;
        let (first_lower, first_upper) = self.series[0].advance(self.steps);
        let (second_lower, second_upper) = self.series[1].advance(self.steps);
        let lower = u128::try_from(16 * first_lower - 4 * second_upper)
            .map_err(|_| PiError::ToleranceUnreachable)?;
        let upper = u128::try_from(16 * first_upper - 4 * second_lower)
            .map_err(|_| PiError::ToleranceUnreachable)?;
        self.bounds.lower = self.bounds.lower.max(lower);
        self.bounds.upper = self.bounds.upper.min(upper);
        self.steps += 1;
        Ok(())
    }

    /// Current checked rational enclosure.
    pub(super) const fn bounds(&self) -> PiBounds {
        self.bounds
    }

    /// Paired terms evaluated by this request.
    pub(super) const fn steps(&self) -> u32 {
        self.steps
    }
}
