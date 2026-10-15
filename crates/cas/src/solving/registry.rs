// SPDX-License-Identifier: AGPL-3.0-only
//! Unavailable typed dispatch fixed before leaf ownership is handed off.

use super::{Availability, SolveCertificate};

/// Per-subtype availability stays independent even when several schemas share a leaf.
const fn checker_availability(value: &SolveCertificate) -> Availability {
    let tag = value.tag();
    match value {
        SolveCertificate::CoefficientIdentity(_) => super::coefficients::schema_availability(tag),
        SolveCertificate::RootAdjunction(_)
        | SolveCertificate::ExtensionUnit(_)
        | SolveCertificate::ComponentSplit(_) => super::towers::schema_availability(tag),
        SolveCertificate::Membership(_)
        | SolveCertificate::SolutionTransport(_)
        | SolveCertificate::TotalPredicateSplit(_)
        | SolveCertificate::BinomialCoverage(_)
        | SolveCertificate::RootSelectionRefinement(_)
        | SolveCertificate::ProductCompleteness(_)
        | SolveCertificate::MultiplicityTransfer(_)
        | SolveCertificate::ConjugationTransport(_)
        | SolveCertificate::RealRestriction(_) => super::elementary::schema_availability(tag),
        SolveCertificate::ExactLinearEquivalence(_) => {
            super::linear_check::schema_availability(tag)
        }
        SolveCertificate::PermutationOrbitCoverage(_) => super::orbits::schema_availability(tag),
        SolveCertificate::RealIsolationAndOrder(_) => super::isolation::schema_availability(tag),
        SolveCertificate::ParameterImageCoverage(_) => {
            super::family_check::schema_availability(tag)
        }
        SolveCertificate::ImplicitFunctionBranch(_) => super::local::schema_availability(tag),
    }
}

/// Reject unavailable typed applications without accepting theorem placeholders.
pub(crate) fn check(
    cas: &crate::graph::Cas,
    draft: &crate::proof_model::ApplicationDraft,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut crate::budget::Budget,
) -> Result<crate::proof_model::ScopedConditions, crate::error::Fault> {
    let crate::proof_model::Certificate::Solve(certificate) = &draft.certificate else {
        return Err(crate::error::Fault::invalid(
            crate::error::InvalidCode::Rule,
            crate::error::Location::new("solving dispatch"),
        ));
    };
    if !matches!(checker_availability(certificate), Availability::Available) {
        return Err(crate::error::Fault::invalid(
            crate::error::InvalidCode::UnsupportedFragment,
            crate::error::Location::new("unavailable solving checker"),
        ));
    }
    match certificate.as_ref() {
        SolveCertificate::CoefficientIdentity(_) => {
            super::coefficients::check(cas, draft, premises, budget)
        }
        SolveCertificate::RootAdjunction(_)
        | SolveCertificate::ExtensionUnit(_)
        | SolveCertificate::ComponentSplit(_) => super::towers::check(cas, draft, premises, budget),
        SolveCertificate::Membership(_)
        | SolveCertificate::SolutionTransport(_)
        | SolveCertificate::TotalPredicateSplit(_)
        | SolveCertificate::BinomialCoverage(_)
        | SolveCertificate::RootSelectionRefinement(_)
        | SolveCertificate::ProductCompleteness(_)
        | SolveCertificate::MultiplicityTransfer(_)
        | SolveCertificate::ConjugationTransport(_)
        | SolveCertificate::RealRestriction(_) => {
            super::elementary::check(cas, draft, premises, budget)
        }
        SolveCertificate::ExactLinearEquivalence(_) => {
            super::linear_check::check(cas, draft, premises, budget)
        }
        SolveCertificate::PermutationOrbitCoverage(_) => {
            super::orbits::check(cas, draft, premises, budget)
        }
        SolveCertificate::RealIsolationAndOrder(_) => {
            super::isolation::check(cas, draft, premises, budget)
        }
        SolveCertificate::ParameterImageCoverage(_) => {
            super::family_check::check(cas, draft, premises, budget)
        }
        SolveCertificate::ImplicitFunctionBranch(_) => {
            super::local::check(cas, draft, premises, budget)
        }
    }
}
