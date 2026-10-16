// SPDX-License-Identifier: AGPL-3.0-only
//! Independent checked integer fractions; no production enclosure arithmetic supplies truth.

use crate::conversion_testing::TestResult;

/// Reduced exact rational for small finite fixtures, with explicit range refusal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rational {
    /// Signed numerator.
    numerator: i128,
    /// Strictly positive denominator.
    denominator: i128,
}

impl Rational {
    /// Reduce before subsequent arithmetic to keep the bounded oracle independent and exact.
    pub(crate) fn new(numerator: i128, denominator: i128) -> TestResult<Self> {
        if denominator <= 0 || numerator == i128::MIN {
            return Err("oracle denominator".into());
        }
        let divisor = gcd(numerator.abs(), denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    /// Decode a binary64 sign, significand and exponent without calling scalar conversion.
    pub(crate) fn binary(value: f64) -> TestResult<Self> {
        let bits = value.to_bits();
        let exponent = i32::try_from((bits >> 52) & 2047)?;
        if exponent == 2047 {
            return Err("nonfinite oracle input".into());
        }
        let significand =
            (bits & ((1_u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1_u64 << 52 };
        if significand == 0 {
            return Self::new(0, 1);
        }
        let zeros = significand.trailing_zeros();
        let magnitude = i128::from(significand >> zeros);
        let signed = if bits >> 63 == 0 {
            magnitude
        } else {
            -magnitude
        };
        let power = if exponent == 0 {
            -1074
        } else {
            exponent - 1075
        } + i32::try_from(zeros)?;
        let factor = 1_i128
            .checked_shl(power.unsigned_abs())
            .filter(|value| *value > 0)
            .ok_or("oracle range")?;
        if power >= 0 {
            Self::new(signed.checked_mul(factor).ok_or("oracle range")?, 1)
        } else {
            Self::new(signed, factor)
        }
    }

    /// Exact addition with denominator cancellation.
    pub(crate) fn add(self, rhs: Self) -> TestResult<Self> {
        let common = gcd(self.denominator, rhs.denominator);
        let first = rhs.denominator / common;
        let second = self.denominator / common;
        Self::new(
            self.numerator
                .checked_mul(first)
                .and_then(|left| {
                    rhs.numerator
                        .checked_mul(second)
                        .and_then(|right| left.checked_add(right))
                })
                .ok_or("oracle overflow")?,
            self.denominator
                .checked_mul(first)
                .ok_or("oracle overflow")?,
        )
    }

    /// Exact subtraction, used to compare a candidate with independent truth.
    pub(crate) fn subtract(self, rhs: Self) -> TestResult<Self> {
        self.add(Self {
            numerator: -rhs.numerator,
            ..rhs
        })
    }

    /// Exact product after cross-canceling numerator and denominator factors.
    pub(crate) fn multiply(self, rhs: Self) -> TestResult<Self> {
        let first = gcd(self.numerator.abs(), rhs.denominator);
        let second = gcd(rhs.numerator.abs(), self.denominator);
        Self::new(
            (self.numerator / first)
                .checked_mul(rhs.numerator / second)
                .ok_or("oracle overflow")?,
            (self.denominator / second)
                .checked_mul(rhs.denominator / first)
                .ok_or("oracle overflow")?,
        )
    }

    /// Exact quotient of independently decoded inputs.
    pub(crate) fn divide(self, rhs: Self) -> TestResult<Self> {
        if rhs.numerator == 0 {
            return Err("oracle zero divisor".into());
        }
        let sign = rhs.numerator.signum();
        self.multiply(Self::new(sign * rhs.denominator, rhs.numerator.abs())?)
    }

    /// Signed ordering through an exact reduced difference.
    pub(crate) fn at_most(self, rhs: Self) -> TestResult<bool> {
        Ok(self.subtract(rhs)?.numerator <= 0)
    }

    /// Compare absolute candidate displacement with a returned binary radius.
    pub(crate) fn covered(self, center: f64, radius: f64) -> TestResult<bool> {
        let difference = self.subtract(Self::binary(center)?)?;
        Self {
            numerator: difference.numerator.abs(),
            ..difference
        }
        .at_most(Self::binary(radius)?)
    }
}

/// Euclid's integer algorithm, including the all-zero normalization case.
fn gcd(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}
