// SPDX-License-Identifier: AGPL-3.0-only
//! Locally owned rational invariants over budgeted arbitrary-precision magnitudes.

use std::cmp::Ordering;
use std::fmt;

use num_bigint::BigUint;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};

/// A reduced exact rational with independently measured component sizes.
#[derive(Clone, Eq, PartialEq)]
pub struct Rational {
    /// True only for a nonzero negative numerator.
    negative: bool,
    /// Coprime nonnegative numerator magnitude.
    numerator: BigUint,
    /// Coprime strictly positive denominator.
    denominator: BigUint,
}

impl fmt::Debug for Rational {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Rational")
            .field("negative", &self.negative)
            .field("numerator_bits", &self.numerator.bits())
            .field("denominator_bits", &self.denominator.bits())
            .finish_non_exhaustive()
    }
}

/// Count conservative machine limbs without overflowing a size calculation.
fn limbs(bits: u64, at: Location) -> Result<u64, Fault> {
    bits.checked_add(63)
        .map(|value| (value / 64).max(1))
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))
}

/// Prepay a conservative quadratic operation bound, including linear overhead.
fn charge(a: u64, b: u64, budget: &mut Budget, at: Location) -> Result<(), Fault> {
    let left = limbs(a, at)?;
    let right = limbs(b, at)?;
    let amount = left
        .checked_add(1)
        .and_then(|n| right.checked_add(1).and_then(|m| n.checked_mul(m)))
        .and_then(|n| n.checked_mul(8))
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
    budget.arithmetic(amount, at)
}

/// Multiply only after an intermediate-size and operation-work preflight.
fn product(a: &BigUint, b: &BigUint, budget: &mut Budget, at: Location) -> Result<BigUint, Fault> {
    let bound = if a.bits() == 0 || b.bits() == 0 {
        0
    } else {
        a.bits()
            .checked_add(b.bits())
            .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))?
    };
    budget.check(BudgetKind::IntermediateBits, bound, at)?;
    charge(a.bits(), b.bits(), budget, at)?;
    let result = a * b;
    budget.check(BudgetKind::IntermediateBits, result.bits(), at)?;
    Ok(result)
}

/// Divide with an explicit nonzero check and a conservative work preflight.
fn quotient(a: &BigUint, b: &BigUint, budget: &mut Budget, at: Location) -> Result<BigUint, Fault> {
    if b.bits() == 0 {
        return Err(Fault::invalid(InvalidCode::ZeroDenominator, at));
    }
    budget.check(BudgetKind::IntermediateBits, a.bits(), at)?;
    charge(a.bits(), b.bits(), budget, at)?;
    Ok(a / b)
}

/// Euclidean gcd with a charge before every remainder and owned copy.
fn gcd(a: &BigUint, b: &BigUint, budget: &mut Budget, at: Location) -> Result<BigUint, Fault> {
    charge(a.bits(), b.bits(), budget, at)?;
    let mut left = a.clone();
    let mut right = b.clone();
    while right.bits() != 0 {
        charge(left.bits(), right.bits(), budget, at)?;
        let remainder = &left % &right;
        left = right;
        right = remainder;
    }
    Ok(left)
}

impl Rational {
    /// Construct and reduce signed magnitudes under the active bit budgets.
    ///
    /// # Errors
    /// Refuses zero denominators, oversized intermediates/components or spent work.
    pub fn new(
        negative: bool,
        numerator: &BigUint,
        denominator: &BigUint,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let at = Location::new("rational constructor");
        budget.check(BudgetKind::IntermediateBits, numerator.bits(), at)?;
        budget.check(BudgetKind::IntermediateBits, denominator.bits(), at)?;
        if denominator.bits() == 0 {
            return Err(Fault::invalid(InvalidCode::ZeroDenominator, at));
        }
        let divisor = gcd(numerator, denominator, budget, at)?;
        let numerator = quotient(numerator, &divisor, budget, at)?;
        let denominator = quotient(denominator, &divisor, budget, at)?;
        budget.check(BudgetKind::ComponentBits, numerator.bits(), at)?;
        budget.check(BudgetKind::ComponentBits, denominator.bits(), at)?;
        Ok(Self {
            negative: negative && numerator.bits() != 0,
            numerator,
            denominator,
        })
    }

