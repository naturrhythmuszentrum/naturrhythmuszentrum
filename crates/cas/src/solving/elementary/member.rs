// SPDX-License-Identifier: AGPL-3.0-only
//! Membership in the original predicate and its strict original domain.

use super::evaluate::Evaluation;
use super::proof::{AT, coordinates, invalid, problem, tuple};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::storage::vector;
use crate::solving::{CertificateMembership, SolveProblemData, SolveStatement};

/// Structural restriction preserves the exact product and every original guard.
pub(crate) fn contained(
    cas: &Cas,
    child: &Handle,
    parent: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let target = cas.state.region(cas.checked(parent)?)?;
    let mut pending = vector(1, budget)?;
    pending.push(cas.checked(child)?);
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let current = cas.state.region(id)?;
        if (&current.ambient, &current.scope, current.header)
            != (&target.ambient, &target.scope, target.header)
        {
            return Err(invalid());
        }
        if id == parent.logical_id()
            || matches!(target.expression, crate::domain_model::RegionExpr::Ambient)
        {
            return Ok(());
        }
        match &current.expression {
            crate::domain_model::RegionExpr::GuardTrue { base, .. }
            | crate::domain_model::RegionExpr::GuardFalse { base, .. } => pending.push(*base),
            crate::domain_model::RegionExpr::Meet(members) => {
                crate::solving::coefficients::storage::reserve(
                    &mut pending,
                    members.len(),
                    budget,
                )?;
                pending.extend(members);
            }
            _ => {}
        }
    }
    Err(invalid())
}

/// Retain the original predicate, unknown domain and every original denominator independently.
pub(crate) fn original(
    evaluation: &Evaluation<'_>,
    original: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    evaluation.predicate(original.predicate.logical_id(), budget)?;
    evaluation.domain(original.domain.logical_id(), budget)?;
    for denominator in &original.denominators {
        let value = evaluation.expression(denominator.logical_id(), true, budget)?;
        evaluation.nonzero(&value, budget)?;
    }
    Ok(())
}

/// Substitution rows must repeat the original unknown order and precisely the tuple's entries.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateMembership,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Member(member) = statement.as_ref() else {
        return Err(invalid());
    };
    if (&member.problem, &member.tuple, &member.region)
        != (
            &certificate.problem,
            &certificate.tuple,
            &certificate.region,
        )
    {
        return Err(invalid());
    }
    let original = problem(cas, &certificate.problem)?;
    super::proof::region(cas, &draft.context, original, &certificate.region)?;
    contained(cas, &certificate.region, &original.region, budget)?;
    let supplied = tuple(cas, &certificate.tuple)?;
    let unknowns = coordinates(cas, &original.unknowns)?;
    if (
        &supplied.coordinates,
        &supplied.scope,
        certificate.substitutions.len(),
    ) != (
        &original.unknowns,
        &original.scope,
        unknowns.coordinates.len(),
    ) {
        return Err(invalid());
    }
    let evaluation = Evaluation {
        exclusions: &[],
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &original.coefficient_domain,
        },
        region: &certificate.region,
        premises,
        free: &[],
        bound: &[],
    };
    let mut replacements = vector(supplied.entries.len(), budget)?;
    for ((coordinate, entry), substitution) in unknowns
        .coordinates
        .iter()
        .zip(&supplied.entries)
        .zip(&certificate.substitutions)
    {
        budget.work(1, AT)?;
        if (&coordinate.key, entry) != (&substitution.key, &substitution.expression) {
            return Err(invalid());
        }
        replacements.push((
            coordinate.key.as_str(),
            evaluation.expression(entry.logical_id(), true, budget)?,
        ));
    }
    for slot in &certificate.definedness {
        super::proof::premise(premises, *slot)?;
    }
    let substituted = Evaluation {
        free: &replacements,
        ..evaluation
    };
    self::original(&substituted, original, budget)
}
