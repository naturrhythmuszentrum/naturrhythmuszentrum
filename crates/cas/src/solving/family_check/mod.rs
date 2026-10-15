// SPDX-License-Identifier: AGPL-3.0-only
//! Universal family membership and recovery from arbitrary original solutions.

mod universal;

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Cas;
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions, Statement};
use crate::solving::elementary::proof::{conditions, invalid};
use crate::solving::{
    CertificateParameterImageCoverage, SolutionBranchKind, SolveCertificate, SolveDraft,
    SolveSchema, SolveStatement,
};

/// The supplied universal checker is executable independently of automatic family production.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Only the universal parameter-image schema belongs to this leaf.
pub(crate) const fn schema_availability(tag: u8) -> super::Availability {
    if tag == 16 {
        availability()
    } else {
        super::Availability::Unavailable
    }
}

/// Associated set coverage contains precisely the universally checked family image.
fn image(
    cas: &Cas,
    claim: &crate::solving::StatementCoverage,
    certificate: &CertificateParameterImageCoverage,
) -> Result<(), Fault> {
    if (&claim.problem, &claim.region) != (&certificate.problem, &certificate.region) {
        return Err(invalid());
    }
    let SolveDraft::SolutionSet(set) =
        crate::solving::graph::object(&cas.state, claim.set.logical_id())?
    else {
        return Err(invalid());
    };
    let [handle] = set.branches.as_slice() else {
        return Err(invalid());
    };
    let SolveDraft::SolutionBranch(branch) =
        crate::solving::graph::object(&cas.state, handle.logical_id())?
    else {
        return Err(invalid());
    };
    let original = crate::solving::elementary::proof::problem(cas, &claim.problem)?;
    if (&branch.problem, &branch.region, &branch.domain)
        != (&claim.problem, &claim.region, &original.domain)
    {
        return Err(invalid());
    }
    if !matches!(&branch.kind, SolutionBranchKind::Image { family } if family == &certificate.family)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Check exact conclusion strength; membership alone never upgrades an image to coverage.
fn conclusion(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateParameterImageCoverage,
) -> Result<bool, Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    match statement.as_ref() {
        SolveStatement::FamilyMembership(value)
            if (&value.problem, &value.family, &value.region)
                == (
                    &certificate.problem,
                    &certificate.family,
                    &certificate.region,
                ) =>
        {
            if certificate.recovery.is_some() || !certificate.recovery_proofs.is_empty() {
                return Err(invalid());
            }
            Ok(false)
        }
        SolveStatement::FamilyCoverage(value)
            if (
                &value.problem,
                &value.family,
                &value.region,
                Some(&value.recovery),
            ) == (
                &certificate.problem,
                &certificate.family,
                &certificate.region,
                certificate.recovery.as_ref(),
            ) =>
        {
            Ok(true)
        }
        SolveStatement::Coverage(value) => {
            image(cas, value, certificate)?;
            Ok(true)
        }
        _ => Err(invalid()),
    }
}

/// Recompute symbolic membership and the recovery identity, preserving all scoped conditions.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let crate::proof_model::Certificate::Solve(certificate) = &draft.certificate else {
        return Err(invalid());
    };
    let (
        crate::rule::Primitive::Solve(SolveSchema::ParameterImageCoverage),
        SolveCertificate::ParameterImageCoverage(value),
    ) = (draft.rule, certificate.as_ref())
    else {
        return Err(invalid());
    };
    let inherited = conditions(cas, draft, premises, budget)?;
    let coverage = conclusion(cas, draft, value)?;
    for slot in value
        .membership
        .iter()
        .chain(&value.recovery_proofs)
        .chain(&value.domain)
        .chain(&value.scope)
    {
        crate::solving::elementary::proof::premise(premises, *slot)?;
    }
    universal::check(cas, draft, value, premises, coverage, budget)?;
    Ok(inherited)
}
