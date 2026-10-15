// SPDX-License-Identifier: AGPL-3.0-only
//! Ground-field values, with all arithmetic delegated to prepaid numeric operations.

use super::storage::{AT, add, begin, invalid, product};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::number::Rational;
use crate::solving::numeric::Gaussian;

/// The admitted exact ground fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Ground {
    /// Rational coefficients.
    Rational,
    /// Gaussian rational coefficients.
    Gaussian,
}

/// Owned arithmetic scratch, deliberately without infallible cloning or operators.
#[derive(Debug)]
pub(crate) enum Scalar {
    /// A rational scalar.
    Rational(Rational),
    /// A Gaussian scalar.
    Gaussian(Gaussian),
}

/// Reserve a rational copy, including its retained limbs, before cloning.
pub(crate) fn rational_copy(value: &Rational, budget: &mut Budget) -> Result<Rational, Fault> {
    value.check(budget, AT)?;
    let (numerator, denominator) = value.component_bits();
    let limbs = add(numerator.div_ceil(64), denominator.div_ceil(64))?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        add(product(limbs, 8)?, 64)?,
        AT,
    )?;
    budget.arithmetic(limbs.max(1), AT)?;
    Ok(value.clone())
}

impl Scalar {
    /// The exact ground field is part of the scratch value's type check.
    pub(crate) const fn ground(&self) -> Ground {
        match self {
            Self::Rational(_) => Ground::Rational,
            Self::Gaussian(_) => Ground::Gaussian,
        }
    }

    /// A bounded additive or multiplicative identity.
    pub(crate) fn constant(
        ground: Ground,
        nonzero: bool,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        begin(budget)?;
        match ground {
            Ground::Rational => {
                budget.check(BudgetKind::ComponentBits, 1, AT)?;
                budget.check(BudgetKind::StagedPayloadBytes, 80, AT)?;
                budget.arithmetic(1, AT)?;
                Ok(Self::Rational(if nonzero {
                    Rational::one()
                } else {
                    Rational::zero()
                }))
            }
            Ground::Gaussian => Ok(Self::Gaussian(if nonzero {
                Gaussian::one(budget)?
            } else {
                Gaussian::zero(budget)?
            })),
        }
    }

    /// Copy without bypassing the caller's ledger.
    pub(crate) fn copy(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        Ok(match self {
            Self::Rational(value) => Self::Rational(rational_copy(value, budget)?),
            Self::Gaussian(value) => Self::Gaussian(value.copied(budget)?),
        })
    }

    /// Exact sign change in the declared ground field.
    pub(crate) fn neg(&self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        Ok(match self {
            Self::Rational(value) => Self::Rational(value.negated(budget)?),
            Self::Gaussian(value) => Self::Gaussian(value.negated(budget)?),
        })
    }

    /// Ground-field addition.
    pub(crate) fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        match (self, other) {
            (Self::Rational(left), Self::Rational(right)) => {
                Ok(Self::Rational(left.add(right, budget)?))
            }
            (Self::Gaussian(left), Self::Gaussian(right)) => {
                Ok(Self::Gaussian(left.add(right, budget)?))
            }
            _ => Err(invalid()),
        }
    }

    /// Ground-field subtraction.
    pub(crate) fn sub(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        self.add(&other.neg(budget)?, budget)
    }

    /// Ground-field multiplication.
    pub(crate) fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        match (self, other) {
            (Self::Rational(left), Self::Rational(right)) => {
                Ok(Self::Rational(left.mul(right, budget)?))
            }
            (Self::Gaussian(left), Self::Gaussian(right)) => {
                Ok(Self::Gaussian(left.mul(right, budget)?))
            }
            _ => Err(invalid()),
        }
    }

    /// Ground-field division rejects zero before invoking dependency arithmetic.
    pub(crate) fn div(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        match (self, other) {
            (Self::Rational(left), Self::Rational(right)) => {
                Ok(Self::Rational(left.div(right, budget)?))
            }
            (Self::Gaussian(left), Self::Gaussian(right)) => {
                Ok(Self::Gaussian(left.div(right, budget)?))
            }
            _ => Err(invalid()),
        }
    }

    /// Meter canonical zero inspection; formal polynomial nonzero uses a separate regional decision.
    pub(crate) fn zero(&self, budget: &mut Budget) -> Result<bool, Fault> {
        begin(budget)?;
        match self {
            Self::Rational(value) => {
                value.check(budget, AT)?;
                budget.arithmetic(add(value.component_bits().0.div_ceil(64), 1)?, AT)?;
                Ok(value.is_zero())
            }
            Self::Gaussian(value) => value.is_zero(budget),
        }
    }

    /// Canonical equality charges all component comparisons.
    pub(crate) fn equals(&self, other: &Self, budget: &mut Budget) -> Result<bool, Fault> {
        begin(budget)?;
        match (self, other) {
            (Self::Rational(left), Self::Rational(right)) => {
                Ok(left.compare(right, budget)? == std::cmp::Ordering::Equal)
            }
            (Self::Gaussian(left), Self::Gaussian(right)) => left.equals(right, budget),
            _ => Ok(false),
        }
    }

    /// Positive powers use a bounded repeated-squaring loop; power zero is the ring identity.
    pub(crate) fn pow(&self, exponent: u32, budget: &mut Budget) -> Result<Self, Fault> {
        begin(budget)?;
        budget.check(BudgetKind::IntegerExponent, u64::from(exponent), AT)?;
        let mut result = Self::constant(self.ground(), true, budget)?;
        let mut factor = self.copy(budget)?;
        let mut remaining = exponent;
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.mul(&factor, budget)?;
            }
            remaining >>= 1;
            if remaining > 0 {
                factor = factor.mul(&factor, budget)?;
            }
        }
        Ok(result)
    }
}
