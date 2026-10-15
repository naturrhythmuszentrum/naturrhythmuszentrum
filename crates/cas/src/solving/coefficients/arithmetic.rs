// SPDX-License-Identifier: AGPL-3.0-only
//! The shared fallible arithmetic implementation for ground and fraction domains.

use super::fraction::Fraction;
use super::model::{CoefficientFailure, Element, Value};
use super::polynomial::Polynomial;
use super::scalar::{Ground, Scalar};
use super::storage::{AT, begin, domain, invalid, value};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Handle, State};
use crate::solving::{CoefficientDomainData, CoefficientValueData, SolveDraft};

/// A bounded view of an immutable graph-owned coefficient domain.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Engine<'a> {
    /// Mathematical graph, never mutated by arithmetic scratch operations.
    pub(crate) state: &'a State,
    /// Exact domain identity.
    pub(crate) domain: &'a Handle,
}

impl Engine<'_> {
    /// Validate an arithmetic operand's source domain.
    fn validate(&self, value: &Element) -> Result<(), Fault> {
        if value.domain != *self.domain {
            return Err(invalid());
        }
        Ok(())
    }

    /// Wrap a newly computed scratch value without giving it graph identity.
    pub(crate) fn element(&self, value: Value) -> Element {
        Element {
            domain: self.domain.clone(),
            value,
            origin: None,
        }
    }

    /// Load an admitted value without cloning any unbounded payload for free.
    pub(crate) fn read(
        &self,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        let data = value(self.state, handle)?;
        if data.domain != *self.domain {
            return Err(invalid().into());
        }
        let mut result = match &data.value {
            CoefficientValueData::Rational { .. } | CoefficientValueData::Gaussian { .. } => self
                .element(Value::Scalar(super::read::scalar(
                    self.state, handle, budget,
                )?)),
            CoefficientValueData::Fraction {
                numerator,
                denominator,
            } => self.element(Value::Fraction(Fraction {
                numerator: super::read::sparse(self.state, numerator, budget)?,
                denominator: super::read::sparse(self.state, denominator, budget)?,
            })),
            CoefficientValueData::Tower { coefficients } => {
                crate::solving::towers::arithmetic::read(self, coefficients, budget)?
            }
        };
        result.origin = Some(handle.clone());
        Ok(result)
    }

    /// Identify the admitted ground and variable count of a fraction domain.
    pub(crate) fn fraction_shape(&self) -> Result<(Ground, usize, &Handle), Fault> {
        let CoefficientDomainData::Fraction { base, coordinates } =
            domain(self.state, self.domain)?
        else {
            return Err(invalid());
        };
        let SolveDraft::CoordinateSpace(product) =
            super::super::graph::object(self.state, coordinates.logical_id())?
        else {
            return Err(invalid());
        };
        Ok((
            super::read::ground(self.state, base)?,
            product.coordinates.len(),
            base,
        ))
    }

    /// Construct a zero or one under the caller's finite limits.
    pub(crate) fn constant(
        &self,
        nonzero: bool,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        let result = match domain(self.state, self.domain)? {
            CoefficientDomainData::Rational => {
                Value::Scalar(Scalar::constant(Ground::Rational, nonzero, budget)?)
            }
            CoefficientDomainData::Gaussian => {
                Value::Scalar(Scalar::constant(Ground::Gaussian, nonzero, budget)?)
            }
            CoefficientDomainData::Fraction { .. } => {
                let (ground, variables, _) = self.fraction_shape()?;
                let numerator = Polynomial::constant(
                    variables,
                    Scalar::constant(ground, nonzero, budget)?,
                    budget,
                )?;
                let denominator = numerator.one(budget)?;
                Value::Fraction(Fraction {
                    numerator,
                    denominator,
                })
            }
            CoefficientDomainData::Quotient { .. } => {
                return crate::solving::towers::arithmetic::constant(self, nonzero, budget);
            }
        };
        Ok(self.element(result))
    }

    /// Copy an operand through its exact numeric or polynomial representation.
    pub(crate) fn copy(
        &self,
        operand: &Element,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        self.validate(operand)?;
        let mut result = self.element(match &operand.value {
            Value::Scalar(value) => Value::Scalar(value.copy(budget)?),
            Value::Fraction(value) => Value::Fraction(value.copy(budget)?),
            Value::Tower(_) => Value::Tower(
                crate::solving::towers::arithmetic::dense(self, operand, budget)?.coefficients,
            ),
        });
        result.origin.clone_from(&operand.origin);
        Ok(result)
    }

    /// Negation preserves canonical denominators.
    pub(crate) fn neg(
        &self,
        operand: &Element,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        self.validate(operand)?;
        Ok(self.element(match &operand.value {
            Value::Scalar(value) => Value::Scalar(value.neg(budget)?),
            Value::Fraction(value) => Value::Fraction(value.neg(budget)?),
            Value::Tower(_) => {
                return crate::solving::towers::arithmetic::neg(self, operand, budget);
            }
        }))
    }

    /// Addition requires identical domains and normalizes every rational-function result.
    pub(crate) fn add(
        &self,
        left: &Element,
        right: &Element,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        self.validate(left)?;
        self.validate(right)?;
        Ok(self.element(match (&left.value, &right.value) {
            (Value::Scalar(a), Value::Scalar(b)) => Value::Scalar(a.add(b, budget)?),
            (Value::Fraction(a), Value::Fraction(b)) => Value::Fraction(a.add(b, budget)?),
            (Value::Tower(_), Value::Tower(_)) => {
                return crate::solving::towers::arithmetic::add(self, left, right, budget);
            }
            _ => return Err(invalid().into()),
        }))
    }

    /// Subtraction uses only fallible sign and sum operations.
    pub(crate) fn sub(
        &self,
        left: &Element,
        right: &Element,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        self.add(left, &self.neg(right, budget)?, budget)
    }

    /// Multiplication charges recursive ground arithmetic and complete uncancelled term bounds.
    pub(crate) fn mul(
        &self,
        left: &Element,
        right: &Element,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        self.validate(left)?;
        self.validate(right)?;
        Ok(self.element(match (&left.value, &right.value) {
            (Value::Scalar(a), Value::Scalar(b)) => Value::Scalar(a.mul(b, budget)?),
            (Value::Fraction(a), Value::Fraction(b)) => Value::Fraction(a.mul(b, budget)?),
            (Value::Tower(_), Value::Tower(_)) => {
                return crate::solving::towers::arithmetic::mul(self, left, right, budget);
            }
            _ => return Err(invalid().into()),
        }))
    }

    /// Power zero is the formal ring identity; every positive power uses repeated squaring.
    pub(crate) fn pow(
        &self,
        value: &Element,
        exponent: u32,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        self.validate(value)?;
        budget.check(BudgetKind::IntegerExponent, u64::from(exponent), AT)?;
        let mut result = self.constant(true, budget)?;
        let mut factor = self.copy(value, budget)?;
        let mut remaining = exponent;
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = self.mul(&result, &factor, budget)?;
            }
            remaining >>= 1;
            if remaining > 0 {
                factor = self.mul(&factor, &factor, budget)?;
            }
        }
        Ok(result)
    }

    /// Recheck the canonical representation; no unnormalized fraction escapes.
    pub(crate) fn reduce(
        &self,
        operand: &Element,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        self.validate(operand)?;
        match &operand.value {
            Value::Scalar(_) | Value::Tower(_) => self.copy(operand, budget),
            Value::Fraction(value) => Ok(self.element(Value::Fraction(Fraction::new(
                &value.numerator,
                &value.denominator,
                budget,
            )?))),
        }
    }
}

