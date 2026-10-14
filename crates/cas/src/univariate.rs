// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded exact univariate coefficient arithmetic, including Euclidean division.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::size;
use crate::number::Rational;

/// A temporary ascending-degree coefficient vector with no trailing zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Polynomial {
    /// Empty means the zero polynomial; interior zero coefficients are retained.
    pub(crate) coefficients: Vec<Rational>,
}

/// Polynomial arithmetic accounting location.
const AT: Location = Location::new("univariate arithmetic");

impl Polynomial {
    /// Validate an untrusted canonical coefficient vector before copying it.
    pub(crate) fn new(values: &[Rational], budget: &mut Budget) -> Result<Self, Fault> {
        validate(values, budget)?;
        let bytes = payload(values, budget)?;
        scratch(capacity(values.len(), budget)?, budget)?;
        budget.work(bytes, AT)?;
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(values.len())
            .map_err(|_| Fault::allocation(AT))?;
        coefficients.extend_from_slice(values);
        Ok(Self { coefficients })
    }

    /// Construct the additive identity without a coefficient allocation.
    pub(crate) const fn zero() -> Self {
        Self {
            coefficients: Vec::new(),
        }
    }

    /// Construct the multiplicative identity under the active limits.
    pub(crate) fn one(budget: &mut Budget) -> Result<Self, Fault> {
        Self::new(&[Rational::one()], budget)
    }

    /// Whether the complete coefficient vector is zero.
    pub(crate) const fn is_zero(&self) -> bool {
        self.coefficients.is_empty()
    }

    /// Whether the complete coefficient vector is the multiplicative identity.
    pub(crate) fn is_one(&self) -> bool {
        self.coefficients.len() == 1 && self.coefficients.first() == Some(&Rational::one())
    }

    /// Degree of a nonzero polynomial; zero has no degree.
    pub(crate) const fn degree(&self) -> Option<usize> {
        self.coefficients.len().checked_sub(1)
    }

    /// Borrow a nonzero leading coefficient.
    pub(crate) fn leading(&self) -> Result<&Rational, Fault> {
        self.coefficients.last().ok_or_else(invalid)
    }

    /// Reserve twice the bound of thirty-two live arithmetic vectors, including pending results.
    pub(crate) fn prepay(&self, budget: &mut Budget) -> Result<(), Fault> {
        let bytes = payload(&self.coefficients, budget)?;
        scratch(capacity(self.coefficients.len(), budget)?, budget)?;
        budget.work(bytes, AT)
    }

    /// Exact coefficient addition or subtraction, with a checked maximum output degree.
    pub(crate) fn add(
        &self,
        other: &Self,
        subtract: bool,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        self.prepay(budget)?;
        other.prepay(budget)?;
        let length = self.coefficients.len().max(other.coefficients.len());
        let mut result = zero_coefficients(length, budget)?;
        for (position, value) in result.iter_mut().enumerate() {
            let left = self
                .coefficients
                .get(position)
                .cloned()
                .unwrap_or_else(Rational::zero);
            let right = other
                .coefficients
                .get(position)
                .cloned()
                .unwrap_or_else(Rational::zero);
            *value = if subtract {
                left.sub(&right, budget)?
            } else {
                left.add(&right, budget)?
            };
        }
        finish(result, budget)
    }

    /// Exact scalar multiplication, including multiplication by zero.
    pub(crate) fn scale(&self, coefficient: &Rational, budget: &mut Budget) -> Result<Self, Fault> {
        self.prepay(budget)?;
        coefficient.check(budget, AT)?;
        let mut result = zero_coefficients(self.coefficients.len(), budget)?;
        for (output, value) in result.iter_mut().zip(&self.coefficients) {
            *output = value.mul(coefficient, budget)?;
        }
        finish(result, budget)
    }

