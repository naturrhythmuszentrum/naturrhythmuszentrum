// SPDX-License-Identifier: AGPL-3.0-only
//! Independently replayed ordered-field signs and Sturm certificates.

mod cell_exists;
#[expect(
    clippy::redundant_pub_crate,
    reason = "RealExact presentation shares semantic Real closure"
)]
pub(crate) mod closure;
mod closure_evidence;
mod count;
mod count_signs;
mod coverage;
mod sign;
mod sign_evidence;
mod sign_rules;
mod sturm;

use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, ScopedConditions};
use crate::solving::{SolveCertificate, SolveSchema};

/// All new real theorem work keeps a distinct diagnostic site.
const AT: Location = Location::new("real solving certificates");

/// Invalid supplied evidence never states a negative mathematical result.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Require the exact local coefficient region and original declaration header.
fn region(cas: &Cas, context: &Handle, domain: &Handle) -> Result<(), Fault> {
    let frame = cas.state.context(cas.checked(context)?)?;
    let shape = cas.state.region(cas.checked(domain)?)?;
    if frame.header_id != shape.header || frame.admissible != domain.logical_id() {
        return Err(invalid());
    }
    Ok(())
}

/// Dispatch only registered finite certificate shapes, inheriting every scoped prerequisite.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the registry invokes the real checker"
)]
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let conditions = crate::solving::coefficients::proof::conditions(cas, draft, premises, budget)?;
    let Certificate::Solve(certificate) = &draft.certificate else {
        return Err(invalid());
    };
    match (draft.rule, certificate.as_ref()) {
        (
            crate::rule::Primitive::Solve(SolveSchema::RealSign),
            SolveCertificate::RealSign(certificate),
        ) => sign::check(cas, draft, certificate, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::RealRootCount),
            SolveCertificate::RealRootCount(certificate),
        ) => count::check(cas, draft, certificate, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::RealCellExists),
            SolveCertificate::RealCellExists(certificate),
        ) => cell_exists::check(cas, draft, certificate, premises, budget)?,
        _ => return Err(invalid()),
    }
    Ok(conditions)
}

mod cells_produce;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real producer shares finite construction components"
)]
pub(crate) mod construction;
mod count_produce;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real producer shares finite construction components"
)]
pub(crate) mod derive;
mod factor_sign;
mod finite;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real producer shares finite construction components"
)]
pub(crate) mod free;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real producer shares finite construction components"
)]
pub(crate) mod leaf;
mod lower_produce;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real entrypoint returns the shared unsealed proposal"
)]
pub(crate) mod producer;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real producer shares finite construction components"
)]
pub(crate) mod render;
mod roots_produce;
mod sign_produce;
mod single_produce;
mod three_produce;
#[expect(
    clippy::redundant_pub_crate,
    reason = "checked Real zero predicates feed the same-sort evaluator"
)]
pub(crate) mod zero;

#[cfg(test)]
mod parameter_tests;
