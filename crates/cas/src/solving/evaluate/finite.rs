// SPDX-License-Identifier: AGPL-3.0-only
//! Recompute every term of the finite ordered polynomial support.

use super::{AT, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Handle;
use crate::solving::coefficients::forms::Form;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::vector;
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::elementary::value::Value;
use crate::solving::{CertificateCoefficientEvaluation, CoefficientEvaluationData};

/// Read every image only after its explicit definedness and root evidence have matched.
pub(super) fn images(
    evaluation: &Evaluation<'_>,
    mapping: &CoefficientEvaluationData<Handle>,
    budget: &mut Budget,
) -> Result<Vec<Value>, Fault> {
    let mut result = vector(mapping.bindings.len(), budget)?;
    for binding in &mapping.bindings {
        result.push(super::syntax::read(evaluation, &binding.image, budget)?);
    }
    Ok(result)
}

/// Specialize finite polynomial variables while preserving the exact coefficient algebra.
pub(super) fn polynomial(
    evaluation: &Evaluation<'_>,
    handle: &Handle,
    images: &[Value],
    budget: &mut Budget,
) -> Result<Value, Fault> {
    let stored = crate::solving::coefficients::storage::polynomial(&evaluation.cas.state, handle)?;
    if stored.domain != *evaluation.engine.domain || stored.variables.len() != images.len() {
        return Err(invalid());
    }
    let mut result = Value::integer(0, &evaluation.engine, budget)?;
    for term in &stored.terms {
        budget.work(1, AT)?;
        let coefficient = evaluation
            .engine
            .read(&term.coefficient, budget)
            .map_err(|error| fault(&error))?;
        let form = Form::constant(coefficient, &evaluation.engine, budget)
            .map_err(|error| fault(&error))?;
        let mut product = Value::polynomial(form, &evaluation.engine, budget)?;
        if term.exponents.len() != images.len() {
            return Err(invalid());
        }
        for (image, exponent) in images.iter().zip(&term.exponents) {
            budget.work(1, AT)?;
            product = product.mul(
                &image.pow(*exponent, &evaluation.engine, budget)?,
                &evaluation.engine,
                budget,
            )?;
        }
        result = result.add(&product, &evaluation.engine, budget)?;
    }
    Ok(result)
}

/// A localization is nonvanishing after specialization, with the actual target-region unit fact.
pub(super) fn localization(
    evaluation: &Evaluation<'_>,
    mapping: &CoefficientEvaluationData<Handle>,
    certificate: &CertificateCoefficientEvaluation,
    images: &[Value],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if certificate.units.len() != mapping.localization.len() {
        return Err(invalid());
    }
    for (source, slot) in mapping.localization.iter().zip(&certificate.units) {
        let unit = crate::solving::elementary::proof::premise(evaluation.premises, *slot)?;
        let value = polynomial(evaluation, source, images, budget)?;
        let crate::proof_model::Statement::PredicateFact { region, predicate } =
            &unit.judgment.statement
        else {
            return Err(invalid());
        };
        let (kind, arguments) = crate::solving::elementary::evidence::predicate(
            evaluation.cas,
            predicate.logical_id(),
        )?;
        let [argument] = arguments else {
            return Err(invalid());
        };
        if region != evaluation.region
            || !matches!(
                kind,
                crate::model::PredicateKind::NonZero | crate::model::PredicateKind::Positive
            )
        {
            return Err(invalid());
        }
        let actual = super::syntax::read(evaluation, &evaluation.cas.handle(*argument), budget)?;
        if !value.equals(&actual, &evaluation.engine, budget)? {
            return Err(invalid());
        }
    }
    Ok(())
}
