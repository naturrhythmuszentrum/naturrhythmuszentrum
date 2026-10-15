// SPDX-License-Identifier: AGPL-3.0-only
//! Strict rational expressions over the independently metered coefficient algebra.

use crate::budget::Budget;
use crate::error::Fault;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::{Form, Variable};
use crate::solving::coefficients::proof::fault;

/// Uncancelled denominator factors retain the defined domain of every input operation.
#[derive(Debug)]
pub(crate) struct Value {
    /// Polynomial numerator.
    pub(crate) numerator: Form,
    /// Polynomial denominator; no cancellation removes its obligations.
    pub(crate) denominator: Form,
}

impl Value {
    /// Embed an exact polynomial as a total rational expression.
    pub(crate) fn polynomial(
        numerator: Form,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let denominator = Form::constant(
            engine.constant(true, budget).map_err(|e| fault(&e))?,
            engine,
            budget,
        )
        .map_err(|e| fault(&e))?;
        Ok(Self {
            numerator,
            denominator,
        })
    }

    /// Construct a formal atom without assigning it any equation or nonzero property.
    pub(crate) fn variable(
        variable: Variable,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let polynomial = Form::variable(variable, engine, budget).map_err(|e| fault(&e))?;
        Self::polynomial(polynomial, engine, budget)
    }

    /// Copy both uncancelled components with metering.
    pub(crate) fn copy(&self, engine: &Engine<'_>, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            numerator: self.numerator.copy(engine, budget).map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .copy(engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Negation preserves every original denominator.
    pub(crate) fn neg(&self, engine: &Engine<'_>, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            numerator: self.numerator.neg(engine, budget).map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .copy(engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Cross multiplication preserves all strict-domain factors.
    pub(crate) fn add(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let left = self
            .numerator
            .mul(&other.denominator, engine, budget)
            .map_err(|e| fault(&e))?;
        let right = other
            .numerator
            .mul(&self.denominator, engine, budget)
            .map_err(|e| fault(&e))?;
        Ok(Self {
            numerator: left.add(&right, engine, budget).map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .mul(&other.denominator, engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Exact subtraction, with no semantic cancellation of holes.
    pub(crate) fn sub(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        self.add(&other.neg(engine, budget)?, engine, budget)
    }

    /// Multiplication retains the product of the input denominators.
    pub(crate) fn mul(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            numerator: self
                .numerator
                .mul(&other.numerator, engine, budget)
                .map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .mul(&other.denominator, engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Division is used only after the caller has checked the divisor's nonvanishing.
    pub(crate) fn div(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            numerator: self
                .numerator
                .mul(&other.denominator, engine, budget)
                .map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .mul(&other.numerator, engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Positive powers retain all denominator factors, including those of zero numerators.
    pub(crate) fn pow(
        &self,
        exponent: u32,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            numerator: self
                .numerator
                .pow(exponent, engine, budget)
                .map_err(|e| fault(&e))?,
            denominator: self
                .denominator
                .pow(exponent, engine, budget)
                .map_err(|e| fault(&e))?,
        })
    }

    /// Equality of already defined rational expressions by exact cross multiplication.
    pub(crate) fn equals(
        &self,
        other: &Self,
        engine: &Engine<'_>,
        budget: &mut Budget,
    ) -> Result<bool, Fault> {
        Ok(self.sub(other, engine, budget)?.numerator.terms.is_empty())
    }
}
