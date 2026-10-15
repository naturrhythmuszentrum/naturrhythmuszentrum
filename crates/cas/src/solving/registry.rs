// SPDX-License-Identifier: AGPL-3.0-only
//! Stable solving schema identities, weights and independent availability.

use super::Availability;
use crate::rule::{CertificateSchema, ConclusionKind, PremiseShape, Primitive, RuleSpec};

/// Closed semantic-version-one solving schemas, independent of implementation availability.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SolveSchema {
    /// The `coeff.identity` checker and certificate subtype 0.
    CoefficientIdentity,
    /// The `extension.adjoin` checker and certificate subtype 1.
    RootAdjunction,
    /// The `extension.unit` checker and certificate subtype 2.
    ExtensionUnit,
    /// The `extension.split` checker and certificate subtype 3.
    ComponentSplit,
    /// The `solve.member` checker and certificate subtype 4.
    Membership,
    /// The `solve.substitute` checker and certificate subtype 5.
    SolutionTransport,
    /// The `solve.split` checker and certificate subtype 6.
    TotalPredicateSplit,
    /// The `root.binomial` checker and certificate subtype 7.
    BinomialCoverage,
    /// The `root.select` checker and certificate subtype 8.
    RootSelectionRefinement,
    /// The `linear.equivalent` checker and certificate subtype 9.
    ExactLinearEquivalence,
    /// The `solve.product` checker and certificate subtype 10.
    ProductCompleteness,
    /// The `solve.multiplicity` checker and certificate subtype 11.
    MultiplicityTransfer,
    /// The `complex.conjugate` checker and certificate subtype 12.
    ConjugationTransport,
    /// The `solve.real-restrict` checker and certificate subtype 13.
    RealRestriction,
    /// The `orbit.cover` checker and certificate subtype 14.
    PermutationOrbitCoverage,
    /// The `root.isolate` checker and certificate subtype 15.
    RealIsolationAndOrder,
    /// The `solve.family` checker and certificate subtype 16.
    ParameterImageCoverage,
    /// The `solve.local-branch` checker and certificate subtype 17.
    ImplicitFunctionBranch,
}

/// Every solving schema in its fixed certificate subtype order.
pub const SOLVE_SCHEMAS: [SolveSchema; 18] = [
    SolveSchema::CoefficientIdentity,
    SolveSchema::RootAdjunction,
    SolveSchema::ExtensionUnit,
    SolveSchema::ComponentSplit,
    SolveSchema::Membership,
    SolveSchema::SolutionTransport,
    SolveSchema::TotalPredicateSplit,
    SolveSchema::BinomialCoverage,
    SolveSchema::RootSelectionRefinement,
    SolveSchema::ExactLinearEquivalence,
    SolveSchema::ProductCompleteness,
    SolveSchema::MultiplicityTransfer,
    SolveSchema::ConjugationTransport,
    SolveSchema::RealRestriction,
    SolveSchema::PermutationOrbitCoverage,
    SolveSchema::RealIsolationAndOrder,
    SolveSchema::ParameterImageCoverage,
    SolveSchema::ImplicitFunctionBranch,
];

