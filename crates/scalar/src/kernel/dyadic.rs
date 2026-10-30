// SPDX-License-Identifier: AGPL-3.0-only
//! Exact integer export of every finite binary32 or binary64 endpoint.

use crate::{Enclosure, Finite, Scalar};

/// The exact rational `significand * 2^exponent`, with no floating payload.
///
/// Exported nonzero significands are odd and have at most 53 magnitude bits.
/// Zero has significand and exponent zero. Consumers check these integer data
/// independently; a finite machine value is never an exact-checker premise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dyadic {
    /// Signed odd coefficient, or zero.
    pub significand: i64,
    /// Signed binary exponent, including subnormal endpoints.
    pub exponent: i16,
}

impl Dyadic {
    /// Decode after the caller prepays the fixed endpoint schedule.
    pub(super) fn from_finite<P: Scalar>(value: Finite<P>) -> Self {
        let bits = value.get().to_f64().to_bits();
        let encoded = (bits >> 52) & 0x7ff;
        let fraction = bits & 0x000f_ffff_ffff_ffff;
        let (significand, exponent) = if encoded == 0 {
            (fraction, -1074)
        } else {
            (
                fraction | (1 << 52),
                i16::try_from(encoded).expect("finite binary exponent") - 1075,
            )
        };
        if significand == 0 {
            return Self {
                significand: 0,
                exponent: 0,
            };
        }
        let shift = significand.trailing_zeros();
        let magnitude = i64::try_from(significand >> shift).expect("53-bit significand");
        Self {
            significand: if bits >> 63 == 0 {
                magnitude
            } else {
                -magnitude
            },
            exponent: exponent + i16::try_from(shift).expect("53-bit significand shift"),
        }
    }
}

/// A closed exact dyadic interval describing the complete covered real set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DyadicInterval {
    /// Included lower endpoint.
    pub lower: Dyadic,
    /// Included upper endpoint.
    pub upper: Dyadic,
}

impl DyadicInterval {
    /// Export both admitted endpoints without evaluating a rounded difference or sum.
    pub(super) fn from_enclosure<P: Scalar>(value: Enclosure<P>) -> Self {
        Self {
            lower: Dyadic::from_finite(value.lower()),
            upper: Dyadic::from_finite(value.upper()),
        }
    }
}