impl Engine<'_> {
    /// Formal zero is used only for canonical algebraic reduction, never as a unit decision.
    pub(crate) fn formal_zero(
        &self,
        element: &Element,
        budget: &mut Budget,
    ) -> Result<bool, CoefficientFailure> {
        begin(budget)?;
        self.validate(element)?;
        match &element.value {
            Value::Scalar(value) => Ok(value.zero(budget)?),
            Value::Fraction(value) => Ok(value.numerator.zero(budget)?),
            Value::Tower(values) => {
                let (_, extension) = crate::solving::towers::arithmetic::level(self)?;
                let parent = Engine {
                    state: self.state,
                    domain: &extension.parent,
                };
                for value in values {
                    if !parent.formal_zero(value, budget)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    /// Equality in the formal algebra, with no claim about specialization definedness.
    pub(crate) fn formal_equal(
        &self,
        left: &Element,
        right: &Element,
        budget: &mut Budget,
    ) -> Result<bool, CoefficientFailure> {
        begin(budget)?;
        self.formal_zero(&self.sub(left, right, budget)?, budget)
    }
}

impl Engine<'_> {
    /// Canonical rational constants enter each parent before a child quotient.
    pub(crate) fn rational(
        &self,
        scalar: &crate::number::Rational,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        match domain(self.state, self.domain)? {
            CoefficientDomainData::Rational => Ok(self.element(Value::Scalar(Scalar::Rational(
                super::scalar::rational_copy(scalar, budget)?,
            )))),
            CoefficientDomainData::Gaussian => {
                let zero = crate::number::Rational::zero();
                Ok(self.element(Value::Scalar(Scalar::Gaussian(
                    crate::solving::numeric::Gaussian::from_parts(scalar, &zero, budget)?,
                ))))
            }
            CoefficientDomainData::Fraction { base, .. } => {
                let parent = Self {
                    state: self.state,
                    domain: base,
                };
                super::maps::constant(
                    self.state,
                    &parent.rational(scalar, budget)?,
                    self.domain,
                    budget,
                )
            }
            CoefficientDomainData::Quotient { .. } => {
                let (_, extension) = crate::solving::towers::arithmetic::level(self)?;
                let parent = Self {
                    state: self.state,
                    domain: &extension.parent,
                };
                let mut entries = super::storage::vector(1, budget)?;
                entries.push(parent.rational(scalar, budget)?);
                let value = crate::solving::towers::dense::Dense::new(&parent, entries, budget)?;
                crate::solving::towers::arithmetic::reduced(self, value, budget)
            }
        }
    }
}

impl Engine<'_> {
    /// Canonical parent inclusion is injective because the child defining relation is monic.
    pub(crate) fn include_parent(
        &self,
        value: &Element,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        let (_, extension) = crate::solving::towers::arithmetic::level(self)?;
        if value.domain != extension.parent {
            return Err(invalid().into());
        }
        let parent = Self {
            state: self.state,
            domain: &extension.parent,
        };
        let mut coefficients = super::storage::vector(1, budget)?;
        coefficients.push(parent.copy(value, budget)?);
        let polynomial = crate::solving::towers::dense::Dense::new(&parent, coefficients, budget)?;
        crate::solving::towers::arithmetic::reduced(self, polynomial, budget)
    }
}