    /// Dense exact convolution, with degree refusal before output allocation.
    pub(crate) fn mul(&self, other: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        self.prepay(budget)?;
        other.prepay(budget)?;
        if self.is_zero() || other.is_zero() {
            return Ok(Self::zero());
        }
        let length = self
            .coefficients
            .len()
            .checked_add(other.coefficients.len())
            .and_then(|n| n.checked_sub(1))
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        let mut result = zero_coefficients(length, budget)?;
        for (left_degree, left) in self.coefficients.iter().enumerate() {
            for (right_degree, right) in other.coefficients.iter().enumerate() {
                let position = left_degree
                    .checked_add(right_degree)
                    .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
                let product = left.mul(right, budget)?;
                let value = result
                    .get(position)
                    .ok_or_else(invalid)?
                    .add(&product, budget)?;
                *result.get_mut(position).ok_or_else(invalid)? = value;
            }
        }
        finish(result, budget)
    }

    /// Formal differentiation by the independent coefficient identity `D(a_n` x^n)=n `a_n` x^(n-1).
    pub(crate) fn derivative(&self, budget: &mut Budget) -> Result<Self, Fault> {
        self.prepay(budget)?;
        let mut result = zero_coefficients(self.coefficients.len().saturating_sub(1), budget)?;
        for (position, coefficient) in self.coefficients.iter().enumerate().skip(1) {
            let degree = i64::try_from(position)
                .map_err(|_| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
            let factor = Rational::from_ratio(degree, 1, budget)?;
            let output = position.checked_sub(1).ok_or_else(invalid)?;
            *result.get_mut(output).ok_or_else(invalid)? = coefficient.mul(&factor, budget)?;
        }
        finish(result, budget)
    }

    /// Long division must strictly reduce the remainder degree on every charged iteration.
    pub(crate) fn divide(
        &self,
        divisor: &Self,
        budget: &mut Budget,
    ) -> Result<(Self, Self), Fault> {
        self.prepay(budget)?;
        divisor.prepay(budget)?;
        let degree = divisor.degree().ok_or_else(invalid)?;
        let Some(input_degree) = self.degree() else {
            return Ok((Self::zero(), Self::zero()));
        };
        if input_degree < degree {
            return Ok((Self::zero(), self.clone()));
        }
        let length = input_degree
            .checked_sub(degree)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        let mut quotient = zero_coefficients(length, budget)?;
        let mut remainder = self.clone();
        while let Some(previous) = remainder.degree() {
            if previous < degree {
                break;
            }
            budget.arithmetic(1, AT)?;
            let shift = previous.checked_sub(degree).ok_or_else(invalid)?;
            let coefficient = remainder.leading()?.div(divisor.leading()?, budget)?;
            let value = quotient
                .get(shift)
                .ok_or_else(invalid)?
                .add(&coefficient, budget)?;
            *quotient.get_mut(shift).ok_or_else(invalid)? = value;
            remainder.subtract_shifted(divisor, &coefficient, shift, budget)?;
            if remainder.degree().is_some_and(|next| next >= previous) {
                return Err(invalid());
            }
        }
        Ok((finish(quotient, budget)?, remainder))
    }

    /// Subtract a shifted scaled divisor in place and trim only exact zero coefficients.
    fn subtract_shifted(
        &mut self,
        divisor: &Self,
        coefficient: &Rational,
        shift: usize,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        for (degree, value) in divisor.coefficients.iter().enumerate() {
            let position = shift
                .checked_add(degree)
                .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
            let product = value.mul(coefficient, budget)?;
            let remainder = self
                .coefficients
                .get(position)
                .ok_or_else(invalid)?
                .sub(&product, budget)?;
            *self.coefficients.get_mut(position).ok_or_else(invalid)? = remainder;
        }
        trim(&mut self.coefficients, budget)
    }

    /// Require an exact Euclidean quotient instead of silently discarding a remainder.
    pub(crate) fn exact_divide(&self, divisor: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let (quotient, remainder) = self.divide(divisor, budget)?;
        if remainder.is_zero() {
            Ok(quotient)
        } else {
            Err(invalid())
        }
    }

    /// Bounded nonnegative powers by repeated squaring.
    pub(crate) fn pow(&self, exponent: u64, budget: &mut Budget) -> Result<Self, Fault> {
        budget.check(BudgetKind::PolynomialDegree, exponent, AT)?;
        self.prepay(budget)?;
        let mut factor = self.clone();
        let mut result = Self::one(budget)?;
        let mut remaining = exponent;
        while remaining != 0 {
            budget.arithmetic(1, AT)?;
            if remaining & 1 == 1 {
                result = result.mul(&factor, budget)?;
            }
            remaining >>= 1;
            if remaining != 0 {
                factor = factor.mul(&factor, budget)?;
            }
        }
        Ok(result)
    }
}

/// Validate canonical zero, degree, nonzero-term count and exact component limits.
pub(crate) fn validate(values: &[Rational], budget: &mut Budget) -> Result<(), Fault> {
    let degree = values.len().saturating_sub(1);
    budget.check(BudgetKind::PolynomialDegree, size(degree, AT)?, AT)?;
    if values.last().is_some_and(Rational::is_zero) {
        return Err(invalid());
    }
    budget.work(size(values.len(), AT)?, AT)?;
    let mut nonzero = 0_u64;
    for value in values {
        value.check(budget, AT)?;
        nonzero = nonzero
            .checked_add(u64::from(!value.is_zero()))
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialTerms, AT))?;
        budget.check(BudgetKind::PolynomialTerms, nonzero, AT)?;
    }
    Ok(())
}

