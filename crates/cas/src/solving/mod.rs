// SPDX-License-Identifier: AGPL-3.0-only
//! Exact solving objects and explicit capability boundaries.

#[expect(
    clippy::redundant_pub_crate,
    reason = "typed admission helpers stay within the crate"
)]
pub(crate) mod graph;
mod model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the typed checker facade stays within the crate"
)]
mod registry;
#[expect(
    clippy::redundant_pub_crate,
    reason = "typed encoding helpers stay within the crate"
)]
pub(crate) mod wire;

#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod coefficients;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod elementary;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod family_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
mod isolation;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod linear_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
mod linear_producer;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
mod local;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod numeric;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod orbits;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod radicals;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
mod rational;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod solve_search;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod symmetric;
#[expect(
    clippy::redundant_pub_crate,
    reason = "leaf availability is shared only within the crate"
)]
pub(crate) mod towers;

pub(crate) use registry::check;
pub use registry::{SOLVE_SCHEMAS, SolveSchema, active_schema_bitmap};

pub use model::{
    AdjunctionMode, BinomialMode, CertificateBinomialCoverage, CertificateCoefficientIdentity,
    CertificateComponentSplit, CertificateConjugationTransport, CertificateExactLinearEquivalence,
    CertificateExtensionUnit, CertificateImplicitFunctionBranch, CertificateMembership,
    CertificateMultiplicityTransfer, CertificateParameterImageCoverage,
    CertificatePermutationOrbitCoverage, CertificateProductCompleteness,
    CertificateRealIsolationAndOrder, CertificateRealRestriction, CertificateRootAdjunction,
    CertificateRootSelectionRefinement, CertificateSolutionTransport,
    CertificateTotalPredicateSplit, CoefficientDomainData, CoefficientMapData,
    CoefficientValueData, CoefficientValueObject, ComponentSplitData, Coordinate, CoordinateRole,
    CoordinateSpaceData, ExclusionPolicy, ExtensionLevelData, FamilyMapData, GeneratorPair,
    IdentityMode, IsolationInterval, LocalBranchData, LocalMode, MapMode, MapTarget,
    MultiplicityMode, Neighborhood, PolynomialData, PolynomialTerm, PredicateSplitMode, PrimePower,
    RationalInterval, RealRestrictionTransport, RefinementMode, RefinementTargetKind,
    RegularityKind, RelationMultiple, RestrictionCondition, RestrictionConditionKind,
    RestrictionPair, RootFactor, RootRelationData, RootSelectionData, RowOperation,
    ScopedCoefficientRelation, SelectionMode, SolutionBranchData, SolutionBranchKind,
    SolutionSetData, SolveAnswer, SolveBlocker, SolveCertificate, SolveDraft, SolveForm, SolveGoal,
    SolveProblemData, SolveReason, SolveSpec, SolveStatement, SolveSubstitution, SolveView,
    SquareFreeRequest, StatementCoefficientEquality, StatementComponentCover, StatementCoverage,
    StatementEquivalent, StatementFamilyCoverage, StatementFamilyMembership, StatementIsolation,
    StatementLinearSystem, StatementLocalBranch, StatementLocalDerivative, StatementMember,
    StatementMultiplicity, StatementOrbit, StatementPresentation, StatementRank,
    StatementRealValues, StatementRootExistence, StatementRootOrder, StatementSelectionUnique,
    StatementUnit, TupleData,
};

/// Availability is separate from a schema's stable mathematical identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    /// No executable checker or producer exists in this build.
    Unavailable,
    /// The owning lane has enabled independently replayed checking.
    Available,
}

/// Report leaf capabilities in the documented lane ownership order.
#[must_use]
pub const fn capabilities() -> [Availability; 14] {
    [
        numeric::availability(),
        coefficients::availability(),
        towers::availability(),
        elementary::availability(),
        linear_check::availability(),
        family_check::availability(),
        orbits::availability(),
        symmetric::availability(),
        radicals::availability(),
        solve_search::availability(),
        rational::availability(),
        isolation::availability(),
        linear_producer::availability(),
        local::availability(),
    ]
}

/// A later producer may handle a prepared request, or decline it without publishing a proposal.
pub(crate) type SolveExecutor = fn(
    &mut crate::graph::Cas,
    &crate::request_model::Request,
    &crate::graph::Handle,
    &crate::search::SearchPolicy,
    &mut crate::budget::Budget,
    &mut crate::budget::Budget,
) -> Option<crate::request_model::Outcome>;

/// The shared service hook offers only implemented producers and otherwise reports the missing capability.
pub(crate) fn execute(
    cas: &mut crate::graph::Cas,
    request: &crate::request_model::Request,
    record: &crate::graph::Handle,
    policy: &crate::search::SearchPolicy,
    generation: &mut crate::budget::Budget,
    checker: &mut crate::budget::Budget,
) -> crate::request_model::Outcome {
    for executor in [
        rational::executor(),
        linear_producer::executor(),
        local::executor(),
        solve_search::executor(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(outcome) = executor(cas, request, record, policy, generation, checker) {
            return outcome;
        }
    }
    crate::request_model::Outcome::Unresolved {
        request: record.clone(),
        solve_reason: Some(SolveReason::CoefficientDomainUnavailable),
        consistency: crate::request_model::ContextConsistency::Unknown,
        blocker: None,
        usage: generation.usage(),
    }
}
