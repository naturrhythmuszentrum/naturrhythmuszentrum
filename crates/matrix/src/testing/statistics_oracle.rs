// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact integer comparisons cover every finite binary64 parameter and radius.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::TestResult;
use std::cmp::Ordering;

/// Signed little-endian integer words; no production arithmetic or floating operations.
#[derive(Clone, Debug)]
struct Integer {
    /// Zero is always nonnegative.
    negative: bool,
    /// Canonical base-2^32 magnitude, with no high zero words.
    words: Vec<u32>,
}
impl Integer {
    /// Canonicalize after an exact word operation.
    fn normalize(negative: bool, mut words: Vec<u32>) -> Self {
        let length = words
            .iter()
            .rposition(|word| *word != 0)
            .map_or(0, |index| index + 1);
        words.truncate(length);
        Self {
            negative: negative && !words.is_empty(),
            words,
        }
    }
    /// Split a bounded mathematical fixture integer without a narrowing cast.
    fn unsigned(mut value: u128) -> TestResult<Self> {
        let mut words = Vec::new();
        while value != 0 {
            words.push(u32::try_from(value & u128::from(u32::MAX))?);
            value >>= 32;
        }
        Ok(Self::normalize(false, words))
    }
    /// Restore an exact rational numerator's sign.
    fn signed(value: i128) -> TestResult<Self> {
        let mut result = Self::unsigned(value.unsigned_abs())?;
        result.negative = value < 0;
        Ok(result)
    }
    /// Multiply by an exact power of two, retaining every bit.
    fn shifted(&self, bits: usize) -> TestResult<Self> {
        let offset = bits / 32;
        let remainder = bits % 32;
        let length = self
            .words
            .len()
            .checked_add(offset)
            .and_then(|n| n.checked_add(1))
            .ok_or("oracle size")?;
        let mut words = vec![0; length];
        let mut carry = 0_u64;
        for (index, &word) in self.words.iter().enumerate() {
            let value = (u64::from(word) << remainder) | carry;
            *words.get_mut(index + offset).ok_or("oracle shift")? =
                u32::try_from(value & u64::from(u32::MAX))?;
            carry = value >> 32;
        }
        *words.last_mut().ok_or("oracle shift")? = u32::try_from(carry)?;
        Ok(Self::normalize(self.negative, words))
    }
    /// Decode value times 2^1074 exactly from IEEE sign, significand and exponent.
    fn binary(value: f64) -> TestResult<Self> {
        let bits = value.to_bits();
        let exponent = usize::try_from((bits >> 52) & 2047)?;
        if exponent == 2047 {
            return Err("nonfinite statistical oracle input".into());
        }
        let significand =
            (bits & ((1_u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1_u64 << 52 };
        let mut result =
            Self::unsigned(u128::from(significand))?.shifted(exponent.saturating_sub(1))?;
        result.negative = bits >> 63 != 0 && !result.words.is_empty();
        Ok(result)
    }
    /// Compare exact magnitudes before applying signs.
    fn magnitude_order(&self, other: &Self) -> Ordering {
        self.words
            .len()
            .cmp(&other.words.len())
            .then_with(|| self.words.iter().rev().cmp(other.words.iter().rev()))
    }
    /// Exact signed order never forms a denominator product.
    fn order(&self, other: &Self) -> Ordering {
        match (self.negative, other.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => self.magnitude_order(other).reverse(),
            (false, false) => self.magnitude_order(other),
        }
    }
    /// Negate an exact integer without introducing a negative zero.
    fn negated(&self) -> Self {
        Self {
            negative: !self.negative && !self.words.is_empty(),
            words: self.words.clone(),
        }
    }
    /// Signed exact addition uses only word addition or ordered magnitude subtraction.
    fn add(&self, other: &Self) -> TestResult<Self> {
        if self.negative == other.negative {
            Ok(Self::normalize(
                self.negative,
                add_words(&self.words, &other.words)?,
            ))
        } else if self.magnitude_order(other) == Ordering::Less {
            Ok(Self::normalize(
                other.negative,
                subtract_words(&other.words, &self.words)?,
            ))
        } else {
            Ok(Self::normalize(
                self.negative,
                subtract_words(&self.words, &other.words)?,
            ))
        }
    }
    /// Schoolbook word products retain the complete exact result.
    fn multiply(&self, other: &Self) -> TestResult<Self> {
        let length = self
            .words
            .len()
            .checked_add(other.words.len())
            .ok_or("oracle size")?;
        let mut words = vec![0_u32; length];
        for (first, &left) in self.words.iter().enumerate() {
            let mut carry = 0_u64;
            for (second, &right) in other.words.iter().enumerate() {
                let cell = words.get_mut(first + second).ok_or("oracle product")?;
                let value = u64::from(left) * u64::from(right) + u64::from(*cell) + carry;
                *cell = u32::try_from(value & u64::from(u32::MAX))?;
                carry = value >> 32;
            }
            if !other.words.is_empty() {
                *words
                    .get_mut(first + other.words.len())
                    .ok_or("oracle carry")? = u32::try_from(carry)?;
            }
        }
        Ok(Self::normalize(self.negative != other.negative, words))
    }
}

/// Add unsigned base-2^32 words with an exact one-word carry.
fn add_words(left: &[u32], right: &[u32]) -> TestResult<Vec<u32>> {
    let length = left.len().max(right.len());
    let mut result = Vec::new();
    let mut carry = 0_u64;
    for index in 0..length {
        let value = u64::from(left.get(index).copied().unwrap_or(0))
            + u64::from(right.get(index).copied().unwrap_or(0))
            + carry;
        result.push(u32::try_from(value & u64::from(u32::MAX))?);
        carry = value >> 32;
    }
    if carry != 0 {
        result.push(u32::try_from(carry)?);
    }
    Ok(result)
}

/// Subtract only ordered magnitudes, with an explicit borrow and no signed overflow.
fn subtract_words(left: &[u32], right: &[u32]) -> TestResult<Vec<u32>> {
    let mut result = Vec::new();
    let mut borrow = 0_u64;
    for (index, &word) in left.iter().enumerate() {
        let subtrahend = u64::from(right.get(index).copied().unwrap_or(0)) + borrow;
        let value = (1_u64 << 32) + u64::from(word) - subtrahend;
        result.push(u32::try_from(value & u64::from(u32::MAX))?);
        borrow = u64::from(u64::from(word) < subtrahend);
    }
    if borrow != 0 {
        return Err("oracle magnitude order".into());
    }
    Ok(result)
}

/// Compare a scaled integer to a small exact fraction by exact cross multiplication.
fn fraction_order(value: &Integer, truth: Rational, scale: usize) -> TestResult<Ordering> {
    let denominator = Integer::unsigned(u128::try_from(truth.denominator)?)?;
    let target = Integer::signed(truth.numerator)?.shifted(scale)?;
    Ok(value.multiply(&denominator)?.order(&target))
}

/// Compare the whole exact binary center-radius interval with independent rational truth.
pub(crate) fn covered(truth: Rational, center: f64, radius: f64) -> TestResult<bool> {
    if !radius.is_finite() || radius.is_sign_negative() {
        return Err("invalid statistical oracle radius".into());
    }
    let nominal = Integer::binary(center)?;
    let error = Integer::binary(radius)?;
    let lower = nominal.add(&error.negated())?;
    let upper = nominal.add(&error)?;
    Ok(fraction_order(&lower, truth, 1074)? != Ordering::Greater
        && fraction_order(&upper, truth, 1074)? != Ordering::Less)
}

#[cfg(test)]
mod tests {
    use super::covered;
    use crate::kernel_oracle::Rational;
    use crate::kernel_testing::TestResult;

    /// Subnormal and maximal finite intervals retain their exact containment boundary.
    /// ´test:unit:statistical-oracle-covers-the-full-binary-range´
    #[test]
    fn statistical_oracle_covers_the_full_binary_range() -> TestResult {
        let zero = Rational::new(0, 1)?;
        let smallest = f64::from_bits(1);
        assert!(covered(zero, smallest, smallest)?);
        assert!(covered(zero, -smallest, smallest)?);
        assert!(!covered(zero, smallest, 0.0)?);
        assert!(covered(zero, f64::MAX, f64::MAX)?);
        assert!(covered(zero, -f64::MAX, f64::MAX)?);
        assert!(covered(Rational::new(1, 3)?, 0.0, 0.5)?);
        assert!(!covered(Rational::new(1, 3)?, 0.0, 0.25)?);
        assert!(covered(zero, f64::INFINITY, 1.0).is_err());
        assert!(covered(zero, 0.0, -smallest).is_err());
        assert!(covered(zero, 0.0, -0.0).is_err());
        Ok(())
    }
}
