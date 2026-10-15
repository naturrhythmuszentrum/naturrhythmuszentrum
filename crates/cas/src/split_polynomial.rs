// SPDX-License-Identifier: AGPL-3.0-only
//! Producer recurrences and independent polynomial endpoint identities.

use crate::budget::Budget;
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::number::Rational;
use crate::split_model::SplitKind;
use crate::univariate::{Polynomial, zero_coefficients};

/// Horner evaluation used by producers.
pub(crate) fn evaluate(
    values: &[Rational],
    point: &Rational,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    let mut result = Rational::zero();
    for value in values.iter().rev() {
        result = result.mul(point, budget)?.add(value, budget)?;
    }
    Ok(result)
}

/// The checker uses explicit powers, independently of the producer's Horner recurrence.
pub(crate) fn endpoint(
    values: &[Rational],
    point: &Rational,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    let mut result = Rational::zero();
    for (degree, value) in values.iter().enumerate() {
        let power = if degree == 0 {
            Rational::one()
        } else {
            point.pow(i64::try_from(degree).map_err(|_| invalid())?, budget)?
        };
        result = result.add(&value.mul(&power, budget)?, budget)?;
    }
    Ok(result)
}

/// Integrate coefficients, or solve the triangular forward-difference system.
pub(crate) fn primitive(
    values: &[Rational],
    kind: SplitKind,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = zero_coefficients(values.len().checked_add(1).ok_or_else(invalid)?, budget)?;
    let mut residual = values.to_vec();
    for degree in (0..values.len()).rev() {
        let power = degree.checked_add(1).ok_or_else(invalid)?;
        let factor = Rational::from_ratio(i64::try_from(power).map_err(|_| invalid())?, 1, budget)?;
        let value = residual
            .get(degree)
            .ok_or_else(invalid)?
            .div(&factor, budget)?;
        result
            .get_mut(power)
            .ok_or_else(invalid)?
            .clone_from(&value);
        if kind == SplitKind::Sum {
            subtract_difference(&mut residual, power, &value, budget)?;
        }
    }
    crate::univariate::validate(&result, budget)?;
    Ok(result)
}

/// Binomial expansion removes the already solved highest forward-difference coefficient.
fn subtract_difference(
    values: &mut [Rational],
    power: usize,
    factor: &Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut binomial = Rational::one();
    for degree in 0..power {
        let value = values.get_mut(degree).ok_or_else(invalid)?;
        *value = value.sub(&factor.mul(&binomial, budget)?, budget)?;
        let numerator = Rational::from_ratio(
            i64::try_from(power - degree).map_err(|_| invalid())?,
            1,
            budget,
        )?;
        let denominator =
            Rational::from_ratio(i64::try_from(degree + 1).map_err(|_| invalid())?, 1, budget)?;
        binomial = binomial
            .mul(&numerator, budget)?
            .div(&denominator, budget)?;
    }
    Ok(())
}

/// Check the supplied primitive by differentiation or polynomial composition, without solving it.
pub(crate) fn check(
    values: &[Rational],
    proposed: &[Rational],
    kind: SplitKind,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let polynomial = Polynomial::new(proposed, budget)?;
    let actual = if kind == SplitKind::Integral {
        polynomial.derivative(budget)?
    } else {
        let affine = Polynomial::new(&[Rational::one(), Rational::one()], budget)?;
        let mut shifted = Polynomial::zero();
        for value in proposed.iter().rev() {
            shifted = shifted.mul(&affine, budget)?;
            if !value.is_zero() {
                shifted = shifted.add(
                    &Polynomial::new(std::slice::from_ref(value), budget)?,
                    false,
                    budget,
                )?;
            }
        }
        shifted.add(&polynomial, true, budget)?
    };
    if actual.coefficients != values {
        return Err(invalid());
    }
    Ok(())
}
