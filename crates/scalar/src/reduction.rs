// SPDX-License-Identifier: AGPL-3.0-only
//! Increasing-index reductions and scaled Euclidean/Frobenius norm bounds.

use crate::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite, Scalar};

/// Enclose a plain slice's exact real sum by a left fold in increasing index order.
///
/// The initial accumulator and the empty result are exactly zero. This order is
/// part of the contract; an overflowing intermediate refuses even if the final
/// real sum is finite.
///
/// # Errors
/// Refuses a nonfinite input or an overflowing intermediate enclosure.
pub fn sum<P: Scalar>(
    values: &[P],
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, ArithmeticFailure> {
    let mut total = Enclosure::singleton(Finite::ZERO);
    for &value in values {
        total = total.add(Enclosure::singleton(Finite::new(value)?), context)?;
    }
    Ok(total)
}

/// Enclose an exact dot product by increasing-index multiply-then-add left fold.
///
/// Each product is enclosed before addition; no implicit fusion or reassociation
/// occurs. Empty equal-length inputs return an exact zero.
///
/// # Errors
/// Refuses unequal lengths, nonfinite input or an overflowing intermediate.
pub fn dot<P: Scalar>(
    left: &[P],
    right: &[P],
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, ArithmeticFailure> {
    if left.len() != right.len() {
        return Err(ArithmeticFailure::LengthMismatch);
    }
    let mut total = Enclosure::singleton(Finite::ZERO);
    for (&first, &second) in left.iter().zip(right) {
        let lhs = Enclosure::singleton(Finite::new(first)?);
        let rhs = Enclosure::singleton(Finite::new(second)?);
        total = total.add(lhs.mul(rhs, context)?, context)?;
    }
    Ok(total)
}

/// Bound the Euclidean norm, or Frobenius norm of all supplied logical entries.
///
/// Choose a positive normal power of two from the greatest magnitude. Enclose
/// every division, square, increasing-index sum, square root and multiplication
/// back. Scaling is enclosed even at underflow. Empty or all-zero data returns
/// zero. Callers must supply full logical multiplicities for structured data.
///
/// # Errors
/// Refuses nonfinite input or any unbounded intermediate or final result.
pub fn norm_upper<P: Scalar>(
    values: &[P],
    context: &ArithmeticContext,
) -> Result<Finite<P>, ArithmeticFailure> {
    let mut largest = Finite::<P>::ZERO;
    for &value in values {
        let magnitude = Finite::new(value)?.abs();
        if magnitude > largest {
            largest = magnitude;
        }
    }
    if largest == Finite::ZERO {
        return Ok(Finite::ZERO);
    }
    let scale = Enclosure::singleton(Finite::new(largest.get().power_of_two_scale())?);
    let mut total = Enclosure::singleton(Finite::ZERO);
    for &value in values {
        let scaled = Enclosure::singleton(Finite::new(value)?).div(scale, context)?;
        total = total.add(scaled.square(context)?, context)?;
    }
    // Each summand is nonnegative, even if an outward addition stepped below zero.
    let lower = if total.lower() < Finite::ZERO {
        P::ZERO
    } else {
        total.lower().get()
    };
    let nonnegative = Enclosure::new(lower, total.upper().get())?;
    Ok(nonnegative.sqrt(context)?.mul(scale, context)?.upper())
}