impl SolveSchema {
    /// The stable checker and certificate subtype tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::CoefficientIdentity => 0,
            Self::RootAdjunction => 1,
            Self::ExtensionUnit => 2,
            Self::ComponentSplit => 3,
            Self::Membership => 4,
            Self::SolutionTransport => 5,
            Self::TotalPredicateSplit => 6,
            Self::BinomialCoverage => 7,
            Self::RootSelectionRefinement => 8,
            Self::ExactLinearEquivalence => 9,
            Self::ProductCompleteness => 10,
            Self::MultiplicityTransfer => 11,
            Self::ConjugationTransport => 12,
            Self::RealRestriction => 13,
            Self::PermutationOrbitCoverage => 14,
            Self::RealIsolationAndOrder => 15,
            Self::ParameterImageCoverage => 16,
            Self::ImplicitFunctionBranch => 17,
        }
    }

    /// The exact owning leaf's executable capability in this build.
    #[must_use]
    pub const fn availability(self) -> Availability {
        match self {
            Self::CoefficientIdentity => super::coefficients::schema_availability(self.tag()),
            Self::RootAdjunction | Self::ExtensionUnit | Self::ComponentSplit => {
                super::towers::schema_availability(self.tag())
            }
            Self::Membership
            | Self::SolutionTransport
            | Self::TotalPredicateSplit
            | Self::BinomialCoverage
            | Self::RootSelectionRefinement
            | Self::ProductCompleteness
            | Self::MultiplicityTransfer
            | Self::ConjugationTransport
            | Self::RealRestriction => super::elementary::schema_availability(self.tag()),
            Self::ExactLinearEquivalence => super::linear_check::schema_availability(self.tag()),
            Self::PermutationOrbitCoverage => super::orbits::schema_availability(self.tag()),
            Self::RealIsolationAndOrder => super::isolation::schema_availability(self.tag()),
            Self::ParameterImageCoverage => super::family_check::schema_availability(self.tag()),
            Self::ImplicitFunctionBranch => super::local::schema_availability(self.tag()),
        }
    }

    /// Immutable type and cost metadata; registration does not confer theorem authority.
    #[must_use]
    pub const fn specification(self) -> RuleSpec {
        let (key, weight) = match self {
            Self::CoefficientIdentity => ("coeff.identity", 4),
            Self::RootAdjunction => ("extension.adjoin", 8),
            Self::ExtensionUnit => ("extension.unit", 8),
            Self::ComponentSplit => ("extension.split", 12),
            Self::Membership => ("solve.member", 4),
            Self::SolutionTransport => ("solve.substitute", 4),
            Self::TotalPredicateSplit => ("solve.split", 4),
            Self::BinomialCoverage => ("root.binomial", 8),
            Self::RootSelectionRefinement => ("root.select", 8),
            Self::ExactLinearEquivalence => ("linear.equivalent", 8),
            Self::ProductCompleteness => ("solve.product", 8),
            Self::MultiplicityTransfer => ("solve.multiplicity", 8),
            Self::ConjugationTransport => ("complex.conjugate", 4),
            Self::RealRestriction => ("solve.real-restrict", 8),
            Self::PermutationOrbitCoverage => ("orbit.cover", 8),
            Self::RealIsolationAndOrder => ("root.isolate", 12),
            Self::ParameterImageCoverage => ("solve.family", 8),
            Self::ImplicitFunctionBranch => ("solve.local-branch", 16),
        };
        RuleSpec {
            primitive: Primitive::Solve(self),
            key,
            version: 1,
            weight,
            conclusion: ConclusionKind::Solve,
            premises: PremiseShape::SolveEvidence,
            certificate: CertificateSchema::Solve,
            available: matches!(self.availability(), Availability::Available),
        }
    }
}

/// Executable solving schemas as subtype-indexed bits, for service and cache identity.
#[must_use]
pub const fn active_schema_bitmap() -> u32 {
    let mut bitmap = 0;
    let mut index = 0;
    while index < SOLVE_SCHEMAS.len() {
        let schema = SOLVE_SCHEMAS[index];
        if matches!(schema.availability(), Availability::Available) {
            bitmap |= 1 << schema.tag();
        }
        index += 1;
    }
    bitmap
}

/// The stable common hook delegates only to the exact owning leaf's independent checker.
pub(crate) fn check(
    cas: &crate::graph::Cas,
    draft: &crate::proof_model::ApplicationDraft,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut crate::budget::Budget,
) -> Result<crate::proof_model::ScopedConditions, crate::error::Fault> {
    let Primitive::Solve(schema) = draft.rule else {
        return Err(crate::error::Fault::invalid(
            crate::error::InvalidCode::Rule,
            crate::error::Location::new("solving dispatch"),
        ));
    };
    match schema {
        SolveSchema::CoefficientIdentity => {
            super::coefficients::check(cas, draft, premises, budget)
        }
        SolveSchema::RootAdjunction | SolveSchema::ExtensionUnit | SolveSchema::ComponentSplit => {
            super::towers::check(cas, draft, premises, budget)
        }
        SolveSchema::Membership
        | SolveSchema::SolutionTransport
        | SolveSchema::TotalPredicateSplit
        | SolveSchema::BinomialCoverage
        | SolveSchema::RootSelectionRefinement
        | SolveSchema::ProductCompleteness
        | SolveSchema::MultiplicityTransfer
        | SolveSchema::ConjugationTransport
        | SolveSchema::RealRestriction => super::elementary::check(cas, draft, premises, budget),
        SolveSchema::ExactLinearEquivalence => {
            super::linear_check::check(cas, draft, premises, budget)
        }
        SolveSchema::PermutationOrbitCoverage => super::orbits::check(cas, draft, premises, budget),
        SolveSchema::RealIsolationAndOrder => super::isolation::check(cas, draft, premises, budget),
        SolveSchema::ParameterImageCoverage => {
            super::family_check::check(cas, draft, premises, budget)
        }
        SolveSchema::ImplicitFunctionBranch => super::local::check(cas, draft, premises, budget),
    }
}
