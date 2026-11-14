// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded unsigned arithmetic; no allocation and no floating conversions.

use super::DecimalError;
use std::cmp::Ordering;

/// Fixed 3456-bit unsigned magnitude, least significant limb first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Integer([u64; 54]);

impl Integer {
    /// Zero magnitude.
    pub(super) const ZERO: Self = Self([0; 54]);

    /// Embed one native limb.
    pub(super) const fn from_u64(value: u64) -> Self {
        let mut limbs = [0; 54];
        limbs[0] = value;
        Self(limbs)
    }

    /// Whether every limb is zero.
    pub(super) fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    /// Significant binary length.
    pub(super) fn bits(self) -> usize {
        self.0
            .iter()
            .rposition(|limb| *limb != 0)
            .map_or(0, |i| i * 64 + (64 - self.0[i].leading_zeros() as usize))
    }

    /// Checked unsigned addition.
    pub(super) fn add(self, rhs: Self) -> Result<Self, DecimalError> {
        let mut out = Self::ZERO;
        let mut carry = false;
        for ((slot, left), right) in out.0.iter_mut().zip(self.0).zip(rhs.0) {
            let (sum, first) = left.overflowing_add(right);
            let (sum, second) = sum.overflowing_add(u64::from(carry));
            *slot = sum;
            carry = first || second;
        }
        if carry {
            Err(DecimalError::Capacity)
        } else {
            Ok(out)
        }
    }

    /// Checked subtraction, refusing negative results.
    pub(super) fn sub(self, rhs: Self) -> Result<Self, DecimalError> {
        let mut out = Self::ZERO;
        let mut borrow = false;
        for ((slot, left), right) in out.0.iter_mut().zip(self.0).zip(rhs.0) {
            let (difference, first) = left.overflowing_sub(right);
            let (difference, second) = difference.overflowing_sub(u64::from(borrow));
            *slot = difference;
            borrow = first || second;
        }
        if borrow {
            Err(DecimalError::Capacity)
        } else {
            Ok(out)
        }
    }

    /// Checked binary shift.
    pub(super) fn shift(self, amount: usize) -> Result<Self, DecimalError> {
        if self.is_zero() {
            return Ok(self);
        }
        if amount > 3456 - self.bits() {
            return Err(DecimalError::Capacity);
        }
        let mut out = Self::ZERO;
        let words = amount / 64;
        let bits = amount % 64;
        for i in 0..(54 - words) {
            out.0[i + words] |= self.0[i] << bits;
            if bits != 0 && i + words + 1 < 54 {
                out.0[i + words + 1] |= self.0[i] >> (64 - bits);
            }
        }
        Ok(out)
    }

    /// Schoolbook multiplication with checked carry propagation.
    pub(super) fn mul(self, rhs: Self) -> Result<Self, DecimalError> {
        let mut out = Self::ZERO;
        for (i, left) in self.0.into_iter().enumerate() {
            if left == 0 {
                continue;
            }
            let mut carry = 0_u128;
            for (j, right) in rhs.0.into_iter().enumerate() {
                if i + j >= 54 {
                    if right != 0 || carry != 0 {
                        return Err(DecimalError::Capacity);
                    }
                    continue;
                }
                let value = u128::from(left) * u128::from(right) + u128::from(out.0[i + j]) + carry;
                out.0[i + j] = u64::try_from(value & u128::from(u64::MAX))
                    .map_err(|_| DecimalError::Capacity)?;
                carry = value >> 64;
            }
            if carry != 0 {
                return Err(DecimalError::Capacity);
            }
        }
        Ok(out)
    }

    /// Exact power of ten.
    pub(super) fn ten_pow(power: u32) -> Result<Self, DecimalError> {
        let mut out = Self::from_u64(1);
        for _ in 0..power {
            out = out.mul(Self::from_u64(10))?;
        }
        Ok(out)
    }

    /// Binary long division using shifted subtraction.
    pub(super) fn div_rem(self, divisor: Self) -> Result<(Self, Self), DecimalError> {
        if divisor.is_zero() {
            return Err(DecimalError::Denominator);
        }
        let mut remainder = self;
        let mut quotient = Self::ZERO;
        if self < divisor {
            return Ok((quotient, remainder));
        }
        for shift in (0..=self.bits() - divisor.bits()).rev() {
            let candidate = divisor.shift(shift)?;
            if remainder >= candidate {
                remainder = remainder.sub(candidate)?;
                quotient.0[shift / 64] |= 1_u64 << (shift % 64);
            }
        }
        Ok((quotient, remainder))
    }

    /// Euclidean greatest common divisor.
    pub(super) fn gcd(mut self, mut rhs: Self) -> Result<Self, DecimalError> {
        while !rhs.is_zero() {
            let remainder = self.div_rem(rhs)?.1;
            self = rhs;
            rhs = remainder;
        }
        Ok(self)
    }

    /// Extract one limb only when all higher limbs vanish.
    pub(super) fn to_u64(self) -> Result<u64, DecimalError> {
        if self.0[1..].iter().any(|limb| *limb != 0) {
            return Err(DecimalError::Capacity);
        }
        Ok(self.0[0])
    }

    /// Decimal integer digits without a sign or leading zeros.
    pub(super) fn digits(mut self) -> Result<String, DecimalError> {
        if self.is_zero() {
            return Ok("0".to_owned());
        }
        let mut digits = Vec::new();
        let ten = Self::from_u64(10);
        while !self.is_zero() {
            let (quotient, remainder) = self.div_rem(ten)?;
            let digit = u8::try_from(remainder.0[0]).map_err(|_| DecimalError::Capacity)?;
            digits.push(b'0' + digit);
            self = quotient;
        }
        digits.reverse();
        String::from_utf8(digits).map_err(|_| DecimalError::Source)
    }
}

impl Ord for Integer {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.0.iter().rev().cmp(rhs.0.iter().rev())
    }
}

impl PartialOrd for Integer {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
