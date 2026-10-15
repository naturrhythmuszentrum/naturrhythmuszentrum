// SPDX-License-Identifier: AGPL-3.0-only
//! Independently replayed membership, strict coordinate transport and total degree partitions.

mod constraints;
mod evaluate;
mod evidence;
mod member;
mod proof;
mod reduction;
mod split;
mod total;
mod transport;
mod value;

/// Elementary availability follows the implemented independently checked schemas.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Enable a schema only together with its concrete checker.
pub(crate) const fn schema_availability(tag: u8) -> super::Availability {
    if matches!(tag, 4..=6) {
        super::Availability::Available
    } else {
        super::Availability::Unavailable
    }
}

/// Dispatch exact certificate subtypes and preserve the complete scoped premise conditions.
pub(crate) fn check(
    cas: &crate::graph::Cas,
    draft: &crate::proof_model::ApplicationDraft,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut crate::budget::Budget,
) -> Result<crate::proof_model::ScopedConditions, crate::error::Fault> {
    use crate::solving::{SolveCertificate, SolveSchema};
    let crate::proof_model::Certificate::Solve(certificate) = &draft.certificate else {
        return Err(proof::invalid());
    };
    let inherited = proof::conditions(cas, draft, premises, budget)?;
    match (draft.rule, certificate.as_ref()) {
        (
            crate::rule::Primitive::Solve(SolveSchema::Membership),
            SolveCertificate::Membership(value),
        ) => member::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::SolutionTransport),
            SolveCertificate::SolutionTransport(value),
        ) => transport::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::TotalPredicateSplit),
            SolveCertificate::TotalPredicateSplit(value),
        ) => split::check(cas, draft, value, premises, budget)?,
        _ => return Err(proof::invalid()),
    }
    Ok(inherited)
}
