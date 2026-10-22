// SPDX-License-Identifier: AGPL-3.0-only
//! Total coefficient guards use declarations that retain their meaning in every child region.

use crate::budget::Budget;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::solving::SolveReason;
use crate::solving::coefficients::model::{CoefficientFailure, Element, Value};
use crate::solving::coefficients::polynomial::Term;
use crate::solving::coefficients::scalar::{Scalar, rational_copy};
use crate::solving::coefficients::storage::invalid;
use crate::solving::radicals::construction::Construction;

/// The source polynomial grammar admits rational scalars, including their Gaussian embedding.
fn scalar(
    value: &Scalar,
    budget: &mut Budget,
) -> Result<crate::number::Rational, CoefficientFailure> {
    match value {
        Scalar::Rational(value) => Ok(rational_copy(value, budget)?),
        Scalar::Gaussian(value) => {
            if !value.imaginary(budget)?.is_zero() {
                return Err(CoefficientFailure::Unsupported(
                    SolveReason::PolynomialFragment,
                ));
            }
            Ok(value.real(budget)?)
        }
    }
}

/// Expand one canonical coefficient monomial through the ordered declaration images.
fn term(
    cas: &mut Cas,
    construction: &Construction,
    monomial: &Term,
    images: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, CoefficientFailure> {
    if monomial.powers.len() != images.len() {
        return Err(invalid().into());
    }
    let value = scalar(&monomial.coefficient, budget)?;
    let mut result = construction.expression(cas, Operator::Rational(value), &[], false, budget)?;
    for (exponent, image) in monomial.powers.iter().zip(images) {
        budget.work(1, super::AT)?;
        if *exponent == 0 {
            continue;
        }
        let factor = if *exponent == 1 {
            image.clone()
        } else {
            construction.expression(
                cas,
                Operator::IntegerPower(i64::from(*exponent)),
                std::slice::from_ref(image),
                false,
                budget,
            )?
        };
        result = construction.expression(cas, Operator::Mul, &[result, factor], false, budget)?;
    }
    Ok(result)
}

/// A pending obligation is total: check its unit denominator before rendering ordinary scalar syntax.
pub(crate) fn expression(
    cas: &mut Cas,
    construction: &Construction,
    pending: &Element,
    budget: &mut Budget,
) -> Result<Handle, CoefficientFailure> {
    let Value::Fraction(value) = &pending.value else {
        return Err(invalid().into());
    };
    let one = value.denominator.one(budget)?;
    if !value.denominator.equals(&one, budget)? {
        return Err(invalid().into());
    }
    if cas
        .state
        .region(construction.region.logical_id())?
        .lies_over
        .is_some()
    {
        let images = super::coefficient_images(cas, construction, &pending.domain, budget)?;
        let map = construction.mapping(cas, &pending.domain, &images, budget)?;
        return Ok(construction.literal(cas, pending, &map, budget)?);
    }
    let images = super::coefficient_images(cas, construction, &pending.domain, budget)?;
    if value.numerator.variables != images.len() {
        return Err(invalid().into());
    }
    let mut result = construction.expression(
        cas,
        Operator::Rational(crate::number::Rational::zero()),
        &[],
        false,
        budget,
    )?;
    for monomial in &value.numerator.terms {
        budget.work(1, super::AT)?;
        let summand = term(cas, construction, monomial, &images, budget)?;
        result = construction.expression(cas, Operator::Add, &[result, summand], false, budget)?;
    }
    Ok(result)
}
