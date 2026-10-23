// SPDX-License-Identifier: AGPL-3.0-only
//! Independently replayed membership, strict coordinate transport and total degree partitions.
//!
//! # Membership, coverage and presentation
//!
//! The elementary leaf enables `solve.member`, `solve.substitute` and `solve.split` (tags 4–6). Membership simultaneously substitutes the ordered unknown tuple into the original predicate, solution domain and every retained original denominator. Reversible coordinate changes independently check both directions, strict domains and inverse compositions. Total zero/nonzero and real sign partitions require definedness before asserting a cover; regional coverage assembly retains every supplied branch. All used hypotheses and residual guards remain visible. Supplied-certificate checking alone grants no automatic producer capability.
//!
//! The supplied elementary checks also enable exact binomial order and coverage, constructive root-selection refinement, and product completeness (tags 7, 8 and 10). Positive factor multiplicities reconstruct the full original polynomial; excluded roots must violate a retained original denominator. Complete products cover the zero-radicand singleton and nonzero constant empty set. Degree-generic unsealed recipes strip specialized zero leading coefficients and translate away the subleading term, deriving quadratics through the same binomial path. Root values require sealed existence for their exact selection. Finite covers transport through the checked inverse maps; no numerical root sampling or radical branch convention supplies a theorem.
//!
//! Tags 11 and 12 add derivative multiplicities and standard complex-conjugation expression induction. Tag 13 and the complete `RealRestriction` API, payload, rule and route are removed; no reader or alias remains. Derivative mode checks every derivative through the first nonzero order; square-free-layer multiplicity certificates remain unsupported. Conjugation never identifies independent root choices. Radical Presentation checks nested binomial/root-of-unity provenance and rejects unresolved general algebraic placeholders. A binomial radical requires its specialized leading coefficient to be proved nonzero on the claimed region; constructive root existence on a degree-degenerate stratum is insufficient. `ExactFamily` presentations require image or empty branches, and explicit family parameters remain bound. A parent Presentation may name earlier closed child Presentation statements through compatibility slots. Composition requires the same original problem and requested form, exact unchanged branch identities, containment of each child region, and a separate complete Coverage theorem for the exact combined set; every target branch must be supplied. This composes recursively across coefficient splits without reopening or widening leaf root and family proofs. Exact supplied-certificate checking is separate from the available automatic producers.

mod binomial;
#[expect(
    clippy::redundant_pub_crate,
    reason = "family evaluation shares the existing exact components"
)]
pub(crate) mod complex;
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
    reason = "crate tests check exact exclusion inferences and boundaries"
)]
pub(crate) mod exclusion;
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
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact solving checks remain crate-private"
)]
pub(crate) mod selected;
mod split;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Real signs share strict scalar totality checking"
)]
pub(crate) mod total;
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
    if matches!(tag, 4..=8 | 10..=12) {
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
    let (inherited, empty) = proof::conditions(cas, draft, premises, budget)?;
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
        ) => product::check(cas, draft, value, premises, &empty, budget)?,
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
        _ => return Err(proof::invalid()),
    }
    Ok(inherited)
}
