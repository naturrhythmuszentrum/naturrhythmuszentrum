// SPDX-License-Identifier: AGPL-3.0-only
//! Sign proposals factor exact parameter polynomials through the actual ancestor guards.

use super::leaf::{Leaf, Signed};
use crate::graph::Handle;
use crate::model::Operator;
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::{CoefficientFailure, Element, Value},
    polynomial::Polynomial,
    scalar::Scalar,
    storage::{invalid, reserve, vector},
};
use crate::solving::{RealSignConclusion as Sign, RealSignRule as Rule, SolveReason};

/// Each discovered factor retains the actual total guard expression and its multiplicity.
type Factors = Vec<(Handle, Sign, u32, Option<u32>)>;

/// Failure to find a finite factorization is a capability boundary, never a sign theorem.
const fn unavailable() -> CoefficientFailure {
    CoefficientFailure::Unsupported(SolveReason::RealIsolationUnavailable)
}

/// Preserve every sign possibility; a producer cannot silently strengthen a weak result.
const fn conclusion(mask: u8) -> Result<Sign, CoefficientFailure> {
    match mask {
        1 => Ok(Sign::Negative),
        2 => Ok(Sign::Zero),
        4 => Ok(Sign::Positive),
        3 => Ok(Sign::NonPositive),
        5 => Ok(Sign::NonZero),
        6 => Ok(Sign::NonNegative),
        _ => Err(unavailable()),
    }
}

