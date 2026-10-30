// SPDX-License-Identifier: AGPL-3.0-only
//! Exact endpoint operations never interpret overlap as equality.

use std::cmp::Ordering;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::number::Rational;

/// Location of interval-domain and resource refusals.
const AT: Location = Location::new("rational interval");

/// A validated closed real interval with canonical, ordered rational endpoints.
/// An interval alone proves neither an expression's range nor a source hypothesis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Interval {
    /// Inclusive lower endpoint.
    lower: Rational,
    /// Inclusive upper endpoint.
    upper: Rational,
}

impl Interval {
    /// Validate endpoints before taking ownership.
    /// # Errors
    /// Refuses reversed endpoints or exhausted arithmetic and component limits.
    pub fn new(lower: Rational, upper: Rational, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        if lower.compare(&upper, budget)? == Ordering::Greater {
            return Err(Fault::invalid(InvalidCode::DecisionBox, AT));
        }
        Ok(Self { lower, upper })
    }

    /// Borrow the exact lower endpoint.
    #[must_use]
    pub const fn lower(&self) -> &Rational {
        &self.lower
    }

    /// Borrow the exact upper endpoint.
    #[must_use]
    pub const fn upper(&self) -> &Rational {
        &self.upper
    }

    /// Compute the exact nonnegative width.
    /// # Errors
    /// Refuses arithmetic or size exhaustion.
    pub fn width(&self, budget: &mut Budget) -> Result<Rational, Fault> {
        scratch(budget)?;
        self.upper.sub(&self.lower, budget)
    }

    /// Copy a singleton under the active payload and arithmetic limits.
    /// # Errors
    /// Refuses a component or Work limit.
    pub fn singleton(value: &Rational, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        Self::new(copy(value, budget)?, copy(value, budget)?, budget)
    }

    /// Round outward to the dyadic grid of spacing two to minus `precision`.
    /// # Errors
    /// Refuses an excessive exponent, component, intermediate or Work requirement.
    pub fn coarsen(&self, precision: u32, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        let scale = scale(precision, budget)?;
        let lower = self
            .lower
            .mul(&scale, budget)?
            .floor(budget)?
            .div(&scale, budget)?;
        let upper = self
            .upper
            .negated(budget)?
            .mul(&scale, budget)?
            .floor(budget)?
            .negated(budget)?
            .div(&scale, budget)?;
        Self::new(lower, upper, budget)
    }

    /// Add endpoints and round outward on the requested grid.
    /// # Errors
    /// Refuses arithmetic, size or Work exhaustion.
    pub fn add(&self, rhs: &Self, precision: u32, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        Self::new(
            self.lower.add(&rhs.lower, budget)?,
            self.upper.add(&rhs.upper, budget)?,
            budget,
        )?
        .coarsen(precision, budget)
    }

    /// Subtract using the reversed endpoints of the subtrahend.
    /// # Errors
    /// Refuses arithmetic, size or Work exhaustion.
    pub fn sub(&self, rhs: &Self, precision: u32, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        Self::new(
            self.lower.sub(&rhs.upper, budget)?,
            self.upper.sub(&rhs.lower, budget)?,
            budget,
        )?
        .coarsen(precision, budget)
    }

    /// Enclose all four endpoint products, then round outward.
    /// # Errors
    /// Refuses arithmetic, size or Work exhaustion.
    pub fn mul(&self, rhs: &Self, precision: u32, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        let mut lower = self.lower.mul(&rhs.lower, budget)?;
        let mut upper = copy(&lower, budget)?;
        for (left, right) in [
            (&self.lower, &rhs.upper),
            (&self.upper, &rhs.lower),
            (&self.upper, &rhs.upper),
        ] {
            let product = left.mul(right, budget)?;
            if product.compare(&lower, budget)? == Ordering::Less {
                lower = copy(&product, budget)?;
            }
            if product.compare(&upper, budget)? == Ordering::Greater {
                upper = product;
            }
        }
        Self::new(lower, upper, budget)?.coarsen(precision, budget)
    }