/// Include coefficient records and measured bigint components, not just encoded magnitude bytes.
pub(crate) fn payload(values: &[Rational], budget: &mut Budget) -> Result<u64, Fault> {
    budget.work(size(values.len(), AT)?, AT)?;
    let mut total = 64_u64;
    for value in values {
        let (numerator, denominator) = value.component_bits();
        let bytes = numerator
            .checked_add(denominator)
            .and_then(|n| n.checked_add(7))
            .and_then(|n| n.checked_div(8))
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        total = total
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    }
    Ok(total)
}

/// Allocate a bounded dense arithmetic result only after checking its maximum degree.
pub(crate) fn zero_coefficients(
    length: usize,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    budget.check(
        BudgetKind::PolynomialDegree,
        size(length.saturating_sub(1), AT)?,
        AT,
    )?;
    let bytes = capacity(length, budget)?;
    scratch(bytes, budget)?;
    budget.work(bytes, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| Fault::allocation(AT))?;
    result.resize(length, Rational::zero());
    Ok(result)
}

/// Strip zero tails, charging each mutation.
fn trim(values: &mut Vec<Rational>, budget: &mut Budget) -> Result<(), Fault> {
    while values.last().is_some_and(Rational::is_zero) {
        budget.work(1, AT)?;
        let _removed = values.pop();
    }
    Ok(())
}

/// Canonicalize only newly computed arithmetic, never untrusted certificate input.
fn finish(mut coefficients: Vec<Rational>, budget: &mut Budget) -> Result<Polynomial, Fault> {
    trim(&mut coefficients, budget)?;
    validate(&coefficients, budget)?;
    Ok(Polynomial { coefficients })
}

/// Invalid polynomial evidence is not a theorem.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Leave half of the allowance for retained certificate records and non-vector arithmetic data.
fn scratch(bytes: u64, budget: &Budget) -> Result<(), Fault> {
    let peak = bytes
        .checked_mul(64)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(BudgetKind::StagedPayloadBytes, peak, AT)
}

/// Maximum retained coefficient payload at the active exact component ceiling.
fn capacity(length: usize, budget: &Budget) -> Result<u64, Fault> {
    // Every coefficient written later is checked against this component ceiling. Reserving
    // the full vector at that ceiling precedes allocation and all in-place arithmetic writes.
    let component = budget
        .limits()
        .component_bits
        .checked_add(7)
        .and_then(|n| n.checked_div(8))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    let record = component
        .checked_mul(2)
        .and_then(|n| n.checked_add(128))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    size(length, AT)?
        .checked_mul(record)
        .and_then(|n| n.checked_add(64))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))
}
