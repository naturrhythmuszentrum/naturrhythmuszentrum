// SPDX-License-Identifier: AGPL-3.0-only
//! Total coefficient guards use declarations that retain their meaning in every child region.

use super::construction::Frame;
use crate::budget::Budget;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::solving::SolveReason;
use crate::solving::coefficients::model::{CoefficientFailure, Element, Value};
use crate::solving::coefficients::polynomial::Term;
use crate::solving::coefficients::scalar::{Scalar, rational_copy};
use crate::solving::coefficients::storage::invalid;

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
    construction: &Frame,
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
    construction: &Frame,
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
    let images = images(cas, construction, &pending.domain, budget)?;
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

/// Original Real declaration keys are the canonical coefficient map images.
pub(crate) fn images(
    cas: &mut Cas,
    construction: &Frame,
    domain: &Handle,
    budget: &mut Budget,
) -> Result<Vec<Handle>, CoefficientFailure> {
    let mut keys = crate::solving::coefficients::storage::vector(0, budget)?;
    if let crate::solving::CoefficientDomainData::Fraction { coordinates, .. } =
        crate::solving::coefficients::storage::domain(&cas.state, domain)?
    {
        let frame = crate::solving::elementary::proof::coordinates(cas, coordinates)?;
        crate::solving::coefficients::storage::reserve(&mut keys, frame.coordinates.len(), budget)?;
        for coordinate in &frame.coordinates {
            if coordinate.field != crate::model::Field::Real {
                return Err(invalid().into());
            }
            budget.work(
                crate::graph::size(coordinate.key.len(), super::AT)?,
                super::AT,
            )?;
            keys.push(coordinate.key.clone());
        }
    }
    let mut images = crate::solving::coefficients::storage::vector(keys.len(), budget)?;
    for key in keys {
        images.push(construction.expression(cas, Operator::Free(key), &[], false, budget)?);
    }
    Ok(images)
}

/// Clearing a coefficient denominator for a branch preserves the actual numerator and its algebra.
pub(crate) fn numerator(
    value: &Element,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    let Value::Fraction(fraction) = &value.value else {
        let Value::Scalar(scalar) = &value.value else {
            return Err(invalid().into());
        };
        return Ok(Element {
            domain: value.domain.clone(),
            origin: None,
            value: Value::Scalar(scalar.copy(budget)?),
        });
    };
    Ok(Element {
        domain: value.domain.clone(),
        origin: None,
        value: Value::Fraction(crate::solving::coefficients::fraction::Fraction {
            numerator: fraction.numerator.copy(budget)?,
            denominator: fraction.denominator.one(budget)?,
        }),
    })
}

/// Extract a manifest ground value without conflating an undecided parameter fraction with a constant.
pub(crate) fn rational(
    value: &Element,
    budget: &mut Budget,
) -> Result<crate::number::Rational, CoefficientFailure> {
    match &value.value {
        Value::Scalar(value) => scalar(value, budget),
        Value::Fraction(value) => {
            let numerator =
                value
                    .numerator
                    .scalar(budget)?
                    .ok_or(CoefficientFailure::Unsupported(
                        SolveReason::PolynomialFragment,
                    ))?;
            let denominator =
                value
                    .denominator
                    .scalar(budget)?
                    .ok_or(CoefficientFailure::Unsupported(
                        SolveReason::PolynomialFragment,
                    ))?;
            Ok(scalar(&numerator, budget)?.div(&scalar(&denominator, budget)?, budget)?)
        }
        Value::Tower(_) => Err(CoefficientFailure::Unsupported(
            SolveReason::PolynomialFragment,
        )),
    }
}
