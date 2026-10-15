// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact binary decoding for public numerical tests only.

use nrz_accuracy::{AccuracyFailure, Nonnegative};
use nrz_scalar::Scalar;
use num_bigint::BigInt;
use num_rational::Ratio;
use num_traits::One;

/// Independent exact-rational oracle type.
pub(crate) type Rational = Ratio<BigInt>;
/// Fallible public test result.
pub(crate) type TestResult = Result<(), AccuracyFailure>;

/// Binary decoding independent of production enclosure and norm operations.
pub(crate) trait Binary: Scalar {
    /// Recover a finite value's exact real rational from its encoding.
    fn rational(self) -> Rational;
}

impl Binary for f32 {
    fn rational(self) -> Rational {
        let bits = self.to_bits();
        let encoded = (bits >> 23) & 0xff;
        assert_ne!(encoded, 0xff);
        let fraction = u64::from(bits & 0x007f_ffff);
        let (significand, exponent) = if encoded == 0 {
            (fraction, -149)
        } else {
            (
                fraction | (1 << 23),
                i32::try_from(encoded).expect("eight bits") - 150,
            )
        };
        binary(bits >> 31 != 0, significand, exponent)
    }
}

impl Binary for f64 {
    fn rational(self) -> Rational {
        let bits = self.to_bits();
        let encoded = (bits >> 52) & 0x7ff;
        assert_ne!(encoded, 0x7ff);
        let fraction = bits & 0x000f_ffff_ffff_ffff;
        let (significand, exponent) = if encoded == 0 {
            (fraction, -1074)
        } else {
            (
                fraction | (1 << 52),
                i32::try_from(encoded).expect("eleven bits") - 1075,
            )
        };
        binary(bits >> 63 != 0, significand, exponent)
    }
}

/// Reconstruct a rational using integer shifts, without floating arithmetic.
fn binary(negative: bool, significand: u64, exponent: i32) -> Rational {
    let integer = BigInt::from(significand);
    let signed = if negative { -integer } else { integer };
    let shift = usize::try_from(exponent.unsigned_abs()).expect("IEEE exponent");
    if exponent >= 0 {
        Ratio::from_integer(signed << shift)
    } else {
        Ratio::new(signed, BigInt::one() << shift)
    }
}

/// Compare a returned upper magnitude against independent exact truth.
pub(crate) fn covers<P: Binary>(result: Nonnegative<P>, exact: &Rational) {
    let upper = result.get().rational();
    assert!(upper >= *exact, "{upper} must cover {exact}");
}
