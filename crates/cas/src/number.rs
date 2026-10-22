// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical `Ratio` backing with prepaid arbitrary-precision magnitude arithmetic.
//!
//! # Exact rational representation
//!
//! `Rational` owns canonical sign/magnitude, positive-denominator and reduction invariants over `num-bigint` 0.4 with default features disabled. Integers, fractions, decimals and decimal scientific notation parse exactly. Arithmetic cross-cancels before multiplication and uses continued fractions for comparison. The default component/intermediate limits are 4,096/8,192 bits; `(4/5)^78` fits with numerator/denominator sizes of 157/182 bits. Every big-integer operation prepays a conservative size-dependent work charge; a smaller bit budget returns `Exhausted` with `ArithmeticRange`. `petgraph` 0.8 supplies append-only graph storage. The seed lock inspected on 2026-10-22 at `51105aeb4927f5e2d0bb76a607d87aae1d40c00b` pins `num-bigint` 0.4.8 and `petgraph` 0.8.3, including their exact transitive dependencies. The production arithmetic uses those dependencies without a numerical engine dependency.

use std::cmp::Ordering;
use std::fmt;

use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer;
use num_rational::Ratio;
use num_traits::{One, Zero};

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};

/// A reduced exact rational with independently measured component sizes.
#[derive(Clone)]
pub struct Rational {
    /// Reduced signed numerator and strictly positive denominator; zero is 0/1.
    value: Ratio<BigInt>,
    /// Cached numerator sign preserves the public constant sign query.
    negative: bool,
}

impl PartialEq for Rational {
    // Canonical components avoid the dependency's arithmetic comparison path.
    fn eq(&self, other: &Self) -> bool {
        self.value.numer() == other.value.numer() && self.value.denom() == other.value.denom()
    }
}

impl Eq for Rational {}

impl fmt::Debug for Rational {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Rational")
            .field("negative", &self.negative)
            .field("numerator_bits", &self.numerator().bits())
            .field("denominator_bits", &self.denominator().bits())
            .finish_non_exhaustive()
    }
}

/// Count conservative machine limbs without overflowing a size calculation.
fn limbs(bits: u64, at: Location) -> Result<u64, Fault> {
    bits.checked_add(63)
        .map(|value| (value / 64).max(1))
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))
}

/// Prepay `C(B) = 8(L(B) + 1)^2` at the largest operand/intermediate bound.
/// This also covers linear scans and both copied magnitudes' retained bytes.
fn charge(a: u64, b: u64, budget: &mut Budget, at: Location) -> Result<(), Fault> {
    let extent = limbs(a.max(b), at)?
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
    let amount = extent
        .checked_mul(extent)
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
    charge(bound.max(a.bits()), b.bits(), budget, at)?;
    let result = a * b;
    budget.check(BudgetKind::IntermediateBits, result.bits(), at)?;
    Ok(result)
}

/// Bound long division's normalized dividend as well as its returned magnitudes.
/// A divisor above 32 bits may need a left shift to fill a dependency limb;
/// rounding to 64 bits conservatively covers both 32-bit and 64-bit digits.
fn division_bits(a: &BigUint, b: &BigUint, at: Location) -> Result<u64, Fault> {
    let largest = a.bits().max(b.bits());
    if a.bits() < b.bits() || b.bits() <= 32 {
        return Ok(largest);
    }
    let shift = (64 - b.bits() % 64) % 64;
    a.bits()
        .checked_add(shift)
        .map(|bound| bound.max(largest))
        .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))
}

/// Divide only after checking the divisor, both inputs and the full work bound.
fn divide(
    a: &BigUint,
    b: &BigUint,
    budget: &mut Budget,
    at: Location,
) -> Result<(BigUint, BigUint), Fault> {
    if b.is_zero() {
        return Err(Fault::invalid(InvalidCode::ZeroDenominator, at));
    }
    let bound = division_bits(a, b, at)?;
    budget.check(BudgetKind::IntermediateBits, bound, at)?;
    charge(bound, b.bits(), budget, at)?;
    let (whole, rest) = a.div_rem(b);
    budget.check(BudgetKind::IntermediateBits, whole.bits(), at)?;
    budget.check(BudgetKind::IntermediateBits, rest.bits(), at)?;
    Ok((whole, rest))
}

