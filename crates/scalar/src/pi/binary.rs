// SPDX-License-Identifier: AGPL-3.0-only
//! Directed quantization of positive dyadic bounds without a decimal import.

use super::{PiBounds, PiBudget, PiError};
use crate::{Enclosure, Precision, Scalar};

/// Quantize each rational endpoint separately, including any outward carry.
pub(super) fn enclose<P: Scalar>(
    bounds: PiBounds,
    budget: &mut PiBudget,
) -> Result<Enclosure<P>, PiError> {
    Ok(Enclosure::new(
        endpoint::<P>(bounds.lower, false, budget)?,
        endpoint::<P>(bounds.upper, true, budget)?,
    )?)
}

/// Each intermediate integer has at most the format's significant digits;
/// the possible final carry is an exactly represented power of two. Scaling
/// uses only normal powers of two, so every floating operation here is exact.
fn endpoint<P: Scalar>(numerator: u128, upward: bool, budget: &mut PiBudget) -> Result<P, PiError> {
    let digits = match P::PRECISION {
        Precision::F32 => 24,
        Precision::F64 => 53,
    };
    let shift = numerator.bit_width().saturating_sub(digits);
    budget.work(22 + u64::from(96 - shift))?;
    let mask = (1_u128 << shift) - 1;
    let significand = (numerator >> shift) + u128::from(upward && numerator & mask != 0);
    let bytes = u64::try_from(significand)
        .map_err(|_| PiError::ToleranceUnreachable)?
        .to_be_bytes();
    let radix = P::from_u8(16) * P::from_u8(16);
    let mut value = P::ZERO;
    for byte in bytes {
        value = value * radix + P::from_u8(byte);
    }
    for _ in shift..96 {
        value = value / P::from_u8(2);
    }
    Ok(value)
}
