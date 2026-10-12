// SPDX-License-Identifier: AGPL-3.0-only
//! Exact vectors in the fixed seven-dimensional SI basis.

use nrz_algebra::Rational;

use crate::UnitError;

/// Basis order for every dimension vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseDimension {
    /// Metre basis.
    Length,
    /// Kilogram basis.
    Mass,
    /// Second basis.
    Time,
    /// Ampere basis.
    Current,
    /// Kelvin basis.
    Temperature,
    /// Mole basis, distinct from counts.
    Amount,
    /// Candela basis.
    LuminousIntensity,
}

/// Runtime admission policy; this does not prescribe any compile-time encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExponentPolicy {
    /// Admit every representable rational result dimension.
    Rational,
    /// Require every resulting exponent to be an integer.
    Integral,
}

/// A dimension with no scale, origin, semantic kind or numerical value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Dimension([Rational; 7]);

impl Dimension {
    /// The zero vector; not a semantic permission to mix kinds.
    pub const ZERO: Self = Self([Rational::ZERO; 7]);

    /// Construct a vector in the documented basis order.
    #[must_use]
    pub const fn new(exponents: [Rational; 7]) -> Self {
        Self(exponents)
    }

    /// Construct integer exponents without a numerical conversion.
    #[must_use]
    pub fn integers(exponents: [i64; 7]) -> Self {
        Self(exponents.map(Rational::integer))
    }

    /// One basis dimension.
    #[must_use]
    pub const fn base(base: BaseDimension) -> Self {
        let index = match base {
            BaseDimension::Length => 0,
            BaseDimension::Mass => 1,
            BaseDimension::Time => 2,
            BaseDimension::Current => 3,
            BaseDimension::Temperature => 4,
            BaseDimension::Amount => 5,
            BaseDimension::LuminousIntensity => 6,
        };
        let mut exponents = [Rational::ZERO; 7];
        exponents[index] = Rational::ONE;
        Self(exponents)
    }

    /// Exponents in fixed SI basis order.
    #[must_use]
    pub const fn exponents(&self) -> &[Rational; 7] {
        &self.0
    }

    /// Product dimensions add exponents.
    ///
    /// # Errors
    /// Refuses rational exponent overflow.
    pub fn product(self, rhs: Self) -> Result<Self, UnitError> {
        let mut result = self.0;
        for (value, other) in result.iter_mut().zip(rhs.0) {
            *value = value.checked_add(other)?;
        }
        Ok(Self(result))
    }

    /// Quotient dimensions subtract exponents.
    ///
    /// # Errors
    /// Refuses rational exponent overflow.
    pub fn quotient(self, rhs: Self) -> Result<Self, UnitError> {
        let mut result = self.0;
        for (value, other) in result.iter_mut().zip(rhs.0) {
            *value = value.checked_sub(other)?;
        }
        Ok(Self(result))
    }

    /// Power dimensions scale every exponent before checking admission.
    ///
    /// # Errors
    /// Refuses overflow or fractional resulting dimensions under an integral policy.
    pub fn power(self, exponent: Rational, policy: ExponentPolicy) -> Result<Self, UnitError> {
        let mut result = self.0;
        for value in &mut result {
            *value = value.checked_mul(exponent)?;
        }
        let dimension = Self(result);
        dimension.admit(policy)?;
        Ok(dimension)
    }

    /// Check whether this descriptor is admitted by an exponent policy.
    ///
    /// # Errors
    /// Refuses a fractional exponent under an integral policy.
    pub fn admit(self, policy: ExponentPolicy) -> Result<(), UnitError> {
        if policy == ExponentPolicy::Integral && self.0.iter().any(|value| !value.is_integer()) {
            Err(UnitError::NonIntegralDimension)
        } else {
            Ok(())
        }
    }
}