/// Read the quotient from the prepaid integer division adapter.
fn quotient(a: &BigUint, b: &BigUint, budget: &mut Budget, at: Location) -> Result<BigUint, Fault> {
    divide(a, b, budget, at).map(|(whole, _)| whole)
}

/// Read the remainder from the prepaid integer division adapter.
fn remainder(
    a: &BigUint,
    b: &BigUint,
    budget: &mut Budget,
    at: Location,
) -> Result<BigUint, Fault> {
    divide(a, b, budget, at).map(|(_, rest)| rest)
}

/// Euclidean gcd with a charge before every `Integer::div_rem` and owned copy.
/// Every two remainders halve the larger input, giving at most `2B + 1` steps.
fn gcd(a: &BigUint, b: &BigUint, budget: &mut Budget, at: Location) -> Result<BigUint, Fault> {
    budget.check(BudgetKind::IntermediateBits, a.bits().max(b.bits()), at)?;
    charge(a.bits(), b.bits(), budget, at)?;
    let mut left = a.clone();
    let mut right = b.clone();
    while !right.is_zero() {
        let rest = remainder(&left, &right, budget, at)?;
        left = right;
        right = rest;
    }
    budget.check(BudgetKind::IntermediateBits, left.bits(), at)?;
    Ok(left)
}

/// Accumulate signed magnitudes only after prepaying comparison and sum bounds.
fn signed_sum(
    (left_negative, left): (bool, BigUint),
    (right_negative, right): (bool, BigUint),
    budget: &mut Budget,
    at: Location,
) -> Result<(bool, BigUint), Fault> {
    let bound = left
        .bits()
        .max(right.bits())
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))?;
    budget.check(BudgetKind::IntermediateBits, bound, at)?;
    charge(left.bits(), right.bits(), budget, at)?;
    let (negative, magnitude) = if left_negative == right_negative {
        (left_negative, left + right)
    } else {
        let order = left.cmp(&right);
        charge(left.bits(), right.bits(), budget, at)?;
        if order == Ordering::Less {
            (right_negative, right - left)
        } else {
            (left_negative, left - right)
        }
    };
    budget.check(BudgetKind::IntermediateBits, magnitude.bits(), at)?;
    Ok((negative, magnitude))
}

impl Rational {
    /// Move already coprime magnitudes into `Ratio` without hidden normalization.
    /// Callers establish a nonzero denominator and prepay any magnitude allocation.
    fn from_coprime_magnitudes(negative: bool, numerator: BigUint, denominator: BigUint) -> Self {
        let negative = negative && !numerator.is_zero();
        let sign = if negative { Sign::Minus } else { Sign::Plus };
        Self {
            value: Ratio::new_raw(
                BigInt::from_biguint(sign, numerator),
                BigInt::from(denominator),
            ),
            negative,
        }
    }

    /// Borrow the canonical numerator magnitude without copying or normalizing.
    fn numerator(&self) -> &BigUint {
        self.value.numer().magnitude()
    }