    /// Invert a strictly same-sign interval; a zero-containing divisor refuses.
    /// # Errors
    /// Refuses a zero-containing interval or arithmetic and resource limits.
    pub fn reciprocal(&self, precision: u32, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        if self.lower.is_zero()
            || self.upper.is_zero()
            || self.lower.is_negative() != self.upper.is_negative()
        {
            return Err(Fault::invalid(InvalidCode::ZeroDenominator, AT));
        }
        Self::new(
            Rational::one().div(&self.upper, budget)?,
            Rational::one().div(&self.lower, budget)?,
            budget,
        )?
        .coarsen(precision, budget)
    }

    /// Enclose absolute value, retaining zero when the source crosses it.
    /// # Errors
    /// Refuses arithmetic and resource limits.
    pub fn abs(&self, precision: u32, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        let result = if self.upper.is_negative() {
            Self::new(
                self.upper.negated(budget)?,
                self.lower.negated(budget)?,
                budget,
            )?
        } else if self.lower.is_negative() {
            let negative = self.lower.negated(budget)?;
            let upper = if negative.compare(&self.upper, budget)? == Ordering::Greater {
                negative
            } else {
                copy(&self.upper, budget)?
            };
            Self::new(Rational::zero(), upper, budget)?
        } else {
            self.copied(budget)?
        };
        result.coarsen(precision, budget)
    }

    /// Enclose the maximum of two independently varying values.
    /// # Errors
    /// Refuses arithmetic and resource limits.
    pub fn maximum(&self, rhs: &Self, precision: u32, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        let lower = select(&self.lower, &rhs.lower, Ordering::Greater, budget)?;
        let upper = select(&self.upper, &rhs.upper, Ordering::Greater, budget)?;
        Self::new(lower, upper, budget)?.coarsen(precision, budget)
    }

    /// Intersect bounds whose caller has established identical subject and premises.
    /// This arithmetic operation does not itself establish that compatibility.
    /// # Errors
    /// Refuses an empty intersection or arithmetic and resource limits.
    pub fn intersection(&self, rhs: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        let lower = select(&self.lower, &rhs.lower, Ordering::Greater, budget)?;
        let upper = select(&self.upper, &rhs.upper, Ordering::Less, budget)?;
        Self::new(lower, upper, budget)
    }

    /// Test containment by exact endpoint comparisons, not overlap.
    /// # Errors
    /// Refuses arithmetic and resource limits.
    pub fn contains(&self, rhs: &Self, budget: &mut Budget) -> Result<bool, Fault> {
        scratch(budget)?;
        Ok(self.lower.compare(&rhs.lower, budget)? != Ordering::Greater
            && self.upper.compare(&rhs.upper, budget)? != Ordering::Less)
    }

    /// Copy endpoints through metered exact arithmetic.
    pub(super) fn copied(&self, budget: &mut Budget) -> Result<Self, Fault> {
        scratch(budget)?;
        Self::new(
            copy(&self.lower, budget)?,
            copy(&self.upper, budget)?,
            budget,
        )
    }
}

/// Copy a rational through its metered arithmetic route.
fn copy(value: &Rational, budget: &mut Budget) -> Result<Rational, Fault> {
    value.add(&Rational::zero(), budget)
}

/// Bound all simultaneously live endpoint and bigint temporaries before arithmetic.
pub(super) const fn scratch(budget: &Budget) -> Result<(), Fault> {
    let bytes = budget
        .limits()
        .intermediate_bits
        .div_ceil(8)
        .saturating_mul(64)
        .saturating_add(1024);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)
}

/// Select the requested endpoint extremum without trusting floating point.
fn select(
    left: &Rational,
    right: &Rational,
    order: Ordering,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    copy(
        if left.compare(right, budget)? == order {
            left
        } else {
            right
        },
        budget,
    )
}

/// Construct the checked dyadic grid denominator.
fn scale(precision: u32, budget: &mut Budget) -> Result<Rational, Fault> {
    let bits = u64::from(precision) + 1;
    budget.check(BudgetKind::ComponentBits, bits, AT)?;
    budget.check(BudgetKind::IntermediateBits, bits, AT)?;
    Rational::from_ratio(2, 1, budget)?.pow(i64::from(precision), budget)
}

#[cfg(test)]
mod tests;