    /// The exact additive identity.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            negative: false,
            numerator: BigUint::from(0_u8),
            denominator: BigUint::from(1_u8),
        }
    }

    /// The exact multiplicative identity.
    #[must_use]
    pub fn one() -> Self {
        Self {
            negative: false,
            numerator: BigUint::from(1_u8),
            denominator: BigUint::from(1_u8),
        }
    }

    /// Construct a small exact fraction, including the signed minimum safely.
    ///
    /// # Errors
    /// Refuses zero denominators or exhausted size/work budgets.
    pub fn from_ratio(
        numerator: i64,
        denominator: u64,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Self::new(
            numerator.is_negative(),
            &BigUint::from(numerator.unsigned_abs()),
            &BigUint::from(denominator),
            budget,
        )
    }

    /// Compute the exact integral floor with prepaid magnitude arithmetic.
    pub(crate) fn floor(&self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("integer region rounding");
        self.check(budget, at)?;
        let mut whole = quotient(&self.numerator, &self.denominator, budget, at)?;
        charge(self.numerator.bits(), self.denominator.bits(), budget, at)?;
        let remainder = &self.numerator % &self.denominator;
        if self.negative && remainder.bits() != 0 {
            let bound = whole
                .bits()
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))?;
            budget.check(BudgetKind::IntermediateBits, bound, at)?;
            charge(whole.bits(), 1, budget, at)?;
            whole += BigUint::from(1_u8);
        }
        Self::new(self.negative, &whole, &BigUint::from(1_u8), budget)
    }

    /// Whether the canonical numerator is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.numerator.bits() == 0
    }

    /// Whether the reduced denominator is exactly one.
    #[must_use]
    pub fn is_integer(&self) -> bool {
        self.denominator.bits() == 1
    }

    /// Whether the canonical rational is negative.
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        self.negative
    }

    /// Measured numerator and denominator bit lengths.
    #[must_use]
    pub fn component_bits(&self) -> (u64, u64) {
        (self.numerator.bits(), self.denominator.bits())
    }

    /// Validate an existing value against a possibly smaller call budget.
    ///
    /// # Errors
    /// Returns `ArithmeticRange` before copying a component that exceeds the limit.
    pub fn check(&self, budget: &Budget, at: Location) -> Result<(), Fault> {
        budget.check(BudgetKind::ComponentBits, self.numerator.bits(), at)?;
        budget.check(BudgetKind::ComponentBits, self.denominator.bits(), at)
    }

    /// Copy and negate without a machine signed-minimum special case.
    ///
    /// # Errors
    /// Refuses component or work budget exhaustion.
    pub fn negated(&self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational negation");
        self.check(budget, at)?;
        charge(self.numerator.bits(), self.denominator.bits(), budget, at)?;
        let mut value = self.clone();
        value.negative = !value.negative && !value.is_zero();
        Ok(value)
    }

    /// Add through denominator gcd reduction and signed-magnitude accumulation.
    ///
    /// # Errors
    /// Refuses any oversized intermediate or exhausted work budget.
    pub fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational addition");
        self.check(budget, at)?;
        other.check(budget, at)?;
        let common = gcd(&self.denominator, &other.denominator, budget, at)?;
        let left_scale = quotient(&other.denominator, &common, budget, at)?;
        let right_scale = quotient(&self.denominator, &common, budget, at)?;
        let left = product(&self.numerator, &left_scale, budget, at)?;
        let right = product(&other.numerator, &right_scale, budget, at)?;
        let bound = left
            .bits()
            .max(right.bits())
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))?;
        budget.check(BudgetKind::IntermediateBits, bound, at)?;
        charge(left.bits(), right.bits(), budget, at)?;
        let (negative, numerator) = if self.negative == other.negative {
            (self.negative, left + right)
        } else {
            let order = left.cmp(&right);
            charge(left.bits(), right.bits(), budget, at)?;
            if order == Ordering::Less {
                (other.negative, right - left)
            } else {
                (self.negative, left - right)
            }
        };
        let denominator = product(&self.denominator, &left_scale, budget, at)?;
        Self::new(negative, &numerator, &denominator, budget).map_err(|mut error| {
            error.location = at;
            error
        })
    }

    /// Subtract with the same exactness and resource contract as addition.
    ///
    /// # Errors
    /// Refuses component, intermediate or work exhaustion.
    pub fn sub(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let negative = other.negated(budget)?;
        self.add(&negative, budget)
    }

    /// Multiply after cross-canceling both numerator/denominator pairs.
    ///
    /// # Errors
    /// Refuses component, intermediate or work exhaustion.
    pub fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational multiplication");
        self.check(budget, at)?;
        other.check(budget, at)?;
        let first = gcd(&self.numerator, &other.denominator, budget, at)?;
        let second = gcd(&other.numerator, &self.denominator, budget, at)?;
        let left_n = quotient(&self.numerator, &first, budget, at)?;
        let right_n = quotient(&other.numerator, &second, budget, at)?;
        let left_d = quotient(&self.denominator, &second, budget, at)?;
        let right_d = quotient(&other.denominator, &first, budget, at)?;
        let numerator = product(&left_n, &right_n, budget, at)?;
        let denominator = product(&left_d, &right_d, budget, at)?;
        Self::new(
            self.negative != other.negative,
            &numerator,
            &denominator,
            budget,
        )
        .map_err(|mut error| {
            error.location = at;
            error
        })
    }

    /// Divide by a nonzero rational, preserving exact values.
    ///
    /// # Errors
    /// Refuses a zero divisor or exhausted size/work budgets.
    pub fn div(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational division");
        other.check(budget, at)?;
        if other.is_zero() {
            return Err(Fault::invalid(InvalidCode::ZeroDenominator, at));
        }
        charge(other.numerator.bits(), other.denominator.bits(), budget, at)?;
        let reciprocal = Self {
            negative: other.negative,
            numerator: other.denominator.clone(),
            denominator: other.numerator.clone(),
        };
        self.mul(&reciprocal, budget).map_err(|mut error| {
            error.location = at;
            error
        })
    }

    /// Execute an integer power by budgeted repeated squaring.
    ///
    /// # Errors
    /// Refuses strict zero-to-zero, zero negative powers or resource exhaustion.
    pub fn pow(&self, exponent: i64, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational power");
        self.check(budget, at)?;
        let mut remaining = exponent.unsigned_abs();
        budget.check(BudgetKind::IntegerExponent, remaining, at)?;
        if self.is_zero() && exponent <= 0 {
            return Err(Fault::invalid(InvalidCode::UndefinedPower, at));
        }
        charge(self.numerator.bits(), self.denominator.bits(), budget, at)?;
        let mut factor = self.clone();
        if exponent < 0 {
            std::mem::swap(&mut factor.numerator, &mut factor.denominator);
        }
        let mut result = Self::one();
        while remaining != 0 {
            if remaining & 1 == 1 {
                result = result.mul(&factor, budget)?;
            }
            remaining /= 2;
            if remaining != 0 {
                factor = factor.mul(&factor, budget)?;
            }
        }
        Ok(result)
    }

    /// Compare exact values using continued fractions instead of cross-products.
    ///
    /// # Errors
    /// Refuses component or work exhaustion.
    pub fn compare(&self, other: &Self, budget: &mut Budget) -> Result<Ordering, Fault> {
        let at = Location::new("rational comparison");
        self.check(budget, at)?;
        other.check(budget, at)?;
        charge(self.numerator.bits(), self.denominator.bits(), budget, at)?;
        charge(other.numerator.bits(), other.denominator.bits(), budget, at)?;
        if self.negative != other.negative {
            return Ok(if self.negative {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let (mut an, mut ad) = (self.numerator.clone(), self.denominator.clone());
        let (mut bn, mut bd) = (other.numerator.clone(), other.denominator.clone());
        let mut reverse = self.negative;
        loop {
            let aq = quotient(&an, &ad, budget, at)?;
            let bq = quotient(&bn, &bd, budget, at)?;
            charge(aq.bits(), bq.bits(), budget, at)?;
            let order = aq.cmp(&bq);
            if order != Ordering::Equal {
                return Ok(if reverse { order.reverse() } else { order });
            }
            charge(an.bits(), ad.bits(), budget, at)?;
            charge(bn.bits(), bd.bits(), budget, at)?;
            let ar = &an % &ad;
            let br = &bn % &bd;
            if ar.bits() == 0 || br.bits() == 0 {
                let order = ar.bits().cmp(&br.bits());
                return Ok(if reverse { order.reverse() } else { order });
            }
            (an, ad) = (ad, ar);
            (bn, bd) = (bd, br);
            reverse = !reverse;
        }
    }

    /// Format an exact signed numerator and optional denominator under a byte limit.
    ///
    /// # Errors
    /// Refuses size or formatting work exhaustion before conversion.
    pub fn exact_string(&self, budget: &mut Budget) -> Result<String, Fault> {
        let at = Location::new("rational formatting");
        self.check(budget, at)?;
        let bound = self
            .numerator
            .bits()
            .checked_add(self.denominator.bits())
            .and_then(|n| n.checked_add(3))
            .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
        budget.check(BudgetKind::OutputBytes, bound, at)?;
        charge(self.numerator.bits(), self.numerator.bits(), budget, at)?;
        charge(self.denominator.bits(), self.denominator.bits(), budget, at)?;
        let prefix = if self.negative { "-" } else { "" };
        if self.is_integer() {
            Ok(format!("{prefix}{}", self.numerator.to_str_radix(10)))
        } else {
            Ok(format!(
                "{prefix}{}/{}",
                self.numerator.to_str_radix(10),
                self.denominator.to_str_radix(10)
            ))
        }
    }

    /// Produce canonical length-prefixed magnitude bytes without a search context.
    ///
    /// # Errors
    /// Refuses bit, output, allocation or operation-work exhaustion.
    pub fn canonical_bytes(&self, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        self.encode_magnitudes(budget, BudgetKind::OutputBytes)
    }

    /// Encode an internal structural key under the payload ceiling.
    pub(crate) fn structural_bytes(&self, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        self.encode_magnitudes(budget, BudgetKind::PayloadBytes)
    }

    /// Share the exact magnitude layout while keeping admission and export limits distinct.
    fn encode_magnitudes(&self, budget: &mut Budget, limit: BudgetKind) -> Result<Vec<u8>, Fault> {
        let at = Location::new("rational encoding");
        self.check(budget, at)?;
        charge(self.numerator.bits(), self.denominator.bits(), budget, at)?;
        for bits in [self.numerator.bits(), self.denominator.bits()] {
            u32::try_from(bits.div_ceil(8)).map_err(|_| Fault::capacity(limit, at))?;
        }
        let size = self
            .numerator
            .bits()
            .div_ceil(8)
            .checked_add(self.denominator.bits().div_ceil(8))
            .and_then(|n| n.checked_add(9))
            .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
        budget.check(limit, size, at)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(
                usize::try_from(size).map_err(|_| Fault::capacity(BudgetKind::OutputBytes, at))?,
            )
            .map_err(|_| Fault::allocation(at))?;
        bytes.push(u8::from(self.negative));
        for magnitude in [&self.numerator, &self.denominator] {
            let digits = if magnitude.bits() == 0 {
                Vec::new()
            } else {
                magnitude.to_bytes_le()
            };
            let length = u32::try_from(digits.len())
                .map_err(|_| Fault::capacity(BudgetKind::OutputBytes, at))?;
            bytes.extend_from_slice(&length.to_le_bytes());
            bytes.extend_from_slice(&digits);
        }
        Ok(bytes)
    }

    /// Use a fixed integer-power payload only when the exact exponent fits it.
    pub(crate) fn integer_exponent(&self, budget: &mut Budget) -> Result<Option<i64>, Fault> {
        if !self.is_integer() || self.numerator.bits() > 64 {
            return Ok(None);
        }
        let at = Location::new("parsed integer exponent");
        charge(self.numerator.bits(), 1, budget, at)?;
        let mut magnitude = 0_u64;
        for (position, byte) in self.numerator.to_bytes_le().iter().enumerate() {
            let shift = u32::try_from(position)
                .ok()
                .and_then(|n| n.checked_mul(8))
                .ok_or_else(|| Fault::capacity(BudgetKind::ComponentBits, at))?;
            magnitude |= u64::from(*byte)
                .checked_shl(shift)
                .ok_or_else(|| Fault::capacity(BudgetKind::ComponentBits, at))?;
        }
        if self.negative && magnitude == (1_u64 << 63) {
            return Ok(Some(i64::MIN));
        }
        Ok(i64::try_from(magnitude)
            .ok()
            .map(|value| if self.negative { -value } else { value }))
    }

    /// Parse an exact integer, fraction, decimal or decimal scientific spelling.
    ///
    /// # Errors
    /// Refuses malformed spellings, zero denominators and resource exhaustion.
    pub fn parse(text: &str, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational parsing");
        let length =
            u64::try_from(text.len()).map_err(|_| Fault::capacity(BudgetKind::LiteralBytes, at))?;
        budget.check(BudgetKind::LiteralBytes, length, at)?;
        budget.work(length, at)?;
        let (negative, body) = strip_sign(text);
        if let Some((numerator, denominator)) = body.split_once('/') {
            return Self::new(
                negative,
                &digits(numerator, budget)?,
                &digits(denominator, budget)?,
                budget,
            );
        }
        let mut parts = body.split(['e', 'E']);
        let mantissa = parts
            .next()
            .ok_or_else(|| Fault::invalid(InvalidCode::Literal, at))?;
        let exponent = parts
            .next()
            .map(|value| decimal_exponent(value, budget))
            .transpose()?;
        if parts.next().is_some() {
            return Err(Fault::invalid(InvalidCode::Literal, at));
        }
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if whole.is_empty() && fraction.is_empty() {
            return Err(Fault::invalid(InvalidCode::Literal, at));
        }
        if !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
        {
            return Err(Fault::invalid(InvalidCode::Literal, at));
        }
        let combined = format!("{whole}{fraction}");
        let numerator = digits(&combined, budget)?;
        let fraction_len = i64::try_from(fraction.len())
            .map_err(|_| Fault::capacity(BudgetKind::DecimalExponent, at))?;
        let scale = exponent
            .unwrap_or(0)
            .checked_sub(fraction_len)
            .ok_or_else(|| Fault::capacity(BudgetKind::DecimalExponent, at))?;
        budget.check(BudgetKind::DecimalExponent, scale.unsigned_abs(), at)?;
        let mut ten_power = BigUint::from(1_u8);
        for _ in 0..scale.unsigned_abs() {
            ten_power = product(&ten_power, &BigUint::from(10_u8), budget, at)?;
        }
        if scale >= 0 {
            Self::new(
                negative,
                &product(&numerator, &ten_power, budget, at)?,
                &BigUint::from(1_u8),
                budget,
            )
        } else {
            Self::new(negative, &numerator, &ten_power, budget)
        }
    }
}

/// Remove exactly one leading sign without changing any other spelling.
fn strip_sign(text: &str) -> (bool, &str) {
    text.strip_prefix('-').map_or_else(
        || (false, text.strip_prefix('+').unwrap_or(text)),
        |body| (true, body),
    )
}

/// Accumulate decimal digits with a size/work preflight before each bigint step.
fn digits(text: &str, budget: &mut Budget) -> Result<BigUint, Fault> {
    let at = Location::new("decimal digits");
    if text.is_empty() {
        return Err(Fault::invalid(InvalidCode::Literal, at));
    }
    let mut value = BigUint::from(0_u8);
    for digit in text.bytes() {
        if !digit.is_ascii_digit() {
            return Err(Fault::invalid(InvalidCode::Literal, at));
        }
        value = product(&value, &BigUint::from(10_u8), budget, at)?;
        let bound = value
            .bits()
            .max(4)
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))?;
        budget.check(BudgetKind::IntermediateBits, bound, at)?;
        charge(value.bits(), 4, budget, at)?;
        value += BigUint::from(digit - b'0');
    }
    Ok(value)
}

