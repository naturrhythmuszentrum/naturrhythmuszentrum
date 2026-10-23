// SPDX-License-Identifier: AGPL-3.0-only
//! Compatible standard conjugation maps and fixed-real-subfield conclusions.

use super::evaluate::Evaluation;
use super::proof::invalid;
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
    components: &mut super::complex::Components<'_, '_>,
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
    if map.region != certificate.region {
        return Err(invalid());
    }
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
        components.equal(
            original.logical_id(),
            transformed.logical_id(),
            true,
            budget,
        )?;
    }
    Ok(())
}

/// A supported but mismatched claim is a rule failure before coefficient-fragment dispatch.
fn claim<'a>(
    draft: &'a ApplicationDraft,
    certificate: &CertificateConjugationTransport,
) -> Result<&'a SolveStatement, Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let valid = match statement.as_ref() {
        SolveStatement::RealValues(value) => {
            (&value.values, &value.region) == (&certificate.values, &certificate.region)
        }
        SolveStatement::CoefficientEquality(value) => {
            (&value.map, &value.region) == (&certificate.map, &certificate.region)
        }
        _ => false,
    };
    if valid {
        Ok(statement.as_ref())
    } else {
        Err(invalid())
    }
}

/// Realness is exactly vanishing imaginary part under the standard conjugation theorem.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateConjugationTransport,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let statement = claim(draft, certificate)?;
    let map = crate::solving::coefficients::proof::mapping(
        cas,
        &certificate.map,
        &draft.context,
        &certificate.region,
        budget,
    )?;
    let evaluation = Evaluation {
        exclusions: &[],
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
    let mut components = super::complex::Components::new(&evaluation, budget)?;
    components.premises(&certificate.relations, budget)?;
    components.premises(&certificate.fixed_field, budget)?;
    maps(&evaluation, certificate, &mut components, budget)?;
    match statement {
        SolveStatement::RealValues(value) => {
            for handle in &value.values {
                components.real(handle.logical_id(), budget)?;
            }
            Ok(())
        }
        SolveStatement::CoefficientEquality(value) => {
            if super::product::decides(components.equal(
                value.left.logical_id(),
                value.right.logical_id(),
                false,
                budget,
            ))? {
                Ok(())
            } else {
                components.cancel(draft, certificate, value, budget)
            }
        }
        _ => Err(invalid()),
    }
}
