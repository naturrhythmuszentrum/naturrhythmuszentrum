// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit descent of rational invariant coefficients and their actual Vieta images.

use super::construction::Construction;
use crate::budget::Budget;
use crate::graph::{Cas, Handle};
use crate::number::Rational;
use crate::solving::SolveReason;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::{Form, Term};
use crate::solving::coefficients::model::{CoefficientFailure, Element, EvaluationScope, Value};
use crate::solving::coefficients::scalar::{Scalar, rational_copy};
use crate::solving::coefficients::storage::{copied, invalid, vector};
use crate::solving::symmetric::{algebra::Polynomial, reduction::Reduction};
use crate::solving::towers::dense::Dense;

/// Reading a ground constant never substitutes a parameter or a chosen root.
fn ground(
    source: &crate::solving::coefficients::polynomial::Polynomial,
    budget: &mut Budget,
) -> Result<Rational, CoefficientFailure> {
    budget.work(
        crate::graph::size(source.variables, super::AT)?.max(1),
        super::AT,
    )?;
    match source.terms.as_slice() {
        [] => Ok(Rational::zero()),
        [term] if term.powers.iter().all(|power| *power == 0) => match &term.coefficient {
            Scalar::Rational(value) => Ok(rational_copy(value, budget)?),
            Scalar::Gaussian(_) => Err(CoefficientFailure::Unsupported(
                SolveReason::GenericRadicalRouteUnavailable,
            )),
        },
        _ => Err(CoefficientFailure::Unsupported(
            SolveReason::GenericRadicalRouteUnavailable,
        )),
    }
}

/// A reduced tower coefficient descends only when each generator has degree zero.
pub(crate) fn rational(
    source: &Element,
    budget: &mut Budget,
) -> Result<Rational, CoefficientFailure> {
    let mut value = source;
    loop {
        budget.work(1, super::AT)?;
        match &value.value {
            Value::Scalar(Scalar::Rational(result)) => return Ok(rational_copy(result, budget)?),
            Value::Fraction(result) => {
                return Ok(ground(&result.numerator, budget)?
                    .div(&ground(&result.denominator, budget)?, budget)?);
            }
            Value::Tower(coefficients) => match coefficients.as_slice() {
                [] => return Ok(Rational::zero()),
                [constant] => value = constant,
                _ => {
                    return Err(CoefficientFailure::Unsupported(
                        SolveReason::GenericRadicalRouteUnavailable,
                    ));
                }
            },
            Value::Scalar(Scalar::Gaussian(_)) => {
                return Err(CoefficientFailure::Unsupported(
                    SolveReason::GenericRadicalRouteUnavailable,
                ));
            }
        }
    }
}

/// Every changed coefficient has an ordinary nonreflexive scalar identity in the actual algebra.
fn polynomial(
    cas: &mut Cas,
    construction: &mut Construction,
    source: &Polynomial,
    base: &Handle,
    map: &Handle,
    budget: &mut Budget,
) -> Result<Polynomial, CoefficientFailure> {
    let mut terms = vector(source.form.terms.len(), budget)?;
    for term in &source.form.terms {
        let scalar = rational(&term.coefficient, budget)?;
        let left = construction.literal(cas, &term.coefficient, map, budget)?;
        let right = construction.expression(
            cas,
            crate::model::Operator::Rational(rational_copy(&scalar, budget)?),
            &[],
            false,
            budget,
        )?;
        super::proof::identity(construction, [&left, &right], map, budget)?;
        let engine = Engine {
            state: &cas.state,
            domain: base,
        };
        terms.push(Term {
            powers: copied(&term.powers, budget)?,
            coefficient: engine.rational(&scalar, budget)?,
        });
    }
    Ok(Polynomial {
        domain: base.clone(),
        variables: copied(&source.variables, budget)?,
        form: Form { terms },
    })
}

/// No normal form changes its coefficient algebra without an explicit replay obligation.
pub(crate) fn reduction(
    cas: &mut Cas,
    construction: &mut Construction,
    source: &Reduction,
    base: &Handle,
    map: &Handle,
    budget: &mut Budget,
) -> Result<Reduction, CoefficientFailure> {
    let original = polynomial(cas, construction, &source.source, base, map, budget)?;
    let expression = polynomial(cas, construction, &source.expression, base, map, budget)?;
    let mut basis = vector(source.basis.len(), budget)?;
    for element in &source.basis {
        basis.push(polynomial(cas, construction, element, base, map, budget)?);
    }
    Ok(Reduction {
        source: original,
        expression,
        basis,
    })
}