    /// Borrow the strictly positive canonical denominator magnitude.
    fn denominator(&self) -> &BigUint {
        self.value.denom().magnitude()
    }

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
        Ok(Self::from_coprime_magnitudes(
            negative,
            numerator,
            denominator,
        ))
    }

    /// The exact additive identity.
    #[must_use]
    pub fn zero() -> Self {
        Self::from_coprime_magnitudes(false, BigUint::zero(), BigUint::one())
    }

    /// The exact multiplicative identity.
    #[must_use]
    pub fn one() -> Self {
        Self::from_coprime_magnitudes(false, BigUint::one(), BigUint::one())
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

    /// Positive rational content: numerator gcd divided by denominator lcm.
    /// The identity relies on the type's reduced canonical numerator and denominator.
    /// This ground rescaling does not certify a regional polynomial unit.
    pub(crate) fn content_gcd(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational content gcd");
        self.check(budget, at)?;
        other.check(budget, at)?;
        let numerator = gcd(self.numerator(), other.numerator(), budget, at)?;
        let common = gcd(self.denominator(), other.denominator(), budget, at)?;
        let scale = quotient(other.denominator(), &common, budget, at)?;
        let denominator = product(self.denominator(), &scale, budget, at)?;
        Self::new(false, &numerator, &denominator, budget)
    }

    /// Compute the exact integral floor with prepaid magnitude arithmetic.
    pub(crate) fn floor(&self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("integer region rounding");
        self.check(budget, at)?;
        let mut whole = quotient(self.numerator(), self.denominator(), budget, at)?;
        let rest = remainder(self.numerator(), self.denominator(), budget, at)?;
        if self.negative && !rest.is_zero() {
            let bound = whole
                .bits()
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))?;
            budget.check(BudgetKind::IntermediateBits, bound, at)?;
            charge(whole.bits(), 1, budget, at)?;
            whole += BigUint::one();
            budget.check(BudgetKind::IntermediateBits, whole.bits(), at)?;
        }
        Self::new(self.negative, &whole, &BigUint::from(1_u8), budget)
    }

    /// Whether the canonical numerator is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.numerator().bits() == 0
    }

    /// Whether the reduced denominator is exactly one.
    #[must_use]
    pub fn is_integer(&self) -> bool {
        self.denominator().bits() == 1
    }

    /// Whether the canonical rational is negative.
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        self.negative
    }

    /// Measured numerator and denominator bit lengths.
    #[must_use]
    pub fn component_bits(&self) -> (u64, u64) {
        (self.numerator().bits(), self.denominator().bits())
    }

    /// Validate an existing value against a possibly smaller call budget.
    ///
    /// # Errors
    /// Returns `ArithmeticRange` before copying a component that exceeds the limit.
    pub fn check(&self, budget: &Budget, at: Location) -> Result<(), Fault> {
        budget.check(BudgetKind::ComponentBits, self.numerator().bits(), at)?;
        budget.check(BudgetKind::ComponentBits, self.denominator().bits(), at)
    }

    /// Copy and negate without a machine signed-minimum special case.
    ///
    /// # Errors
    /// Refuses component or work budget exhaustion.
    pub fn negated(&self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational negation");
        self.check(budget, at)?;
        charge(
            self.numerator().bits(),
            self.denominator().bits(),
            budget,
            at,
        )?;
        Ok(Self::from_coprime_magnitudes(
            !self.negative,
            self.numerator().clone(),
            self.denominator().clone(),
        ))
    }

    /// Add through denominator gcd reduction and signed-magnitude accumulation.
    ///
    /// # Errors
    /// Refuses any oversized intermediate or exhausted work budget.
    pub fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("rational addition");
        self.check(budget, at)?;
        other.check(budget, at)?;
        let common = gcd(self.denominator(), other.denominator(), budget, at)?;
        let left_scale = quotient(other.denominator(), &common, budget, at)?;
        let right_scale = quotient(self.denominator(), &common, budget, at)?;
        let left = product(self.numerator(), &left_scale, budget, at)?;
        let right = product(other.numerator(), &right_scale, budget, at)?;
        let (negative, numerator) =
            signed_sum((self.negative, left), (other.negative, right), budget, at)?;
        let denominator = product(self.denominator(), &left_scale, budget, at)?;
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
        let first = gcd(self.numerator(), other.denominator(), budget, at)?;
        let second = gcd(other.numerator(), self.denominator(), budget, at)?;
        let left_n = quotient(self.numerator(), &first, budget, at)?;
        let right_n = quotient(other.numerator(), &second, budget, at)?;
        let left_d = quotient(self.denominator(), &second, budget, at)?;
        let right_d = quotient(other.denominator(), &first, budget, at)?;
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
        charge(
            other.numerator().bits(),
            other.denominator().bits(),
            budget,
            at,
        )?;
        let reciprocal = Self::from_coprime_magnitudes(
            other.negative,
            other.denominator().clone(),
            other.numerator().clone(),
        );
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
        charge(
            self.numerator().bits(),
            self.denominator().bits(),
            budget,
            at,
        )?;
        let mut factor = if exponent < 0 {
            Self::from_coprime_magnitudes(
                self.negative,
                self.denominator().clone(),
                self.numerator().clone(),
            )
        } else {
            self.clone()
        };
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
        charge(
            self.numerator().bits(),
            self.denominator().bits(),
            budget,
            at,
        )?;
        charge(
            other.numerator().bits(),
            other.denominator().bits(),
            budget,
            at,
        )?;
        if self.negative != other.negative {
            return Ok(if self.negative {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let (mut an, mut ad) = (self.numerator().clone(), self.denominator().clone());
        let (mut bn, mut bd) = (other.numerator().clone(), other.denominator().clone());
        let mut reverse = self.negative;
        loop {
            let aq = quotient(&an, &ad, budget, at)?;
            let bq = quotient(&bn, &bd, budget, at)?;
            charge(aq.bits(), bq.bits(), budget, at)?;
            let order = aq.cmp(&bq);
            if order != Ordering::Equal {
                return Ok(if reverse { order.reverse() } else { order });
            }
            let ar = remainder(&an, &ad, budget, at)?;
            let br = remainder(&bn, &bd, budget, at)?;
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
            .numerator()
            .bits()
            .checked_add(self.denominator().bits())
            .and_then(|n| n.checked_add(3))
            .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
        budget.check(BudgetKind::OutputBytes, bound, at)?;
        charge(self.numerator().bits(), self.numerator().bits(), budget, at)?;
        charge(
            self.denominator().bits(),
            self.denominator().bits(),
            budget,
            at,
        )?;
        let prefix = if self.negative { "-" } else { "" };
        if self.is_integer() {
            Ok(format!("{prefix}{}", self.numerator().to_str_radix(10)))
        } else {
            Ok(format!(
                "{prefix}{}/{}",
                self.numerator().to_str_radix(10),
                self.denominator().to_str_radix(10)
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
        charge(
            self.numerator().bits(),
            self.denominator().bits(),
            budget,
            at,
        )?;
        for bits in [self.numerator().bits(), self.denominator().bits()] {
            u32::try_from(bits.div_ceil(8)).map_err(|_| Fault::capacity(limit, at))?;
        }
        let size = self
            .numerator()
            .bits()
            .div_ceil(8)
            .checked_add(self.denominator().bits().div_ceil(8))
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
        for magnitude in [self.numerator(), self.denominator()] {
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

    /// Read reduced components when the numerator fits `i64` and denominator fits `u64`.
    ///
    /// A negative numerator includes the signed minimum. Values outside this finite
    /// component contract return `None` without narrowing or rounding.
    ///
    /// # Errors
    /// Refuses component or work budget exhaustion before reading magnitudes.
    pub fn small_ratio(&self, budget: &mut Budget) -> Result<Option<(i64, u64)>, Fault> {
        let at = Location::new("small rational components");
        self.check(budget, at)?;
        charge(
            self.numerator().bits(),
            self.denominator().bits(),
            budget,
            at,
        )?;
        if self.numerator().bits() > 64 || self.denominator().bits() > 64 {
            return Ok(None);
        }
        let magnitude = self.numerator().iter_u64_digits().next().unwrap_or(0);
        let denominator = self.denominator().iter_u64_digits().next().unwrap_or(1);
        let numerator = if self.negative && magnitude == (1_u64 << 63) {
            i64::MIN
        } else if let Ok(value) = i64::try_from(magnitude) {
            if self.negative { -value } else { value }
        } else {
            return Ok(None);
        };
        Ok(Some((numerator, denominator)))
    }

    /// Use a fixed integer-power payload only when the exact exponent fits it.
    pub(crate) fn integer_exponent(&self, budget: &mut Budget) -> Result<Option<i64>, Fault> {
        if !self.is_integer() || self.numerator().bits() > 64 {
            return Ok(None);
        }
        let at = Location::new("parsed integer exponent");
        charge(self.numerator().bits(), 1, budget, at)?;
        let mut magnitude = 0_u64;
        for (position, byte) in self.numerator().to_bytes_le().iter().enumerate() {
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
        budget.check(BudgetKind::IntermediateBits, value.bits(), at)?;
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
    use num_bigint::{BigInt, BigUint, Sign};

    use super::{Rational, charge, divide, gcd, product, signed_sum};
    use crate::{
        Fault, InvalidCode, Location,
        limits::{Budget, BudgetKind, Limits},
    };

    /// Reduction, zero and sign are invariants of the private `Ratio` itself.
    /// ´test:unit:ratio-backing-canonical´
    #[test]
    fn ratio_backing_canonical() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::default())?;
        for (negative, numerator, denominator, expected) in [
            (false, 42_u8, 30_u8, (7_i64, 5_u64)),
            (true, 42, 30, (-7, 5)),
            (false, 0, 19, (0, 1)),
            (true, 0, 19, (0, 1)),
            (true, 18, 18, (-1, 1)),
        ] {
            let value = Rational::new(
                negative,
                &BigUint::from(numerator),
                &BigUint::from(denominator),
                &mut budget,
            )?;
            assert_eq!(value.value.numer(), &BigInt::from(expected.0));
            assert_eq!(value.value.denom(), &BigInt::from(expected.1));
            assert_eq!(
                value.is_negative(),
                value.value.numer().sign() == Sign::Minus
            );
        }
        Ok(())
    }

    /// Equivalent spellings have identical private components and canonical bytes.
    /// ´test:unit:ratio-backing-canonical-bytes´
    #[test]
    fn ratio_backing_canonical_bytes() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::default())?;
        for text in ["-1/2", "-2/4", "-0.5", "-5e-1"] {
            let value = Rational::parse(text, &mut budget)?;
            assert_eq!(value.value.numer(), &BigInt::from(-1));
            assert_eq!(value.value.denom(), &BigInt::from(2));
            assert_eq!(
                value.canonical_bytes(&mut budget)?,
                [1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 2]
            );
        }
        Ok(())
    }

    /// Sign-changing paths preserve a positive denominator and the cached sign.
    /// ´test:unit:ratio-backing-sign-operations´
    #[test]
    fn ratio_backing_sign_operations() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::default())?;
        let value = Rational::from_ratio(-2, 3, &mut budget)?;
        for (result, numerator, denominator) in [
            (value.negated(&mut budget)?, 2, 3),
            (Rational::one().div(&value, &mut budget)?, -3, 2),
            (value.pow(-3, &mut budget)?, -27, 8),
            (value.sub(&value, &mut budget)?, 0, 1),
            (Rational::zero().negated(&mut budget)?, 0, 1),
        ] {
            assert_eq!(result.value.numer(), &BigInt::from(numerator));
            assert_eq!(result.value.denom(), &BigInt::from(denominator));
            assert_eq!(
                result.is_negative(),
                result.value.numer().sign() == Sign::Minus
            );
        }
        Ok(())
    }

    /// Product size refusal precedes arithmetic, at the bound and one bit below.
    /// ´test:unit:product-preflight-bits´
    #[test]
    fn product_preflight_bits() -> Result<(), Fault> {
        let at = Location::new("product preflight test");
        let value = BigUint::from(15_u8);
        let mut exact = Budget::new(Limits {
            component_bits: 4,
            intermediate_bits: 8,
            ..Limits::default()
        })?;
        assert_eq!(
            product(&value, &value, &mut exact, at)?,
            BigUint::from(225_u8)
        );
        let mut short = Budget::new(Limits {
            intermediate_bits: 7,
            ..exact.limits().clone()
        })?;
        let before = short.usage();
        assert_eq!(
            product(&value, &value, &mut short, at),
            Err(Fault::exhausted(BudgetKind::IntermediateBits, 7, 8, at))
        );
        assert_eq!(short.usage(), before);
        Ok(())
    }

    /// Addition reserves its carry bit even when opposite signs could cancel.
    /// ´test:unit:sum-preflight-bits´
    #[test]
    fn sum_preflight_bits() -> Result<(), Fault> {
        let at = Location::new("sum preflight test");
        let mut exact = Budget::new(Limits {
            component_bits: 4,
            intermediate_bits: 5,
            ..Limits::default()
        })?;
        assert_eq!(
            signed_sum((false, 15_u8.into()), (false, 1_u8.into()), &mut exact, at)?,
            (false, BigUint::from(16_u8))
        );
        let mut short = Budget::new(Limits {
            intermediate_bits: 4,
            ..exact.limits().clone()
        })?;
        let before = short.usage();
        assert_eq!(
            signed_sum((false, 15_u8.into()), (true, 15_u8.into()), &mut short, at),
            Err(Fault::exhausted(BudgetKind::IntermediateBits, 4, 5, at))
        );
        assert_eq!(short.usage(), before);
        Ok(())
    }

    /// Product work covers a 65-bit predicted result before the dependency call.
    /// ´test:unit:product-preflight-work´
    #[test]
    fn product_preflight_work() -> Result<(), Fault> {
        let at = Location::new("product work test");
        let large = BigUint::from(u64::MAX);
        let unit = BigUint::from(1_u8);
        let mut exact = Budget::new(Limits {
            work: 72,
            arithmetic_steps: 1,
            ..Limits::default()
        })?;
        assert_eq!(product(&large, &unit, &mut exact, at)?, large);
        assert_eq!(exact.usage().work, 72);
        assert_eq!(exact.usage().arithmetic_steps, 1);
        let mut short = Budget::new(Limits {
            work: 71,
            ..exact.limits().clone()
        })?;
        let before = short.usage();
        assert_eq!(
            product(&large, &unit, &mut short, at),
            Err(Fault::exhausted(BudgetKind::Work, 71, 72, at))
        );
        assert_eq!(short.usage(), before);
        Ok(())
    }

    /// Division charges both outputs once and rejects zero before spending work.
    /// ´test:unit:division-preflight-steps´
    #[test]
    fn division_preflight_steps() -> Result<(), Fault> {
        let at = Location::new("division preflight test");
        let dividend = BigUint::from(17_u8);
        let divisor = BigUint::from(5_u8);
        let mut exact = Budget::new(Limits {
            arithmetic_steps: 1,
            ..Limits::default()
        })?;
        assert_eq!(
            divide(&dividend, &divisor, &mut exact, at)?,
            (BigUint::from(3_u8), BigUint::from(2_u8))
        );
        let mut short = Budget::new(Limits {
            arithmetic_steps: 0,
            ..exact.limits().clone()
        })?;
        let before = short.usage();
        assert_eq!(
            divide(&dividend, &divisor, &mut short, at),
            Err(Fault::exhausted(BudgetKind::ArithmeticSteps, 0, 1, at))
        );
        assert_eq!(
            divide(&dividend, &BigUint::from(0_u8), &mut short, at),
            Err(Fault::invalid(InvalidCode::ZeroDenominator, at))
        );
        assert_eq!(short.usage(), before);
        Ok(())
    }

    /// Division also preflights the dependency's normalized temporary dividend.
    /// ´test:unit:division-preflight-scratch´
    #[test]
    fn division_preflight_scratch() -> Result<(), Fault> {
        let at = Location::new("division scratch test");
        let dividend = BigUint::from(u128::MAX);
        let divisor = BigUint::from(u64::MAX) + BigUint::from(2_u8);
        let mut exact = Budget::new(Limits {
            component_bits: 128,
            intermediate_bits: 191,
            ..Limits::default()
        })?;
        assert_eq!(
            divide(&dividend, &divisor, &mut exact, at)?,
            (BigUint::from(u64::MAX), BigUint::from(0_u8))
        );
        let mut short = Budget::new(Limits {
            intermediate_bits: 190,
            ..exact.limits().clone()
        })?;
        let before = short.usage();
        assert_eq!(
            divide(&dividend, &divisor, &mut short, at),
            Err(Fault::exhausted(BudgetKind::IntermediateBits, 190, 191, at))
        );
        assert_eq!(short.usage(), before);
        Ok(())
    }

    /// Euclidean remainders stop on the first unpaid call without refunding work.
    /// ´test:unit:gcd-preflight-steps´
    #[test]
    fn gcd_preflight_steps() -> Result<(), Fault> {
        let at = Location::new("gcd preflight test");
        let left = BigUint::from(55_u8);
        let right = BigUint::from(34_u8);
        let mut exact = Budget::new(Limits {
            arithmetic_steps: 9,
            ..Limits::default()
        })?;
        assert_eq!(gcd(&left, &right, &mut exact, at)?, BigUint::from(1_u8));
        assert_eq!(exact.usage().arithmetic_steps, 9);
        assert_eq!(exact.usage().work, 9 * 32);
        let mut short = Budget::new(Limits {
            arithmetic_steps: 8,
            ..exact.limits().clone()
        })?;
        assert_eq!(
            gcd(&left, &right, &mut short, at),
            Err(Fault::exhausted(BudgetKind::ArithmeticSteps, 8, 9, at))
        );
        assert_eq!(short.usage().arithmetic_steps, 8);
        assert_eq!(short.usage().work, 8 * 32);
        Ok(())
    }

    /// Overflow in the dependency-work bound refuses before ledger mutation.
    /// ´test:unit:arithmetic-preflight-capacity´
    #[test]
    fn arithmetic_preflight_capacity() -> Result<(), Fault> {
        let at = Location::new("arithmetic capacity test");
        let mut budget = Budget::new(Limits::default())?;
        let before = budget.usage();
        for bits in [u64::MAX, u64::MAX - 63] {
            assert_eq!(
                charge(bits, 1, &mut budget, at),
                Err(Fault::capacity(BudgetKind::Work, at))
            );
            assert_eq!(budget.usage(), before);
        }
        Ok(())
    }

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