/// Read a bounded decimal exponent without constructing a large power first.
fn decimal_exponent(text: &str, budget: &mut Budget) -> Result<i64, Fault> {
    let at = Location::new("decimal exponent");
    let (negative, body) = strip_sign(text);
    if body.is_empty() {
        return Err(Fault::invalid(InvalidCode::Literal, at));
    }
    let mut value = 0_u64;
    for byte in body.bytes() {
        if !byte.is_ascii_digit() {
            return Err(Fault::invalid(InvalidCode::Literal, at));
        }
        budget.work(1, at)?;
        value = value
            .checked_mul(10)
            .and_then(|n| n.checked_add(u64::from(byte - b'0')))
            .ok_or_else(|| Fault::capacity(BudgetKind::DecimalExponent, at))?;
        budget.check(BudgetKind::DecimalExponent, value, at)?;
    }
    let signed =
        i64::try_from(value).map_err(|_| Fault::capacity(BudgetKind::DecimalExponent, at))?;
    Ok(if negative { -signed } else { signed })
}

#[cfg(test)]
mod tests {
    use super::Rational;
    use crate::{
        Fault,
        limits::{Budget, Limits},
    };

    /// Fixed power payloads respect both signed endpoints without narrowing exact values.
    /// ´test:unit:parsed-integer-exponent-range´
    #[test]
    fn parsed_integer_exponent_range() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::default())?;
        for (text, expected) in [
            ("0", Some(0)),
            ("-0", Some(0)),
            ("4/2", Some(2)),
            ("9223372036854775807", Some(i64::MAX)),
            ("-9223372036854775808", Some(i64::MIN)),
            ("9223372036854775808", None),
            ("-9223372036854775809", None),
            ("18446744073709551616", None),
            ("1/2", None),
        ] {
            assert_eq!(
                Rational::parse(text, &mut budget)?.integer_exponent(&mut budget)?,
                expected
            );
        }
        Ok(())
    }
}