/// Actual signed leading ratios keep unused formal slots for the ideal-combination proof.
pub(crate) fn images(
    source: &Dense,
    frame: &Polynomial,
    scope: &EvaluationScope<'_>,
    cas: &Cas,
    budget: &mut Budget,
) -> Result<Vec<Polynomial>, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &source.domain,
    };
    let degree = source
        .coefficients
        .len()
        .checked_sub(1)
        .ok_or_else(invalid)?;
    if source.domain != frame.domain || degree != frame.variables.len() {
        return Err(invalid().into());
    }
    let reciprocal = super::roots::inverse(
        &engine,
        source.coefficients.last().ok_or_else(invalid)?,
        scope,
        budget,
    )?;
    let mut result = vector(degree, budget)?;
    for index in 1..=degree {
        let value = engine.mul(
            source
                .coefficients
                .get(degree - index)
                .ok_or_else(invalid)?,
            &reciprocal,
            budget,
        )?;
        let signed = if index.is_multiple_of(2) {
            value
        } else {
            engine.neg(&value, budget)?
        };
        result.push(Polynomial::constant(
            &source.domain,
            &frame.variables,
            signed,
            &cas.state,
            budget,
        )?);
    }
    Ok(result)
}

/// The original leading coefficient, rather than a substituted unit, anchors positive degree.
pub(crate) fn leading(
    cas: &mut Cas,
    construction: &mut Construction,
    source: &Dense,
    inverse: &Element,
    budget: &mut Budget,
) -> Result<u32, CoefficientFailure> {
    use crate::model::Field;
    use crate::solving::towers::materialize;
    use crate::solving::{
        CertificateExtensionUnit, CoefficientDomainData, SolveCertificate, SolveSchema,
        SolveStatement, StatementUnit,
    };
    let engine = Engine {
        state: &cas.state,
        domain: &source.domain,
    };
    let original = source.coefficients.last().ok_or_else(invalid)?;
    let one = engine.constant(true, budget)?;
    if engine.formal_equal(original, &one, budget)? {
        return Ok(construction.unit(cas, &source.domain, budget)?);
    }
    if matches!(
        crate::solving::coefficients::storage::domain(&cas.state, &source.domain)?,
        CoefficientDomainData::Quotient { .. }
    ) {
        return Err(CoefficientFailure::Unsupported(
            SolveReason::MissingUnitProof,
        ));
    }
    let element = Dense::new(&engine, vec![engine.copy(original, budget)?], budget)?;
    let reciprocal = Dense::new(&engine, vec![engine.copy(inverse, budget)?], budget)?;
    let constant = Dense::one(&engine, budget)?;
    let zero = Dense::zero(&engine, budget)?;
    let variable = constant.scaled(&one, 1, &cas.state, budget)?;
    let value = materialize::value(cas, original, budget)?;
    let inverse = materialize::value(cas, inverse, budget)?;
    let element = materialize::polynomial(cas, &element, &[Field::Complex], budget)?;
    let reciprocal = materialize::polynomial(cas, &reciprocal, &[Field::Complex], budget)?;
    let constant = materialize::polynomial(cas, &constant, &[Field::Complex], budget)?;
    let zero = materialize::polynomial(cas, &zero, &[Field::Complex], budget)?;
    let modulus = materialize::polynomial(cas, &variable, &[Field::Complex], budget)?;
    Ok(construction.push(
        SolveSchema::ExtensionUnit,
        SolveStatement::Unit(StatementUnit {
            value: value.clone(),
            inverse,
            domain: source.domain.clone(),
            region: construction.region.clone(),
        }),
        SolveCertificate::ExtensionUnit(CertificateExtensionUnit {
            element: value,
            modulus: modulus.clone(),
            gcd: constant,
            quotients: vec![element, modulus],
            bezout: vec![reciprocal, zero],
            region: construction.region.clone(),
        }),
        budget,
    )?)
}
