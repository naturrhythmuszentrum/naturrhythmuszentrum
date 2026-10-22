// SPDX-License-Identifier: AGPL-3.0-only
//! Exact solving objects and explicit capability boundaries.
//!
//! # Capability registry
//!
//! `SOLVE_SCHEMAS` publishes the 21 current certificate subtype identities; `active_schema_bitmap()` and `SearchPolicy::active_schema_bitmap()` expose the separate executable capability. The bitmap belongs to search state keys and response audits, while mathematical witnesses carry only their exact used-rule manifest. Policy identity lookup uses a dedicated `(key, version)` index and never scans unrelated graph records.
//!
//! |Surface|Available contract|Boundary|
//! |-------|------------------|--------|
//! |Registry|80 version-one primitives: 59 general and 19 executable solving rules among 21 solving entries; logical policy v10.|`root.isolate` and `solve.local-branch` remain unavailable; subtype 13 is removed.|
//! |Complex producer|Complete symbolic cubic with original Complex parameters, all degree drops and Radicals presentation.|No general higher-degree radical or generic Galois-obstruction checker.|
//! |Real producer|Complete symbolic cubic with Real radicals or counted Real cells and `RealExact` presentation.|Three-root cells do not satisfy a separate Real-radicals request.|
//! |Mixed families|Explicit Real base and Complex fibre, complete linear/binomial and algebraic cubic pieces, checked optional base/fibre covers.|Mixed cubic radical discovery and automatic general nonlinear systems remain unavailable.|
//! |Portable answer|One presentation closure supplies exact expected Coverage and Presentation under a cumulative ledger.|Presentation without matching reachable coverage cannot certify a complete answer.|
//!
//! The registry counts and unavailable keys are pinned by (´test:integration:complete-answer-registry-inventory´). Constructors and represented payloads grant no theorem authority.

mod evaluate;
#[expect(
    clippy::redundant_pub_crate,
    reason = "native cases dispatch only the new family shapes"
)]
pub(crate) mod family;
pub use family::cover::{FamilyCoverGroup, FamilyCoverRequest, FamilyCoverResult, FamilyPiece};
pub use family::membership::PlacementOutcome;
#[expect(
    clippy::redundant_pub_crate,
    reason = "typed admission helpers stay within the crate"
)]
pub(crate) mod graph;
mod model;
mod real;
#[expect(
    clippy::redundant_pub_crate,
    reason = "scalar-space admission is shared within the crate"
)]
pub(crate) mod sorts;
pub use sorts::{
    AmbientPlacement, AmbientResolution, CertificateCoefficientEvaluation,
    CertificateRealCellExists, CertificateRealRootCount, CertificateRealSign,
    CoefficientEvaluationData, EvaluationBinding, EvaluationTarget, FamilyArgument,
    FamilyArgumentRole, FamilyExpressionData, FibrePieceData, LiesOver, ParameterBaseData,
    PlacementUse, RealCellData, RealEndpoint, RealSignConclusion, RealSignRule, RealSignStep,
    ScalarPieceData, ScalarRoute, ScalarSpaceData, SortRouting, SpaceFamilyData,
    SpacePlacementData, StatementCoefficientEvaluation, StatementRealCellExists,
    StatementRealRootCount, StatementRealSign,
};
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
    CertificateRealIsolationAndOrder, CertificateRootAdjunction,
    CertificateRootSelectionRefinement, CertificateSolutionTransport,
    CertificateTotalPredicateSplit, CoefficientDomainData, CoefficientMapData,
    CoefficientValueData, CoefficientValueObject, ComponentSplitData, Coordinate, CoordinateRole,
    CoordinateSpaceData, ExclusionPolicy, ExtensionLevelData, FamilyMapData, GeneratorPair,
    IdentityMode, IsolationInterval, LocalBranchData, LocalMode, MapMode, MapTarget,
    MultiplicityMode, Neighborhood, PolynomialData, PolynomialTerm, PredicateSplitMode, PrimePower,
    RationalInterval, RefinementMode, RefinementTargetKind, RegularityKind, RelationMultiple,
    RootFactor, RootRelationData, RootSelectionData, RowOperation, ScopedCoefficientRelation,
    SelectionMode, SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveAnswer,
    SolveBlocker, SolveCertificate, SolveDraft, SolveForm, SolveGoal, SolveProblemData,
    SolveReason, SolveSpec, SolveStatement, SolveSubstitution, SolveView, SquareFreeRequest,
    StatementCoefficientEquality, StatementComponentCover, StatementCoverage, StatementEquivalent,
    StatementFamilyCoverage, StatementFamilyMembership, StatementIsolation, StatementLinearSystem,
    StatementLocalBranch, StatementLocalDerivative, StatementMember, StatementMultiplicity,
    StatementOrbit, StatementPresentation, StatementRank, StatementRealValues,
    StatementRootExistence, StatementRootOrder, StatementSelectionUnique, StatementUnit, TupleData,
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
    let routing = match sorts::preflight(cas, request, generation) {
        Ok(routing) => routing,
        Err(fault) => return crate::request_model::failure(fault, &generation.usage()),
    };
    if let Some(reason) = routing.refusal {
        return crate::request_model::Outcome::Unresolved {
            solve_reason: Some(reason),
            request: record.clone(),
            consistency: crate::request_model::ContextConsistency::Unknown,
            blocker: None,
            usage: generation.usage(),
        };
    }
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
