// SPDX-License-Identifier: AGPL-3.0-only
//! Finite exact rationals with reduction before checked narrowing.

use std::cmp::Ordering;
use std::fmt;

/// A refusal to construct an exact finite rational result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ArithmeticError {
    /// The reduced result or an intermediate exceeds the supported representation.
    #[error("exact rational arithmetic overflow")]
    Overflow,
    /// A denominator is zero.
    #[error("zero rational denominator")]
    ZeroDenominator,
    /// Zero raised to a nonpositive power is not admitted.
    #[error("zero to a nonpositive power")]
    UndefinedPower,
}

/// A reduced fraction with an i64 numerator and positive i64 denominator.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rational {
    /// Signed reduced numerator.
    numerator: i64,
    /// Strictly positive reduced denominator.
    denominator: i64,
}

impl Rational {
    /// The additive identity.
    pub const ZERO: Self = Self::integer(0);
    /// The multiplicative identity.
    pub const ONE: Self = Self::integer(1);

    /// Construct an integer without arithmetic.
    #[must_use]
    pub const fn integer(value: i64) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    /// Reduce a fraction and normalize its sign.
    ///
    /// # Errors
    /// Refuses a zero denominator or a reduced part outside the supported range.
    pub fn new(numerator: i64, denominator: i64) -> Result<Self, ArithmeticError> {
        Self::from_wide(i128::from(numerator), i128::from(denominator))
    }

    /// Reduce wide intermediate parts before checking their stored range.
    pub(crate) fn from_wide(mut n: i128, mut d: i128) -> Result<Self, ArithmeticError> {
        if d == 0 {
            return Err(ArithmeticError::ZeroDenominator);
        }
        if n == 0 {
            return Ok(Self::ZERO);
        }
        let mut a = n.unsigned_abs();
        let mut b = d.unsigned_abs();
        while b != 0 {
            (a, b) = (b, a % b);
        }
        let divisor = i128::try_from(a).map_err(|_| ArithmeticError::Overflow)?;
        n /= divisor;
        d /= divisor;
        if d < 0 {
            n = n.checked_neg().ok_or(ArithmeticError::Overflow)?;
            d = d.checked_neg().ok_or(ArithmeticError::Overflow)?;
        }
        Ok(Self {
            numerator: i64::try_from(n).map_err(|_| ArithmeticError::Overflow)?,
            denominator: i64::try_from(d).map_err(|_| ArithmeticError::Overflow)?,
        })
    }

    /// Reduced numerator.
    #[must_use]
    pub const fn numerator(self) -> i64 {
        self.numerator
    }

    /// Positive reduced denominator.
    #[must_use]
    pub const fn denominator(self) -> i64 {
        self.denominator
    }

    /// Whether this is an integer.
    #[must_use]
    pub const fn is_integer(self) -> bool {
        self.denominator == 1
    }

    /// Add exactly.
    ///
    /// # Errors
    /// Refuses a result outside the finite rational representation.
    pub fn checked_add(self, rhs: Self) -> Result<Self, ArithmeticError> {
        let left = i128::from(self.numerator) * i128::from(rhs.denominator);
        let right = i128::from(rhs.numerator) * i128::from(self.denominator);
        Self::from_wide(
            left.checked_add(right).ok_or(ArithmeticError::Overflow)?,
            i128::from(self.denominator) * i128::from(rhs.denominator),
        )
    }

    /// Subtract exactly without first negating a stored numerator.
    ///
    /// # Errors
    /// Refuses a result outside the finite rational representation.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, ArithmeticError> {
        let left = i128::from(self.numerator) * i128::from(rhs.denominator);
        let right = i128::from(rhs.numerator) * i128::from(self.denominator);
        Self::from_wide(
            left.checked_sub(right).ok_or(ArithmeticError::Overflow)?,
            i128::from(self.denominator) * i128::from(rhs.denominator),
        )
    }

    /// Multiply exactly, reducing before narrowing.
    ///
    /// # Errors
    /// Refuses a result outside the finite rational representation.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, ArithmeticError> {
        Self::from_wide(
            i128::from(self.numerator) * i128::from(rhs.numerator),
            i128::from(self.denominator) * i128::from(rhs.denominator),
        )
    }

    /// Divide exactly, reducing before narrowing.
    ///
    /// # Errors
    /// Refuses a zero divisor or an unrepresentable result.
    pub fn checked_div(self, rhs: Self) -> Result<Self, ArithmeticError> {
        Self::from_wide(
            i128::from(self.numerator) * i128::from(rhs.denominator),
            i128::from(self.denominator) * i128::from(rhs.numerator),
        )
    }

    /// Negate exactly.
    ///
    /// # Errors
    /// Refuses the unrepresentable negation of the minimum numerator.
    pub fn checked_neg(self) -> Result<Self, ArithmeticError> {
        Self::from_wide(-i128::from(self.numerator), i128::from(self.denominator))
    }

    /// Raise to an integer power by checked repeated squaring.
    ///
    /// # Errors
    /// Refuses zero to a nonpositive power and arithmetic overflow.
    pub fn checked_pow(self, exponent: i64) -> Result<Self, ArithmeticError> {
        if self == Self::ZERO && exponent <= 0 {
            return Err(ArithmeticError::UndefinedPower);
        }
        let mut base = if exponent < 0 {
            Self::ONE.checked_div(self)?
        } else {
            self
        };
        let mut bits = exponent.unsigned_abs();
        let mut result = Self::ONE;
        while bits != 0 {
            if bits & 1 != 0 {
                result = result.checked_mul(base)?;
            }
            bits >>= 1;
            if bits != 0 {
                base = base.checked_mul(base)?;
            }
        }
        Ok(result)
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        // Products of stored i64 parts fit in i128.
        (i128::from(self.numerator) * i128::from(other.denominator))
            .cmp(&(i128::from(other.numerator) * i128::from(self.denominator)))
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator == 1 {
            self.numerator.fmt(f)
        } else {
            write!(f, "{}/{}", self.numerator, self.denominator)
        }
    }
}
