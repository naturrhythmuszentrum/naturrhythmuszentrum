// SPDX-License-Identifier: AGPL-3.0-only
//! Independently replayed membership, strict coordinate transport and total degree partitions.

mod binomial;
mod complex;
mod conjugation;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod constraints;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod evaluate;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod evidence;
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact inverse evidence is shared with linear replay"
)]
pub(crate) mod inverse;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod member;
mod multiplicity;
mod nonreal;
mod presentation;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod product;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod proof;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the later search lane consumes unsealed elementary recipes"
    )
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "elementary recipes remain crate-private"
)]
pub(crate) mod recipes;
mod reduction;
mod refine;
mod restriction;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod selected;
mod split;
mod total;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod transport;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod value;

/// Elementary availability follows the implemented independently checked schemas.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Enable a schema only together with its concrete checker.
pub(crate) const fn schema_availability(tag: u8) -> super::Availability {
    if matches!(tag, 4..=8 | 10..=13) {
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
    let inherited = if matches!(certificate.as_ref(), SolveCertificate::RealRestriction(_)) {
        restriction::conditions(cas, draft, premises, budget)?
    } else {
        proof::conditions(cas, draft, premises, budget)?
    };
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
        (
            crate::rule::Primitive::Solve(SolveSchema::ProductCompleteness),
            SolveCertificate::ProductCompleteness(value),
        ) => product::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::BinomialCoverage),
            SolveCertificate::BinomialCoverage(value),
        ) => binomial::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::RootSelectionRefinement),
            SolveCertificate::RootSelectionRefinement(value),
        ) => refine::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::MultiplicityTransfer),
            SolveCertificate::MultiplicityTransfer(value),
        ) => multiplicity::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::ConjugationTransport),
            SolveCertificate::ConjugationTransport(value),
        ) => conjugation::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::RealRestriction),
            SolveCertificate::RealRestriction(value),
        ) => restriction::check(cas, draft, value, premises, budget)?,
        _ => return Err(proof::invalid()),
    }
    Ok(inherited)
}