impl Leaf<'_> {
    /// Group repeated divisors before sign arithmetic, so an even power of a nonzero factor is positive.
    fn factors(
        &mut self,
        polynomial: &Polynomial,
    ) -> Result<(Scalar, Factors), CoefficientFailure> {
        let mut remainder = polynomial.copy(self.budget)?;
        let mut factors = vector(self.guards.len(), self.budget)?;
        for guard in self.guards.iter().chain(&self.extra) {
            if matches!(
                guard.sign,
                Sign::Zero | Sign::NonNegative | Sign::NonPositive
            ) {
                continue;
            }
            let Value::Fraction(value) = &guard.value.value else {
                continue;
            };
            if value.numerator.scalar(self.budget)?.is_some() {
                continue;
            }
            let mut exponent = 0_u32;
            while let Some(quotient) =
                crate::solving::coefficients::gcd::exact(&remainder, &value.numerator, self.budget)?
            {
                if quotient.total_degree(self.budget)? >= remainder.total_degree(self.budget)? {
                    break;
                }
                remainder = quotient;
                exponent = exponent.checked_add(1).ok_or_else(invalid)?;
            }
            if exponent > 0 {
                reserve(&mut factors, 1, self.budget)?;
                factors.push((guard.expression.clone(), guard.sign, exponent, guard.proof));
            }
        }
        Ok((
            remainder.scalar(self.budget)?.ok_or_else(unavailable)?,
            factors,
        ))
    }

    /// Rational signs are proposed from exact comparison and independently replayed as literal rules.
    pub(crate) fn rational_sign(
        &mut self,
        value: &crate::number::Rational,
    ) -> Result<Signed, CoefficientFailure> {
        let sign = match value.compare(&crate::number::Rational::zero(), self.budget)? {
            std::cmp::Ordering::Less => Sign::Negative,
            std::cmp::Ordering::Equal => Sign::Zero,
            std::cmp::Ordering::Greater => Sign::Positive,
        };
        let value = crate::solving::coefficients::scalar::rational_copy(value, self.budget)?;
        let subject = self.expression(Operator::Rational(value), &[])?;
        Ok(self.derive(Rule::Rational, &subject, sign, &[], Vec::new(), 0)?)
    }

    /// Products and quotients retain their full sign sets and strict nonzero denominator evidence.
    pub(crate) fn product_sign(
        &mut self,
        left: &Signed,
        right: &Signed,
        divide: bool,
    ) -> Result<Signed, CoefficientFailure> {
        if divide {
            self.predicate(right, crate::model::PredicateKind::NonZero)?;
        }
        let subject = self.expression(
            if divide { Operator::Div } else { Operator::Mul },
            &[left.subject.clone(), right.subject.clone()],
        )?;
        let sign = conclusion(super::sign_rules::combine(
            super::sign::mask(left.sign),
            super::sign::mask(right.sign),
            true,
        ))?;
        Ok(self.derive(Rule::Product, &subject, sign, &[left, right], Vec::new(), 0)?)
    }

    /// Grouped powers expose the ordered-field parity rule instead of guessing a product sign.
    pub(crate) fn power_sign(
        &mut self,
        source: &Signed,
        exponent: u32,
    ) -> Result<Signed, CoefficientFailure> {
        let subject = self.expression(
            Operator::IntegerPower(i64::from(exponent)),
            std::slice::from_ref(&source.subject),
        )?;
        let (rule, sign) = if exponent.is_multiple_of(2) {
            (
                Rule::EvenPower,
                if source.sign == Sign::Zero {
                    Sign::Zero
                } else {
                    Sign::Positive
                },
            )
        } else {
            (Rule::OddPowerReflection, source.sign)
        };
        Ok(self.derive(rule, &subject, sign, &[source], Vec::new(), exponent)?)
    }

    /// Multiply the independently replayed guard factors and the exact rational residue.
    fn polynomial_sign(&mut self, polynomial: &Polynomial) -> Result<Signed, CoefficientFailure> {
        let (constant, factors) = self.factors(polynomial)?;
        let Scalar::Rational(value) = constant else {
            return Err(unavailable());
        };
        let mut result = self.rational_sign(&value)?;
        for (subject, sign, exponent, proof) in factors {
            let source = self.derive(
                Rule::Hypothesis,
                &subject,
                sign,
                &[],
                proof.into_iter().collect(),
                0,
            )?;
            let powered = self.power_sign(&source, exponent)?;
            result = self.product_sign(&result, &powered, false)?;
        }
        Ok(result)
    }

    /// Exact coefficients inherit signs only through a checked reconstruction of their full fraction.
    pub(crate) fn coefficient_sign(
        &mut self,
        coefficient: &Element,
    ) -> Result<Signed, CoefficientFailure> {
        let target = self
            .frame
            .literal(self.cas, coefficient, &self.map, self.budget)?;
        let engine = Engine {
            state: &self.cas.state,
            domain: &coefficient.domain,
        };
        if engine.zero_on(coefficient, &self.frame.region, self.budget)? {
            return Ok(self.derive(
                Rule::Identity,
                &target,
                Sign::Zero,
                &[],
                vec![self.unit],
                0,
            )?);
        }
        let value = match &coefficient.value {
            Value::Scalar(Scalar::Rational(value)) => self.rational_sign(value)?,
            Value::Fraction(value) => {
                let numerator = self.polynomial_sign(&value.numerator)?;
                let denominator = self.polynomial_sign(&value.denominator)?;
                self.product_sign(&numerator, &denominator, true)?
            }
            _ => return Err(unavailable()),
        };
        Ok(self.transport(&value, &target)?)
    }
}

impl Leaf<'_> {
    /// Preserve a derived numerator sign for subsequent fraction factors and regional unit proposals.
    pub(crate) fn remember(
        &mut self,
        coefficient: &Element,
        sign: &Signed,
    ) -> Result<Signed, CoefficientFailure> {
        let Value::Fraction(value) = &coefficient.value else {
            return Ok(self.transport(sign, &sign.subject)?);
        };
        let denominator = Element {
            domain: coefficient.domain.clone(),
            origin: None,
            value: Value::Fraction(crate::solving::coefficients::fraction::Fraction {
                numerator: value.denominator.copy(self.budget)?,
                denominator: value.denominator.one(self.budget)?,
            }),
        };
        let denominator = self.coefficient_sign(&denominator)?;
        let product = self.product_sign(sign, &denominator, false)?;
        let numerator = super::render::numerator(coefficient, self.budget)?;
        let expression = self
            .frame
            .literal(self.cas, &numerator, &self.map, self.budget)?;
        let proved = self.transport(&product, &expression)?;
        reserve(&mut self.extra, 1, self.budget)?;
        self.extra.push(super::leaf::Guard {
            value: numerator,
            expression,
            sign: proved.sign,
            proof: Some(proved.slot),
        });
        Ok(proved)
    }
}
