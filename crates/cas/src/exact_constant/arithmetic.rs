// SPDX-License-Identifier: AGPL-3.0-only
//! Rational fast paths preserve the existing coefficient ledger; other operations use CAS graphs.

use super::{ExactConstant, graph};
use crate::budget::Budget;
use crate::error::Fault;
use crate::model::Operator;

impl ExactConstant {
    /// Add two finite exact constants.
    /// # Errors
    /// Refuses exhausted exact arithmetic or graph budgets.
    pub fn add(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        if let Some((left, right)) = self.as_rational().zip(other.as_rational()) {
            return Ok(left.add(right, budget)?.into());
        }
        graph::construct(Operator::Add, &[self, other], budget)
    }

    /// Subtract two finite exact constants.
    /// # Errors
    /// Refuses exhausted exact arithmetic or graph budgets.
    pub fn sub(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        if let Some((left, right)) = self.as_rational().zip(other.as_rational()) {
            return Ok(left.sub(right, budget)?.into());
        }
        graph::construct(Operator::Sub, &[self, other], budget)
    }

    /// Multiply two finite exact constants and cancel exact constant factors in the CAS.
    /// # Errors
    /// Refuses exhausted exact arithmetic or graph budgets.
    pub fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        if let Some((left, right)) = self.as_rational().zip(other.as_rational()) {
            return Ok(left.mul(right, budget)?.into());
        }
        graph::construct(Operator::Mul, &[self, other], budget)
    }

    /// Divide by an exact constant with a checked nonzero sign.
    /// # Errors
    /// Refuses a zero or unproved divisor and exhausted exact arithmetic or graph budgets.
    pub fn div(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        if let Some((left, right)) = self.as_rational().zip(other.as_rational()) {
            return Ok(left.div(right, budget)?.into());
        }
        graph::construct(Operator::Div, &[self, other], budget)
    }

    /// Negate an exact finite constant.
    /// # Errors
    /// Refuses exhausted exact arithmetic or graph budgets.
    pub fn negated(&self, budget: &mut Budget) -> Result<Self, Fault> {
        if let Some(value) = self.as_rational() {
            return Ok(value.negated(budget)?.into());
        }
        graph::construct(Operator::Neg, &[self], budget)
    }

    /// Raise an exact constant to an integer power under strict real conventions.
    /// # Errors
    /// Refuses an unproved domain for nonpositive powers or exhausted exact budgets.
    pub fn pow(&self, exponent: i64, budget: &mut Budget) -> Result<Self, Fault> {
        if let Some(value) = self.as_rational() {
            return Ok(value.pow(exponent, budget)?.into());
        }
        graph::construct(Operator::IntegerPower(exponent), &[self], budget)
    }
}

impl super::PositiveConstant {
    /// Take the positive real square root of a rational or rational-times-Pi-power radicand.
    /// # Errors
    /// Refuses radicands outside the rational and Pi monomial square-root fragment or exhausted exact budgets.
    pub fn sqrt(&self, budget: &mut Budget) -> Result<ExactConstant, Fault> {
        let half = ExactConstant::from(crate::Rational::from_ratio(1, 2, budget)?);
        graph::construct(
            Operator::GeneralPower(crate::model::Branch::StrictReal),
            &[self.value(), &half],
            budget,
        )
    }
}
