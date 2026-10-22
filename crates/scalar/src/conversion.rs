// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit precision changes with independently outward displacement bounds.
//!
//! # Coverage through precision changes
//!
//! `widen` converts a binary32 center and enclosure exactly to binary64, preserving their existing coverage. `narrow` range-checks a binary64 center, rounds it to nearest binary32, and converts each endpoint outward by comparing its exact widening with the source. `PrecisionChange` exposes the new representation and an outward destination-format bound on additional center displacement. Existing error remains the caller's obligation. `Enclosure::displacement_from` bounds the distance of a finite candidate from every point in an independently obtained enclosure.
//!
//! Conversion and literal tests independently decode returned endpoints into rationals, covering exact widening, outward narrowing, halfway ties, subnormal loss, underflow to zero, common descriptor constants, syntax/resource/range refusals and an import-to-narrowing-to-widening lifecycle. The lifecycle retains import error and conversion displacement as separate numerical contributions.

use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite, Scalar};

/// A new representation and the additional displacement of its center.
///
/// The converted enclosure contains the original enclosure, independently of
/// the center. `displacement` bounds only the change from the original center;
/// the caller must retain and outward-convert all pre-existing error coverage.
#[derive(Debug, Clone, Copy)]
pub struct PrecisionChange<P: Scalar, Q: Scalar> {
    /// Original represented center, identifying the displacement's reference.
    source_center: Finite<P>,
    /// New represented center.
    center: Finite<Q>,
    /// Exact widening or outward narrowing of the source enclosure.
    enclosure: Enclosure<Q>,
    /// Outward upper bound in the destination precision.
    displacement: Finite<Q>,
}

impl<P: Scalar, Q: Scalar> PrecisionChange<P, Q> {
    /// The original represented center.
    #[must_use]
    pub const fn source_center(self) -> Finite<P> {
        self.source_center
    }
    /// The destination center.
    #[must_use]
    pub const fn center(self) -> Finite<Q> {
        self.center
    }
    /// The converted enclosure, preserving all its old coverage.
    #[must_use]
    pub const fn enclosure(self) -> Enclosure<Q> {
        self.enclosure
    }
    /// Additional center displacement, rounded upward in destination precision.
    #[must_use]
    pub const fn displacement(self) -> Finite<Q> {
        self.displacement
    }
}

/// Widen a center and both enclosure endpoints exactly, with zero new displacement.
#[must_use]
pub fn widen(center: Finite<f32>, enclosure: Enclosure<f32>) -> PrecisionChange<f32, f64> {
    PrecisionChange {
        source_center: center,
        center: center.widen(),
        enclosure: enclosure.widen(),
        displacement: Finite::ZERO,
    }
}

/// Narrow a center to nearest binary32 and its enclosure endpoints outward.
///
/// Widen each rounded endpoint exactly for comparison, stepping only when it
/// would exclude its source. Compute center displacement outward in binary64,
/// then round that upper bound upward to binary32, including subnormal loss.
///
/// # Errors
/// Refuses source values outside binary32's finite range or nonfinite bounds.
pub fn narrow(
    center: Finite<f64>,
    enclosure: Enclosure<f64>,
    context: &ArithmeticContext,
) -> Result<PrecisionChange<f64, f32>, ArithmeticFailure> {
    let rounded = Finite::new(nearest(center.get())?)?;
    let lower = endpoint(enclosure.lower().get(), false)?;
    let upper = endpoint(enclosure.upper().get(), true)?;
    let converted = Enclosure::new(lower.get(), upper.get())?;
    let delta = Enclosure::singleton(center).displacement_from(rounded.widen(), context)?;
    let displacement = endpoint(delta.get(), true)?;
    Ok(PrecisionChange {
        source_center: center,
        center: rounded,
        enclosure: converted,
        displacement,
    })
}

/// Range-checked nearest conversion, never used as an outward bound alone.
fn nearest(value: f64) -> Result<f32, ArithmeticFailure> {
    if value.abs() > f64::from(f32::MAX) {
        return Err(ArithmeticFailure::Overflow);
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "range checked; callers enclose displacement"
    )]
    let converted = value as f32;
    Ok(converted)
}

/// Directed endpoint conversion by exact comparison after nearest rounding.
fn endpoint(value: f64, upward: bool) -> Result<Finite<f32>, ArithmeticFailure> {
    let converted = nearest(value)?;
    let widened = f64::from(converted);
    let adjusted = if upward && widened < value {
        converted.next_up()
    } else if !upward && widened > value {
        converted.next_down()
    } else {
        converted
    };
    Finite::new(adjusted).map_err(|_| ArithmeticFailure::Overflow)
}
