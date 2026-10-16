// SPDX-License-Identifier: AGPL-3.0-only
//! Exact arithmetic of two real components.

use super::super::evaluate::Evaluation;
use super::super::proof::{AT, invalid};
use super::super::value::Value;
use crate::budget::Budget;
use crate::error::Fault;
use crate::solving::coefficients::forms::Form;
use crate::solving::coefficients::proof::fault;

/// Formal real components preserve independent root choices as independent variables.
pub(crate) struct Parts {
    /// Exact real component.
    pub(crate) real: Value,
    /// Exact imaginary component.
    pub(crate) imaginary: Value,
}

impl Parts {
    /// Rational real and imaginary constants embed through the standard fixed subfield.
    pub(crate) fn rational(
        evaluation: &Evaluation<'_>,
        real: &crate::number::Rational,
        imaginary: &crate::number::Rational,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let coefficient = |value, budget: &mut Budget| {
            let value = evaluation
                .engine
                .rational(value, budget)
                .map_err(|e| fault(&e))?;
            Value::polynomial(
                Form::constant(value, &evaluation.engine, budget).map_err(|e| fault(&e))?,
                &evaluation.engine,
                budget,
            )
        };
        Ok(Self {
            real: coefficient(real, budget)?,
            imaginary: coefficient(imaginary, budget)?,
        })
    }

    /// Componentwise addition and subtraction are exact over the underlying real polynomial ring.
    pub(crate) fn sum(
        &self,
        other: &Self,
        subtract: bool,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        if subtract {
            Ok(Self {
                real: self.real.sub(&other.real, &evaluation.engine, budget)?,
                imaginary: self
                    .imaginary
                    .sub(&other.imaginary, &evaluation.engine, budget)?,
            })
        } else {
            Ok(Self {
                real: self.real.add(&other.real, &evaluation.engine, budget)?,
                imaginary: self
                    .imaginary
                    .add(&other.imaginary, &evaluation.engine, budget)?,
            })
        }
    }

    /// Multiplication uses i²=-1, without identifying any unrelated algebraic choices.
    pub(crate) fn mul(
        &self,
        other: &Self,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let real = self
            .real
            .mul(&other.real, &evaluation.engine, budget)?
            .sub(
                &self
                    .imaginary
                    .mul(&other.imaginary, &evaluation.engine, budget)?,
                &evaluation.engine,
                budget,
            )?;
        let imaginary = self
            .real
            .mul(&other.imaginary, &evaluation.engine, budget)?
            .add(
                &self
                    .imaginary
                    .mul(&other.real, &evaluation.engine, budget)?,
                &evaluation.engine,
                budget,
            )?;
        Ok(Self { real, imaginary })
    }

    /// Conjugation negates exactly the imaginary component.
    pub(crate) fn conjugate(
        &self,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            real: self.real.copy(&evaluation.engine, budget)?,
            imaginary: self.imaginary.neg(&evaluation.engine, budget)?,
        })
    }

    /// Copy both real components without changing their denominators.
    pub(crate) fn copy(
        &self,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            real: self.real.copy(&evaluation.engine, budget)?,
            imaginary: self.imaginary.copy(&evaluation.engine, budget)?,
        })
    }

    /// Nonnegative powers are bounded repeated multiplication, independent of degree-specific formulas.
    pub(crate) fn power(
        &self,
        exponent: i64,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        if exponent < 0 {
            return Err(invalid());
        }
        let mut remaining = u64::try_from(exponent).map_err(|_| invalid())?;
        budget.check(crate::budget::BudgetKind::IntegerExponent, remaining, AT)?;
        let mut result = Self {
            real: Value::integer(1, &evaluation.engine, budget)?,
            imaginary: Value::integer(0, &evaluation.engine, budget)?,
        };
        let mut factor = Self {
            real: self.real.copy(&evaluation.engine, budget)?,
            imaginary: self.imaginary.copy(&evaluation.engine, budget)?,
        };
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.mul(&factor, evaluation, budget)?;
            }
            remaining >>= 1;
            if remaining > 0 {
                factor = factor.mul(&factor, evaluation, budget)?;
            }
        }
        Ok(result)
    }
}
