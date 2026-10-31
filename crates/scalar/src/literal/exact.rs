// SPDX-License-Identifier: AGPL-3.0-only
//! Integer-only representability checks precede construction of an exact normal binary value.

use super::{DecodedLiteral, limits::Work, syntax::Decimal};
use crate::{ArithmeticFailure, Enclosure, Finite, Precision, Scalar};

/// The prepaid bounded candidate scan covers fixed-width parsing and Euclidean reduction.
/// Only a proved normal dyadic reaches floating construction; every other case keeps the old route.
pub(super) fn fraction<P: Scalar>(
    numerator: &Decimal<'_>,
    denominator: &Decimal<'_>,
    work: &mut Work,
) -> Result<Option<DecodedLiteral<P>>, ArithmeticFailure> {
    let Some((mut top, mut bottom)) = integer(numerator).zip(integer(denominator)) else {
        return Ok(None);
    };
    if bottom == 0 {
        return Ok(None);
    }
    if top == 0 {
        return finish(Finite::ZERO, work).map(Some);
    }
    let divisor = gcd(top, bottom);
    top /= divisor;
    bottom /= divisor;
    if !bottom.is_power_of_two() {
        return Ok(None);
    }
    let shift = top.trailing_zeros();
    let significand = top >> shift;
    let exponent = i64::from(shift) - i64::from(bottom.trailing_zeros());
    let (digits, minimum, maximum) = match P::PRECISION {
        Precision::F32 => (24, -126, 127),
        Precision::F64 => (53, -1022, 1023),
    };
    let highest = i64::from(significand.bit_width()) - 1 + exponent;
    if significand.bit_width() > digits || highest < minimum || highest > maximum {
        return Ok(None);
    }
    // The exact odd significand fits the selected format, independently of the native parser.
    let text = significand.to_string();
    let mut value = P::parse_nearest(&text).ok_or(ArithmeticFailure::InvalidLiteral)?;
    for _ in 0..exponent.unsigned_abs() {
        work.charge(1)?;
        value = if exponent < 0 {
            value / P::from_u8(2)
        } else {
            value * P::from_u8(2)
        };
    }
    if numerator.negative != denominator.negative {
        value = -value;
    }
    finish(Finite::new(value)?, work).map(Some)
}

/// Parse only effective integer significands; unsupported sizes use conservative evaluation.
fn integer(decimal: &Decimal<'_>) -> Option<u128> {
    if decimal.exponent != 0 {
        return None;
    }
    decimal
        .whole
        .bytes()
        .chain(decimal.fraction.bytes())
        .try_fold(0_u128, |value, digit| {
            value.checked_mul(10)?.checked_add(u128::from(digit - b'0'))
        })
}

/// Euclidean reduction uses only the already bounded unsigned integer operands.
const fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// A proved exact normal result needs no interval division or displacement widening.
fn finish<P: Scalar>(
    center: Finite<P>,
    work: &mut Work,
) -> Result<DecodedLiteral<P>, ArithmeticFailure> {
    work.charge(1)?;
    Ok(DecodedLiteral {
        center,
        enclosure: Enclosure::singleton(center),
        displacement: Finite::ZERO,
        work_used: work.used(),
    })
}
