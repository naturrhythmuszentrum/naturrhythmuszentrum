// SPDX-License-Identifier: AGPL-3.0-only
//! Compatible standard conjugation maps and fixed-real-subfield conclusions.

use super::evaluate::Evaluation;
use super::proof::{invalid, premise};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Cas;
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::{CertificateConjugationTransport, SolveDraft, SolveStatement};

/// Each supplied generator pair must be the actual involution image, never an independent root choice.
fn maps(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateConjugationTransport,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let SolveDraft::CoefficientMap(map) =
        crate::solving::graph::object(&evaluation.cas.state, certificate.map.logical_id())?
    else {
        return Err(invalid());
    };
    let SolveDraft::CoefficientMap(conjugate) = crate::solving::graph::object(
        &evaluation.cas.state,
        certificate.conjugate_map.logical_id(),
    )?
    else {
        return Err(invalid());
    };
    if (&map.source, &map.target, &map.region, map.images.len())
        != (
            &conjugate.source,
            &conjugate.target,
            &conjugate.region,
            certificate.images.len(),
        )
        || map.images.len() != conjugate.images.len()
    {
        return Err(invalid());
    }
    crate::solving::coefficients::proof::identity_map(
        evaluation.cas,
        map,
        evaluation.premises,
        budget,
    )?;
    crate::solving::coefficients::proof::identity_map(
        evaluation.cas,
        conjugate,
        evaluation.premises,
        budget,
    )?;
    for ((original, transformed), pair) in map
        .images
        .iter()
        .zip(&conjugate.images)
        .zip(&certificate.images)
    {
        if (original, transformed) != (&pair.original, &pair.conjugate) {
            return Err(invalid());
        }
        let value = super::complex::expression(evaluation, original.logical_id(), budget)?;
        let expected = super::complex::expression(evaluation, transformed.logical_id(), budget)?;
        value
            .conjugate(evaluation, budget)?
            .equal(&expected, evaluation, budget)?;
    }
    Ok(())
}

/// Realness is exactly vanishing imaginary part under the standard conjugation theorem.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateConjugationTransport,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let map = crate::solving::coefficients::proof::mapping(
        cas,
        &certificate.map,
        &draft.context,
        &certificate.region,
        budget,
    )?;
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &map.source,
        },
        region: &certificate.region,
        premises,
        free: &[],
        bound: &[],
    };
    for slot in certificate.relations.iter().chain(&certificate.fixed_field) {
        premise(premises, *slot)?;
    }
    maps(&evaluation, certificate, budget)?;
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    match statement.as_ref() {
        SolveStatement::RealValues(value)
            if (&value.values, &value.region) == (&certificate.values, &certificate.region) =>
        {
            for handle in &value.values {
                evaluation.zero(
                    &super::complex::expression(&evaluation, handle.logical_id(), budget)?
                        .imaginary,
                    budget,
                )?;
            }
            Ok(())
        }
        SolveStatement::CoefficientEquality(value)
            if (&value.map, &value.region) == (&certificate.map, &certificate.region) =>
        {
            let left = super::complex::expression(&evaluation, value.left.logical_id(), budget)?;
            let right = super::complex::expression(&evaluation, value.right.logical_id(), budget)?;
            left.equal(&right, &evaluation, budget)
        }
        _ => Err(invalid()),
    }
}
