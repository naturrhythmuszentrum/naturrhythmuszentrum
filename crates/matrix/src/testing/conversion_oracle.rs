// SPDX-License-Identifier: AGPL-3.0-only
//! Independent bounded integer fractions and IEEE decoding for conversion test expectations.

use super::conversion_testing::TestResult;

/// Reduced rational truth, used only by small exact test fixtures.
#[derive(Clone, Copy, Debug)]
struct Fraction {
    /// Signed reduced exact numerator.
    numerator: i128,
    /// Positive reduced exact denominator.
    denominator: i128,
}

impl Fraction {
    /// Reduce signed numerator and positive denominator before arithmetic.
    fn new(numerator: i128, denominator: i128) -> TestResult<Self> {
        if denominator <= 0 {
            return Err("invalid oracle denominator".into());
        }
        let divisor = gcd(numerator.abs(), denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }
    /// Decode IEEE sign, exponent and significand directly, without scalar operations.
    fn binary(value: f64) -> TestResult<Self> {
        let bits = value.to_bits();
        let exponent = i32::try_from((bits >> 52) & 2047)?;
        if exponent == 2047 {
            return Err("nonfinite oracle input".into());
        }
        let mantissa = i128::from(bits & ((1_u64 << 52) - 1));
        let magnitude = if exponent == 0 {
            mantissa
        } else {
            mantissa + (1_i128 << 52)
        };
        if magnitude == 0 {
            return Self::new(0, 1);
        }
        let signed = if bits >> 63 == 0 {
            magnitude
        } else {
            -magnitude
        };
        let power = if exponent == 0 {
            -1074
        } else {
            exponent - 1075
        };
        if power >= 0 {
            Self::new(
                signed
                    .checked_mul(
                        1_i128
                            .checked_shl(u32::try_from(power)?)
                            .ok_or("oracle range")?,
                    )
                    .ok_or("oracle overflow")?,
                1,
            )
        } else {
            Self::new(
                signed,
                1_i128
                    .checked_shl(u32::try_from(-power)?)
                    .ok_or("oracle range")?,
            )
        }
    }
    /// Exact difference using denominator cancellation before multiplication.
    fn subtract(self, rhs: Self) -> TestResult<Self> {
        let common = gcd(self.denominator, rhs.denominator);
        let left_scale = rhs.denominator / common;
        let right_scale = self.denominator / common;
        let numerator = self
            .numerator
            .checked_mul(left_scale)
            .and_then(|left| {
                rhs.numerator
                    .checked_mul(right_scale)
                    .and_then(|right| left.checked_sub(right))
            })
            .ok_or("oracle overflow")?;
        Self::new(
            numerator,
            self.denominator
                .checked_mul(left_scale)
                .ok_or("oracle overflow")?,
        )
    }
    /// Compare nonnegative fractions after canceling common factors on both sides.
    fn magnitude_at_most(self, rhs: Self) -> TestResult<bool> {
        if rhs.numerator < 0 {
            return Ok(false);
        }
        let denominators = gcd(self.denominator, rhs.denominator);
        let numerators = gcd(self.numerator.abs(), rhs.numerator);
        let left = (self.numerator.abs() / numerators)
            .checked_mul(rhs.denominator / denominators)
            .ok_or("oracle overflow")?;
        let right = (rhs.numerator / numerators)
            .checked_mul(self.denominator / denominators)
            .ok_or("oracle overflow")?;
        Ok(left <= right)
    }
}

/// Euclid's integer algorithm; zero/zero is harmless in a zero comparison.
fn gcd(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

/// Check the returned radius against exact rational displacement, independently of production bounds.
pub(crate) fn covers(
    center: f64,
    radius: f64,
    numerator: i128,
    denominator: i128,
) -> TestResult<bool> {
    Fraction::binary(center)?
        .subtract(Fraction::new(numerator, denominator)?)?
        .magnitude_at_most(Fraction::binary(radius)?)
}
