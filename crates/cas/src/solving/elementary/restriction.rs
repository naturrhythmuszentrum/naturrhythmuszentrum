// SPDX-License-Identifier: AGPL-3.0-only
//! Symbolic real restriction with explicit coefficient, declaration and condition transport.

#[expect(
    clippy::redundant_pub_crate,
    reason = "transport checks remain crate-private"
)]
mod branches;
#[expect(
    clippy::redundant_pub_crate,
    reason = "transport checks remain crate-private"
)]
mod scope;
#[expect(
    clippy::redundant_pub_crate,
    reason = "transport checks remain crate-private"
)]
mod syntax;
#[expect(
    clippy::redundant_pub_crate,
    reason = "transport checks remain crate-private"
)]
mod transport;

use super::proof::{invalid, premise, problem};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Cas;
use crate::model::Field;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};
use crate::solving::{
    CertificateRealRestriction, SolveCertificate, SolveDraft, SolveStatement, StatementCoverage,
};

/// Recover the precise source and target claims from their authoritative statement payloads.
fn claims<'a>(
    draft: &'a ApplicationDraft,
    certificate: &CertificateRealRestriction,
    premises: &'a [CheckedApplication],
) -> Result<(&'a StatementCoverage, &'a StatementCoverage), Fault> {
    let Statement::Solve(target) = &draft.conclusion else {
        return Err(invalid());
    };
    let Statement::Solve(source) = &premise(premises, certificate.coverage)?.judgment.statement
    else {
        return Err(invalid());
    };
    let (SolveStatement::Coverage(real), SolveStatement::Coverage(complex)) =
        (target.as_ref(), source.as_ref())
    else {
        return Err(invalid());
    };
    if (&real.problem, &real.set) != (&certificate.problem, &certificate.set) {
        return Err(invalid());
    }
    let overall = certificate.transport.regions.first().ok_or_else(invalid)?;
    if (&real.region, &complex.region) != (&overall.real, &overall.complex) {
        return Err(invalid());
    }
    Ok((real, complex))
}

/// No qualified source hypothesis is inherited across a header boundary.
pub(crate) fn conditions(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Certificate::Solve(certificate) = &draft.certificate else {
        return Err(invalid());
    };
    let SolveCertificate::RealRestriction(certificate) = certificate.as_ref() else {
        return Err(invalid());
    };
    let (real, complex) = claims(draft, certificate, premises)?;
    let mut comparison = syntax::Correspondence::new(
        cas,
        problem(cas, &real.problem)?,
        problem(cas, &complex.problem)?,
        &certificate.transport,
        premises,
        budget,
    )?;
    scope::check(&mut comparison, draft, budget)
}

/// Independently reconstruct the exact real subset of a complete complex symbolic cover.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRealRestriction,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (target, source) = claims(draft, certificate, premises)?;
    let real = problem(cas, &target.problem)?;
    let complex = problem(cas, &source.problem)?;
    if (&real.ambient, &complex.ambient) != (&Field::Real, &Field::Complex) {
        return Err(invalid());
    }
    super::proof::region(cas, &draft.context, real, &target.region)?;
    super::member::contained(cas, &target.region, &real.region, budget)?;
    let mut comparison =
        syntax::Correspondence::new(cas, real, complex, &certificate.transport, premises, budget)?;
    transport::check(&mut comparison, budget)?;
    transport::covers(certificate, premises, budget)?;
    for slot in &certificate.classification {
        premise(premises, *slot)?;
    }
    let SolveDraft::SolutionSet(original) =
        crate::solving::graph::object(&cas.state, cas.checked(&source.set)?)?
    else {
        return Err(invalid());
    };
    let SolveDraft::SolutionSet(restricted) =
        crate::solving::graph::object(&cas.state, cas.checked(&target.set)?)?
    else {
        return Err(invalid());
    };
    if (&original.problem, &restricted.problem) != (&source.problem, &target.problem) {
        return Err(invalid());
    }
    branches::check(&mut comparison, certificate, original, restricted, budget)
}
