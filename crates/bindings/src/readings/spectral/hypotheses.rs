// SPDX-License-Identifier: AGPL-3.0-only
//! Authenticate transport hypotheses against the actual ordered positive input predicates.

use super::{AT, PositiveReciprocalMap, SpectralLawError as Error};
use nrz_cas::{
    Cas, FaultKind, Handle, InvalidCode,
    domain::HypothesisRef,
    limits::Budget,
    proof::{ApplicationDraft, Certificate, Primitive, Statement},
    source::{SyntaxDescriptor, SyntaxRole},
};

/// The proof may traverse C before L; reporting follows exact L/C subjects instead of traversal order.
pub(super) fn ordered(
    cas: &mut Cas,
    operator: &PositiveReciprocalMap,
    budget: &mut Budget,
) -> Result<Vec<HypothesisRef>, Error> {
    let used = operator.witness().hypotheses();
    if used.is_empty() {
        return Ok(Vec::new());
    }
    if used.len() != 2 {
        return Err(Error::Conditional);
    }
    let context = operator.frame().context();
    let region = cas.admissible_region(context)?;
    let last = cas.describe_syntax(&region, budget)?;
    let first = cas.describe_syntax(edge(&last, SyntaxRole::Base)?, budget)?;
    let predicates = [
        edge(&first, SyntaxRole::Predicate)?,
        edge(&last, SyntaxRole::Predicate)?,
    ];
    let mut result = super::super::support::reserve(2, budget)?;
    for predicate in predicates {
        let mut found = None;
        for candidate in used {
            budget.work(1, AT)?;
            if result.contains(candidate) {
                continue;
            }
            if authenticates(cas, context, &region, predicate, candidate, budget)? {
                found = Some(candidate.clone());
                break;
            }
        }
        result.push(found.ok_or(Error::Conditional)?);
    }
    Ok(result)
}

/// Context-hypothesis checking authenticates the actual predicate; names and slot numbers do not.
fn authenticates(
    cas: &mut Cas,
    context: &Handle,
    region: &Handle,
    predicate: &Handle,
    reference: &HypothesisRef,
    budget: &mut Budget,
) -> Result<bool, Error> {
    let step = ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: context.clone(),
        conclusion: Statement::PredicateFact {
            region: region.clone(),
            predicate: predicate.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(reference.clone()),
    };
    match cas.check_proof(context, &[step], budget) {
        Ok(_) => Ok(true),
        Err(error) if error.kind == FaultKind::Invalid(InvalidCode::Context) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Both guard predicates belong to the previously checked whole positive target domain.
fn edge(syntax: &SyntaxDescriptor, role: SyntaxRole) -> Result<&Handle, Error> {
    syntax
        .dependencies
        .iter()
        .find(|edge| edge.role == role)
        .map(|edge| &edge.target)
        .ok_or(Error::Domain)
}
