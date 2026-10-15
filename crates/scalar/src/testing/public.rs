// SPDX-License-Identifier: AGPL-3.0-only
//! Independent binary decoding and rational expectations, used only by tests.

use nrz_scalar::{ArithmeticFailure, Enclosure, Finite, Scalar};
use num_bigint::BigInt;
use num_rational::Ratio;
use num_traits::{One, Signed, Zero};

/// Exact rational type without production arithmetic or CAS calls.
pub(crate) type Rational = Ratio<BigInt>;
/// Fallible numerical test result.
pub(crate) type TestResult = Result<(), ArithmeticFailure>;

/// Independent decoding for each primitive format.
pub(crate) trait Binary: Scalar {
    /// Decode the sign, exponent and significand into an exact rational.
    fn rational(self) -> Rational;
    /// Lower edge of the rounding cell whose outward neighbour is infinite.
    fn overflow_threshold() -> Rational;
}

impl Binary for f32 {
    fn overflow_threshold() -> Rational {
        (Self::from_bits(0x7f7f_ffff).rational() + Self::from_bits(0x7f7f_fffe).rational())
            / BigInt::from(2)
    }
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
    fn overflow_threshold() -> Rational {
        (Self::from_bits(0x7fef_ffff_ffff_ffff).rational()
            + Self::from_bits(0x7fef_ffff_ffff_fffe).rational())
            / BigInt::from(2)
    }
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

/// Reconstruct a binary rational using integer powers of two.
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

/// An input constructor, never used to calculate an expected value.
pub(crate) fn point<P: Scalar>(value: P) -> Result<Enclosure<P>, ArithmeticFailure> {
    Ok(Enclosure::singleton(Finite::new(value)?))
}

/// Compare decoded returned endpoints against independently computed truth.
pub(crate) fn contains<P: Binary>(result: Enclosure<P>, exact: &Rational) {
    let lower = result.lower().get().rational();
    let upper = result.upper().get().rational();
    assert!(
        lower <= *exact && *exact <= upper,
        "{lower} <= {exact} <= {upper}"
    );
}

/// A successful result contains truth; overflow is allowed only at a range boundary.
pub(crate) fn contains_or_overflows<P: Binary>(
    result: Result<Enclosure<P>, ArithmeticFailure>,
    exact: &Rational,
) -> TestResult {
    match result {
        Ok(value) => contains(value, exact),
        Err(ArithmeticFailure::Overflow) => {
            let threshold = P::overflow_threshold();
            assert!(exact.abs() >= threshold, "unexpected overflow for {exact}");
        }
        Err(error) => return Err(error),
    }
    Ok(())
}

/// Check square-root inclusion with rational squared endpoints.
pub(crate) fn root_contains<P: Binary>(result: Enclosure<P>, exact: &Rational) {
    let lower = result.lower().get().rational();
    let upper = result.upper().get().rational();
    assert!(lower >= Rational::zero());
    assert!(&lower * &lower <= *exact);
    assert!(&upper * &upper >= *exact);
}
