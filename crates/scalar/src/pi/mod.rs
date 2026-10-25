// SPDX-License-Identifier: AGPL-3.0-only
//! Certified mathematical pi with bounded rational refinement and atomic usage.
//!
//! # Constant theorem
//!
//! Revision one uses `pi = 16 atan(1/5) - 4 atan(1/239)` and the geometric seed
//! `3 < pi < 4`. The tangent double-angle formula gives `tan(4 atan(1/5)) =
//! 120/119`; subtracting `atan(1/239)` gives tangent one in the first quadrant.
//! The alternating arctangent series bounds its remainder by the next term,
//! with that term's sign. Integer floors and ceilings enclose every term over
//! `2^96`. Repeated integer division preserves the exact floor of each power.
//! Intersecting with earlier bounds preserves containment and nesting.
//!
//! Directed binary quantization then encloses these checked rational bounds.
//! Its fixed significands and normal power-of-two scaling are exact. The lower
//! endpoint is the candidate; outward displacement bounds the entire interval.
//! The requested tolerance bounds that displacement, including quantization.
//! This is a fixed-resolution admitted algorithm, not arbitrary precision or
//! an evaluator of supplied expressions. Exhausted refinement, format precision
//! or caller resources refuse without publishing a result or charging usage.
//!
//! # Resource contract
//!
//! Work prepays five initialization units, 26 per paired refinement, each
//! endpoint's 22-unit integer/import schedule plus one unit per exact binary
//! scaling, and one outward displacement per tolerance check. These are fixed
//! schedule allowances, not processor instruction counts. The explicit caller
//! ledger accumulates successful work, steps and output bytes; staging is a
//! high-water mark of logical state plus result. There is no heap allocation,
//! hidden default, cache or physical interpretation. See (´rule:scalar:pi-policy´).

mod binary;
mod budget;
mod series;

use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite, Scalar};
pub use budget::{PiBudget, PiLimits, PiUsage};

/// A refused constant request; the caller's ledger remains unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PiError {
    /// A strictly positive finite absolute displacement tolerance is required.
    #[error("pi tolerance must be positive")]
    InvalidTolerance,
    /// The fixed rational or binary resolution cannot satisfy this request.
    #[error("pi tolerance is below the admitted resolution")]
    ToleranceUnreachable,
    /// Cumulative numerical work is exhausted.
    #[error("pi work limit exceeded")]
    WorkLimit,
    /// Cumulative paired series refinements are exhausted.
    #[error("pi refinement step limit exceeded")]
    StepLimit,
    /// Logical scratch plus returned result exceeds the staging allowance.
    #[error("pi staged byte limit exceeded")]
    StagedBytesLimit,
    /// Cumulative logical result bytes exceed the output allowance.
    #[error("pi output byte limit exceeded")]
    OutputBytesLimit,
    /// The supported scalar arithmetic refused.
    #[error("pi arithmetic: {0}")]
    Arithmetic(#[from] ArithmeticFailure),
}

/// Checked positive rational pi bounds, both over the exact denominator `2^96`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PiBounds {
    /// Lower integer numerator, constructed only by the admitted theorem.
    lower: u128,
    /// Upper integer numerator, constructed only by the admitted theorem.
    upper: u128,
}

impl PiBounds {
    /// Lower numerator over `2^PiBounds::denominator_power()`.
    #[must_use]
    pub const fn lower_numerator(self) -> u128 {
        self.lower
    }

    /// Upper numerator over `2^PiBounds::denominator_power()`.
    #[must_use]
    pub const fn upper_numerator(self) -> u128 {
        self.upper
    }

    /// Exact dyadic denominator exponent of this constant theorem.
    #[must_use]
    pub const fn denominator_power() -> u32 {
        96
    }
}

/// One complete certified constant with rational evidence and numerical displacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PiEnclosure<P: Scalar> {
    /// Mathematical rational bounds before binary quantization.
    rational: PiBounds,
    /// Outward binary endpoints enclosing the entire rational interval.
    enclosure: Enclosure<P>,
    /// Candidate displacement, including all endpoint quantization.
    displacement: Finite<P>,
    /// Successful work belonging to this request alone.
    work: u64,
    /// Successful paired refinements belonging to this request alone.
    steps: u32,
    /// Admitted constant theorem revision, separate from the arithmetic context.
    revision: u32,
}

impl<P: Scalar> PiEnclosure<P> {
    /// Refine the fixed mathematical pi definition to an absolute displacement tolerance.
    ///
    /// A successful result and its cumulative usage commit together. Existing
    /// usage is preserved on every error, including late refinement exhaustion.
    /// # Errors
    /// Refuses nonpositive tolerance, unavailable precision, any explicit resource
    /// limit or scalar arithmetic failure. A positive tolerance does not guarantee
    /// success in the chosen format or this fixed-resolution rational algorithm.
    pub fn new(
        tolerance: Finite<P>,
        context: &ArithmeticContext,
        budget: &mut PiBudget,
    ) -> Result<Self, PiError> {
        if tolerance <= Finite::ZERO {
            return Err(PiError::InvalidTolerance);
        }
        let mut staged = *budget;
        staged.storage(size_of::<series::State>(), size_of::<Self>())?;
        staged.work(5)?;
        let mut state = series::State::new();
        loop {
            let enclosure = binary::enclose::<P>(state.bounds(), &mut staged)?;
            staged.work(1)?;
            let displacement = enclosure.displacement_from(enclosure.lower(), context)?;
            if displacement <= tolerance {
                let result = Self {
                    rational: state.bounds(),
                    enclosure,
                    displacement,
                    work: staged.usage().work - budget.usage().work,
                    steps: state.steps(),
                    revision: 1,
                };
                *budget = staged;
                return Ok(result);
            }
            state.refine(&mut staged)?;
        }
    }

    /// Checked rational bounds before any binary import.
    #[must_use]
    pub const fn rational_bounds(self) -> PiBounds {
        self.rational
    }

    /// Finite binary enclosure of the mathematical constant.
    #[must_use]
    pub const fn enclosure(self) -> Enclosure<P> {
        self.enclosure
    }

    /// Candidate chosen from the lower binary endpoint; no correct-rounding claim.
    #[must_use]
    pub const fn center(self) -> Finite<P> {
        self.enclosure.lower()
    }

    /// Outward numerical displacement from the fixed mathematical pi definition.
    #[must_use]
    pub const fn displacement(self) -> Finite<P> {
        self.displacement
    }

    /// Charged numerical work for this request, independent of prior ledger usage.
    #[must_use]
    pub const fn work_used(self) -> u64 {
        self.work
    }

    /// Paired Machin-series refinements used by this request; the seed uses zero.
    #[must_use]
    pub const fn steps_used(self) -> u32 {
        self.steps
    }

    /// Constant theorem revision; consumers retain it beside the arithmetic revision.
    #[must_use]
    pub const fn revision(self) -> u32 {
        self.revision
    }
}
