// SPDX-License-Identifier: AGPL-3.0-only
//! Exact solving syntax and closed, untrusted theorem payloads.

use crate::graph::{Handle, Reference};
use crate::model::{AtomicSort, Declaration, Field, Scope};
use crate::number::Rational;

/// `CoordinateRole` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateRole {
    /// A free coefficient parameter.
    Coefficient,
    /// An unknown in the original predicate.
    Unknown,
    /// A bound family parameter.
    Family,
}

/// `SquareFreeRequest` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SquareFreeRequest {
    /// No square-free request.
    NotRequested,
    /// Square-freeness is requested, never asserted.
    Required,
}

/// `MapMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapMode {
    /// An injective homomorphism requires a theorem.
    Embedding,
    /// A partial evaluation retains denominator obligations.
    Specialization,
}

/// `SelectionMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionMode {
    /// Any compatible root tuple.
    Any,
    /// A jointly constrained root tuple.
    Constrained,
    /// A named principal convention.
    Principal,
    /// One real root in an exact interval.
    Isolated,
}

/// `SolveGoal` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveGoal {
    /// Prove at least one member, or prove emptiness.
    Candidates,
    /// Prove whole-domain coverage.
    Complete,
}

/// `SolveForm` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveForm {
    /// Any exact representation.
    AnyExact,
    /// Explicit algebraic root relations.
    AlgebraicRoots,
    /// A checked recursive radical presentation.
    Radicals,
    /// An exact parameter image.
    ExactFamily,
    /// A local germ with explicit neighborhood scope.
    LocalBranch,
}

/// `ExclusionPolicy` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExclusionPolicy {
    /// Retain all original definedness exclusions.
    PreserveOriginal,
}

/// `RegularityKind` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegularityKind {
    /// Continuously differentiable on a real neighborhood.
    RealC1,
    /// Holomorphic on a complex neighborhood.
    Holomorphic,
}

/// `IdentityMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityMode {
    /// Normalize an exact difference.
    Normalize,
    /// Check a finite ideal combination.
    RelationCombination,
}

/// `AdjunctionMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdjunctionMode {
    /// A named algebraic closure.
    AlgebraicClosure,
    /// The fundamental theorem of algebra over C.
    ComplexFTA,
    /// The primitive-root-of-unity theorem.
    PrimitiveUnity,
    /// An odd real root.
    RealOddRoot,
    /// The positive real square root.
    PositiveRealSquareRoot,
}

/// `PredicateSplitMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredicateSplitMode {
    /// A defined scalar zero/nonzero partition.
    ZeroNonzero,
    /// A real negative/zero/positive partition.
    RealSigns,
    /// Eliminate an earlier exact cover.
    EliminateCover,
}

/// `BinomialMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinomialMode {
    /// Prove exact multiplicative order.
    Order,
    /// Prove complete binomial coverage.
    Coverage,
    /// Prove the zero-radicand case.
    ZeroRadicand,
}

/// `RefinementMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefinementMode {
    /// Prove compatible selection existence.
    Existence,
    /// Prove selection uniqueness separately from existence.
    Unique,
    /// Check the requested presentation.
    Presentation,
}

/// `RefinementTargetKind` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefinementTargetKind {
    /// A root selection.
    Selection,
    /// An exact solution set.
    SolutionSet,
}

/// `MultiplicityMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiplicityMode {
    /// Check derivative values.
    Derivatives,
    /// Check square-free gcd layers.
    SquareFreeLayers,
}

/// `LocalMode` discriminator with a stable explicit encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalMode {
    /// Check polynomial regularity.
    PolynomialRegularity,
    /// Check an open product neighborhood.
    OpenNeighborhood,
    /// Apply the local existence theorem.
    ImplicitExistence,
    /// Check the implicit derivative.
    Derivative,
}

/// One declared coordinate in an explicitly ordered product.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coordinate {
    /// Stable declaration key.
    pub key: String,
    /// Declared scalar field.
    pub field: Field,
    /// Coefficient, unknown or family role.
    pub role: CoordinateRole,
}

/// An exact rational interval; endpoints are not implicit root assertions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalInterval {
    /// Explicit left.
    pub left: Rational,
    /// Explicit right.
    pub right: Rational,
}

/// An ordered finite product, including the empty product.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateSpaceData<R = Reference> {
    /// Exact semantic header.
    pub header: R,
    /// Ordered coordinate declarations or their product.
    pub coordinates: Vec<Coordinate>,
}

/// Ordered scalar entries in an explicit product space.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TupleData<R = Reference> {
    /// Exact semantic header.
    pub header: R,
    /// Ordered outer binder frames.
    pub scope: Scope,
    /// Ordered coordinate declarations or their product.
    pub coordinates: R,
    /// Ordered entries entries.
    pub entries: Vec<R>,
}

/// A proposed branch set with no constructor completeness flag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolutionSetData<R = Reference> {
    /// Original solve problem.
    pub problem: R,
    /// Ordered branches entries.
    pub branches: Vec<R>,
}

/// A family binder and its capture-free tuple body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyMapData<R = Reference> {
    /// Exact semantic header.
    pub header: R,
    /// Ordered outer binder frames.
    pub scope: Scope,
    /// Explicit coefficients.
    pub coefficients: R,
    /// Explicit family.
    pub family: R,
    /// Exact coefficient region.
    pub region: R,
    /// Explicit target.
    pub target: R,
    /// Explicit body.
    pub body: R,
}

/// A sparse monomial with a nonzero coefficient.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolynomialTerm<R = Reference> {
    /// Ordered exponents entries.
    pub exponents: Vec<u32>,
    /// Explicit coefficient.
    pub coefficient: R,
}

/// A canonical sparse polynomial with ordered bound variable slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolynomialData<R = Reference> {
    /// Exact domain.
    pub domain: R,
    /// Ordered variables entries.
    pub variables: Vec<Field>,
    /// Ordered terms entries.
    pub terms: Vec<PolynomialTerm<R>>,
}

/// A monic extension relation depending only on its parent algebra.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionLevelData<R = Reference> {
    /// Explicit parent.
    pub parent: R,
    /// Explicit polynomial.
    pub polynomial: R,
    /// Ordered generator entries.
    pub generator: Vec<AtomicSort>,
    /// Explicit square free.
    pub square_free: SquareFreeRequest,
}

/// A proposed map with a finite localization and no implied injectivity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoefficientMapData<R = Reference> {
    /// Explicit source.
    pub source: R,
    /// Explicit target.
    pub target: MapTarget<R>,
    /// Closed schema mode.
    pub mode: MapMode,
    /// Exact coefficient region.
    pub region: R,
    /// Ordered images entries.
    pub images: Vec<R>,
    /// Original ordered denominators, or the explicitly specified finite localization.
    pub denominators: Vec<R>,
}

/// A root equation with an explicit coefficient specialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootRelationData<R = Reference> {
    /// Exact semantic header.
    pub header: R,
    /// Ordered outer binder frames.
    pub scope: Scope,
    /// Explicit ambient.
    pub ambient: Field,
    /// Explicit polynomial.
    pub polynomial: R,
    /// Ordered binder entries.
    pub binder: Vec<AtomicSort>,
    /// Explicit map.
    pub map: R,
    /// Exact coefficient region.
    pub region: R,
    /// Requested multiplicity, without a truth assertion.
    pub multiplicity: Option<u32>,
}

/// Ordered root slots with a jointly scoped selection predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootSelectionData<R = Reference> {
    /// Exact semantic header.
    pub header: R,
    /// Ordered outer binder frames.
    pub scope: Scope,
    /// Ordered relations entries.
    pub relations: Vec<R>,
    /// Original or jointly scoped predicate.
    pub predicate: R,
    /// Closed schema mode.
    pub mode: SelectionMode,
    /// Explicit convention.
    pub convention: Option<String>,
    /// Explicit interval.
    pub interval: Option<RationalInterval>,
}

/// A proposed proper factor split with both restriction maps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentSplitData<R = Reference> {
    /// Explicit parent.
    pub parent: R,
    /// Explicit left factor.
    pub left_factor: R,
    /// Explicit right factor.
    pub right_factor: R,
    /// Explicit left domain.
    pub left_domain: R,
    /// Explicit right domain.
    pub right_domain: R,
    /// Explicit left map.
    pub left_map: R,
    /// Explicit right map.
    pub right_map: R,
    /// Exact coefficient region.
    pub region: R,
}

/// A coefficient value with an explicit algebra dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoefficientValueObject<R = Reference> {
    /// Exact domain.
    pub domain: R,
    /// Explicit value.
    pub value: CoefficientValueData<R>,
}

/// A solution proposal retaining both coefficient and solution regions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolutionBranchData<R = Reference> {
    /// Original solve problem.
    pub problem: R,
    /// Exact coefficient region.
    pub region: R,
    /// Exact domain.
    pub domain: R,
    /// Explicit kind.
    pub kind: SolutionBranchKind<R>,
}

/// The original predicate, ordered unknowns and strict original domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveProblemData<R = Reference> {
    /// Exact semantic header.
    pub header: R,
    /// Ordered outer binder frames.
    pub scope: Scope,
    /// Original or jointly scoped predicate.
    pub predicate: R,
    /// Explicit coefficients.
    pub coefficients: R,
    /// Explicit unknowns.
    pub unknowns: R,
    /// Exact coefficient region.
    pub region: R,
    /// Exact domain.
    pub domain: R,
    /// Explicit coefficient domain.
    pub coefficient_domain: R,
    /// Explicit ambient.
    pub ambient: Field,
    /// Original ordered denominators, or the explicitly specified finite localization.
    pub denominators: Vec<R>,
}

/// A proposed local germ with exact base and Jacobian witnesses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBranchData<R = Reference> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit base coefficients.
    pub base_coefficients: R,
    /// Explicit base unknowns.
    pub base_unknowns: R,
    /// Explicit neighborhood.
    pub neighborhood: Neighborhood<R>,
    /// Explicit family symbol.
    pub family_symbol: Declaration,
    /// Explicit jacobian.
    pub jacobian: R,
    /// Explicit inverse.
    pub inverse: R,
}

/// One exact isolation interval and its independently checked counts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsolationInterval {
    /// Explicit left.
    pub left: Rational,
    /// Explicit right.
    pub right: Rational,
    /// Explicit left variations.
    pub left_variations: u32,
    /// Explicit right variations.
    pub right_variations: u32,
    /// Explicit count.
    pub count: u32,
}

/// An ordered polynomial multiple in a relation combination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationMultiple<R = Reference> {
    /// Explicit multiplier.
    pub multiplier: R,
    /// Explicit relation.
    pub relation: R,
    /// Optional independently matched scoped equality premise and ordered root interpretation.
    pub hypothesis: Option<ScopedCoefficientRelation<R>>,
}

/// An exact scoped equality interpreted at explicitly ordered formal root slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedCoefficientRelation<R = Reference> {
    /// Earlier application slot containing the exact equality predicate fact.
    pub premise: u32,
    /// Polynomial independently matched to the hypothesis's left endpoint.
    pub left: R,
    /// Polynomial independently matched to the hypothesis's right endpoint.
    pub right: R,
    /// Scalar root terms in formal polynomial slot order, without name inference.
    pub roots: Vec<R>,
    /// Optional exact root relation anchoring a complete existential Vieta discharge.
    pub vieta: Option<R>,
}

/// One simultaneous declaration substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveSubstitution<R = Reference> {
    /// Stable declaration key.
    pub key: String,
    /// Explicit expression.
    pub expression: R,
}

/// A positive prime power in a finite factor certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimePower {
    /// Explicit prime.
    pub prime: u32,
    /// Explicit exponent.
    pub exponent: u32,
}

/// One ordered root factor with a positive multiplicity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootFactor<R = Reference> {
    /// Explicit root.
    pub root: R,
    /// Requested multiplicity, without a truth assertion.
    pub multiplicity: u32,
}

/// An ordered generator image and its proposed conjugate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratorPair<R = Reference> {
    /// Explicit original.
    pub original: R,
    /// Explicit conjugate.
    pub conjugate: R,
}

/// Typed `Member` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementMember<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit tuple.
    pub tuple: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `Equivalent` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementEquivalent<R = Handle> {
    /// Explicit source.
    pub source: R,
    /// Explicit target.
    pub target: R,
    /// Explicit forward.
    pub forward: R,
    /// Explicit backward.
    pub backward: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `Coverage` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementCoverage<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit set.
    pub set: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `FamilyMembership` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementFamilyMembership<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit family.
    pub family: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `FamilyCoverage` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementFamilyCoverage<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit family.
    pub family: R,
    /// Explicit recovery.
    pub recovery: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `Rank` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementRank<R = Handle> {
    /// Explicit matrix.
    pub matrix: R,
    /// Exact domain.
    pub domain: R,
    /// Explicit rank.
    pub rank: u32,
    /// Ordered pivots entries.
    pub pivots: Vec<u32>,
    /// Explicit minor.
    pub minor: R,
    /// Explicit elimination.
    pub elimination: R,
}

/// Typed `LinearSystem` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementLinearSystem<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit reduced.
    pub reduced: R,
    /// Explicit rank.
    pub rank: u32,
    /// Explicit augmented rank.
    pub augmented_rank: u32,
    /// Exact coefficient region.
    pub region: R,
    /// Ordered free columns entries.
    pub free_columns: Vec<u32>,
}

/// Typed `LocalBranch` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementLocalBranch<R = Handle> {
    /// Explicit branch.
    pub branch: R,
    /// Explicit regularity.
    pub regularity: RegularityKind,
    /// Explicit neighborhood.
    pub neighborhood: Neighborhood<R>,
}

/// Typed `LocalDerivative` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementLocalDerivative<R = Handle> {
    /// Explicit branch.
    pub branch: R,
    /// Explicit derivative.
    pub derivative: R,
    /// Explicit neighborhood.
    pub neighborhood: Neighborhood<R>,
}

/// Typed `CoefficientEquality` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementCoefficientEquality<R = Handle> {
    /// Explicit left.
    pub left: R,
    /// Explicit right.
    pub right: R,
    /// Explicit map.
    pub map: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `Unit` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementUnit<R = Handle> {
    /// Explicit value.
    pub value: R,
    /// Explicit inverse.
    pub inverse: R,
    /// Exact domain.
    pub domain: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `ComponentCover` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementComponentCover<R = Handle> {
    /// Explicit split.
    pub split: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `RootExistence` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementRootExistence<R = Handle> {
    /// Explicit relation.
    pub relation: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `SelectionUnique` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementSelectionUnique<R = Handle> {
    /// Explicit selection.
    pub selection: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `RootOrder` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementRootOrder<R = Handle> {
    /// Explicit value.
    pub value: R,
    /// Explicit order.
    pub order: u32,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `Orbit` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementOrbit<R = Handle> {
    /// Explicit seed.
    pub seed: R,
    /// Ordered generators entries.
    pub generators: Vec<Vec<u32>>,
    /// Ordered members entries.
    pub members: Vec<R>,
    /// Ordered actions entries.
    pub actions: Vec<Vec<u32>>,
}

/// Typed `Multiplicity` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementMultiplicity<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit selection.
    pub selection: R,
    /// Ordered multiplicities entries.
    pub multiplicities: Vec<u32>,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `RealValues` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementRealValues<R = Handle> {
    /// Ordered values entries.
    pub values: Vec<R>,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `Isolation` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementIsolation<R = Handle> {
    /// Explicit relation.
    pub relation: R,
    /// Ordered intervals entries.
    pub intervals: Vec<IsolationInterval>,
    /// Explicit exterior count.
    pub exterior_count: u32,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `Presentation` statement payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementPresentation<R = Handle> {
    /// Explicit set.
    pub set: R,
    /// Requested exact representation.
    pub form: SolveForm,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `CoefficientIdentity` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateCoefficientIdentity<R = Handle> {
    /// Closed schema mode.
    pub mode: IdentityMode,
    /// Explicit left.
    pub left: R,
    /// Explicit right.
    pub right: R,
    /// Explicit map.
    pub map: R,
    /// Exact coefficient region.
    pub region: R,
    /// Ordered combinations entries.
    pub combinations: Vec<RelationMultiple<R>>,
    /// Original ordered denominators, or the explicitly specified finite localization.
    pub denominators: Vec<R>,
}

/// Typed `RootAdjunction` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRootAdjunction<R = Handle> {
    /// Closed schema mode.
    pub mode: AdjunctionMode,
    /// Explicit relation.
    pub relation: R,
    /// Explicit selection.
    pub selection: Option<R>,
    /// Explicit leading unit.
    pub leading_unit: u32,
    /// Explicit order.
    pub order: Option<u32>,
    /// Ordered splitting chain entries.
    pub splitting_chain: Vec<R>,
}

/// Typed `ExtensionUnit` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateExtensionUnit<R = Handle> {
    /// Explicit element.
    pub element: R,
    /// Explicit modulus.
    pub modulus: R,
    /// Explicit gcd.
    pub gcd: R,
    /// Ordered quotients entries.
    pub quotients: Vec<R>,
    /// Ordered bezout entries.
    pub bezout: Vec<R>,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `ComponentSplit` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateComponentSplit<R = Handle> {
    /// Explicit split.
    pub split: R,
    /// Explicit product.
    pub product: u32,
    /// Ordered bezout entries.
    pub bezout: Vec<R>,
    /// Explicit square free.
    pub square_free: u32,
    /// Ordered selections entries.
    pub selections: Vec<R>,
}

/// Typed `Membership` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateMembership<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit tuple.
    pub tuple: R,
    /// Exact coefficient region.
    pub region: R,
    /// Ordered substitutions entries.
    pub substitutions: Vec<SolveSubstitution<R>>,
    /// Ordered definedness entries.
    pub definedness: Vec<u32>,
}

/// Typed `SolutionTransport` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateSolutionTransport<R = Handle> {
    /// Explicit source.
    pub source: R,
    /// Explicit target.
    pub target: R,
    /// Explicit forward.
    pub forward: R,
    /// Explicit backward.
    pub backward: R,
    /// Exact coefficient region.
    pub region: R,
    /// Ordered definedness entries.
    pub definedness: Vec<u32>,
    /// Ordered equations entries.
    pub equations: Vec<u32>,
    /// Ordered forward inverse entries.
    pub forward_inverse: Vec<u32>,
    /// Ordered backward inverse entries.
    pub backward_inverse: Vec<u32>,
}

/// Typed `TotalPredicateSplit` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateTotalPredicateSplit<R = Handle> {
    /// Closed schema mode.
    pub mode: PredicateSplitMode,
    /// Explicit expression.
    pub expression: R,
    /// Exact coefficient region.
    pub region: R,
    /// Ordered children entries.
    pub children: Vec<R>,
    /// Ordered branches entries.
    pub branches: Vec<u32>,
    /// Explicit cover.
    pub cover: Option<u32>,
}

/// Typed `BinomialCoverage` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateBinomialCoverage<R = Handle> {
    /// Closed schema mode.
    pub mode: BinomialMode,
    /// Explicit order.
    pub order: u32,
    /// Explicit unity.
    pub unity: Option<R>,
    /// Explicit selection.
    pub selection: Option<R>,
    /// Ordered factors entries.
    pub factors: Vec<PrimePower>,
    /// Ordered exclusions entries.
    pub exclusions: Vec<u32>,
    /// Explicit product.
    pub product: Option<u32>,
}

/// Typed `RootSelectionRefinement` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRootSelectionRefinement<R = Handle> {
    /// Closed schema mode.
    pub mode: RefinementMode,
    /// Explicit target kind.
    pub target_kind: RefinementTargetKind,
    /// Explicit target.
    pub target: R,
    /// Ordered witnesses entries.
    pub witnesses: Vec<R>,
    /// Ordered compatibility entries.
    pub compatibility: Vec<u32>,
    /// Requested exact representation.
    pub form: Option<SolveForm>,
}

/// Typed `ExactLinearEquivalence` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateExactLinearEquivalence<R = Handle> {
    /// Explicit matrix.
    pub matrix: R,
    /// Explicit augmented columns.
    pub augmented_columns: u32,
    /// Ordered operations entries.
    pub operations: Vec<RowOperation<R>>,
    /// Ordered pivots entries.
    pub pivots: Vec<u32>,
    /// Explicit minor.
    pub minor: R,
    /// Explicit reduced.
    pub reduced: R,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `ProductCompleteness` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateProductCompleteness<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit set.
    pub set: R,
    /// Explicit scalar.
    pub scalar: R,
    /// Ordered factors entries.
    pub factors: Vec<RootFactor<R>>,
    /// Explicit product.
    pub product: u32,
    /// Ordered existence entries.
    pub existence: Vec<u32>,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `MultiplicityTransfer` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateMultiplicityTransfer<R = Handle> {
    /// Closed schema mode.
    pub mode: MultiplicityMode,
    /// Original solve problem.
    pub problem: R,
    /// Explicit selection.
    pub selection: R,
    /// Ordered multiplicities entries.
    pub multiplicities: Vec<u32>,
    /// Ordered values entries.
    pub values: Vec<R>,
    /// Explicit product.
    pub product: u32,
    /// Ordered nonzero entries.
    pub nonzero: Vec<u32>,
    /// Exact coefficient region.
    pub region: R,
}

/// Typed `ConjugationTransport` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateConjugationTransport<R = Handle> {
    /// Explicit map.
    pub map: R,
    /// Explicit conjugate map.
    pub conjugate_map: R,
    /// Ordered images entries.
    pub images: Vec<GeneratorPair<R>>,
    /// Ordered values entries.
    pub values: Vec<R>,
    /// Ordered relations entries.
    pub relations: Vec<u32>,
    /// Ordered fixed field entries.
    pub fixed_field: Vec<u32>,
    /// Exact coefficient region.
    pub region: R,
}

/// Ordered endpoints of an explicit real-to-complex transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestrictionPair<R = Handle> {
    /// Real-header endpoint, qualified by its actual graph identity.
    pub real: R,
    /// Complex-header endpoint, qualified by its actual graph identity.
    pub complex: R,
}

/// The exact source collection addressed by a condition transport row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestrictionConditionKind {
    /// A qualified used hypothesis; the index is its position in the premise's used hypotheses.
    Hypothesis,
    /// An original definedness or logical requirement, including discharged requirements.
    Requirement,
    /// An unresolved guarded obligation.
    Residual,
}

/// Transport one actually used condition, retaining it unless explicitly discharged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestrictionCondition<R = Handle> {
    /// Source application in the ordered premise vector.
    pub premise: u32,
    /// Source condition collection.
    pub kind: RestrictionConditionKind,
    /// Exact position in that collection.
    pub index: u32,
    /// Source and destination home contexts.
    pub contexts: RestrictionPair<R>,
    /// Source and destination regions.
    pub regions: RestrictionPair<R>,
    /// Source and destination predicates.
    pub predicates: RestrictionPair<R>,
    /// Optional proof of the transported predicate in the real context.
    pub discharge: Option<u32>,
}

/// Mandatory cross-header data; none of these correspondences assert a theorem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealRestrictionTransport<R = Handle> {
    /// Directed embedding from the real problem's coefficient domain to the complex domain.
    pub coefficient_map: R,
    /// Free expressions in the real coefficient-coordinate order.
    pub coefficients: Vec<RestrictionPair<R>>,
    /// Free expressions in the real unknown-coordinate order.
    pub unknowns: Vec<RestrictionPair<R>>,
    /// Overall region pair followed by the ordered coefficient strata.
    pub regions: Vec<RestrictionPair<R>>,
    /// Every used foreign hypothesis, requirement and residual has an explicit row.
    pub conditions: Vec<RestrictionCondition<R>>,
    /// Earlier cover of the exact real coefficient region by the listed strata.
    pub real_cover: u32,
}

/// Typed `RealRestriction` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRealRestriction<R = Handle> {
    /// Explicit coverage.
    pub coverage: u32,
    /// Original solve problem.
    pub problem: R,
    /// Explicit set.
    pub set: R,
    /// Explicit ordered coefficient, declaration, region and condition transport.
    pub transport: RealRestrictionTransport<R>,
    /// Ordered retained entries.
    pub retained: Vec<R>,
    /// Ordered rejected entries.
    pub rejected: Vec<R>,
    /// Ordered classification entries.
    pub classification: Vec<u32>,
    /// Explicit cover.
    pub cover: u32,
}

/// Typed `PermutationOrbitCoverage` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificatePermutationOrbitCoverage<R = Handle> {
    /// Explicit seed.
    pub seed: R,
    /// Explicit degree.
    pub degree: u32,
    /// Ordered generators entries.
    pub generators: Vec<Vec<u32>>,
    /// Ordered members entries.
    pub members: Vec<R>,
    /// Ordered actions entries.
    pub actions: Vec<Vec<u32>>,
    /// Ordered words entries.
    pub words: Vec<Vec<u32>>,
}

/// Typed `RealIsolationAndOrder` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRealIsolationAndOrder<R = Handle> {
    /// Explicit relation.
    pub relation: R,
    /// Ordered sturm entries.
    pub sturm: Vec<R>,
    /// Ordered intervals entries.
    pub intervals: Vec<IsolationInterval>,
    /// Explicit exterior left.
    pub exterior_left: u32,
    /// Explicit exterior right.
    pub exterior_right: u32,
    /// Requested multiplicity, without a truth assertion.
    pub multiplicity: Vec<u32>,
    /// Explicit ordered index.
    pub ordered_index: Option<u32>,
}

/// Typed `ParameterImageCoverage` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateParameterImageCoverage<R = Handle> {
    /// Original solve problem.
    pub problem: R,
    /// Explicit family.
    pub family: R,
    /// Explicit recovery.
    pub recovery: Option<R>,
    /// Exact coefficient region.
    pub region: R,
    /// Ordered membership entries.
    pub membership: Vec<u32>,
    /// Ordered recovery proofs entries.
    pub recovery_proofs: Vec<u32>,
    /// Exact domain.
    pub domain: Vec<u32>,
    /// Ordered outer binder frames.
    pub scope: Vec<u32>,
}

/// Typed `ImplicitFunctionBranch` certificate payload; its fields are untrusted until replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateImplicitFunctionBranch<R = Handle> {
    /// Closed schema mode.
    pub mode: LocalMode,
    /// Explicit branch.
    pub branch: R,
    /// Explicit regularity.
    pub regularity: RegularityKind,
    /// Ordered regularity proofs entries.
    pub regularity_proofs: Vec<u32>,
    /// Ordered base equations entries.
    pub base_equations: Vec<u32>,
    /// Ordered inverse entries.
    pub inverse: Vec<u32>,
    /// Ordered derivatives entries.
    pub derivatives: Vec<u32>,
    /// Explicit derivative.
    pub derivative: Option<R>,
}

/// Exact coefficient algebra syntax; a quotient is not automatically a field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoefficientDomainData<R = Reference> {
    /// Rational syntax; admission does not prove its mathematical claims.
    Rational,
    /// Gaussian syntax; admission does not prove its mathematical claims.
    Gaussian,
    /// Fraction syntax; admission does not prove its mathematical claims.
    Fraction {
        /// Explicit base.
        base: R,
        /// Ordered coordinate declarations or their product.
        coordinates: R,
    },
    /// Quotient syntax; admission does not prove its mathematical claims.
    Quotient {
        /// Explicit level.
        level: R,
    },
}

/// The explicitly typed codomain of a coefficient map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapTarget<R = Reference> {
    /// `CoefficientDomain` syntax; admission does not prove its mathematical claims.
    CoefficientDomain {
        /// Exact domain.
        domain: R,
    },
    /// `ScalarAmbient` syntax; admission does not prove its mathematical claims.
    ScalarAmbient {
        /// Exact semantic header.
        header: R,
        /// Declared scalar field.
        field: Field,
    },
}

/// Canonical coefficient payload in its declared algebra.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoefficientValueData<R = Reference> {
    /// Rational syntax; admission does not prove its mathematical claims.
    Rational {
        /// Explicit value.
        value: Rational,
    },
    /// Gaussian syntax; admission does not prove its mathematical claims.
    Gaussian {
        /// Explicit real.
        real: Rational,
        /// Explicit imaginary.
        imaginary: Rational,
    },
    /// Fraction syntax; admission does not prove its mathematical claims.
    Fraction {
        /// Explicit numerator.
        numerator: R,
        /// Explicit denominator.
        denominator: R,
    },
    /// Tower syntax; admission does not prove its mathematical claims.
    Tower {
        /// Ordered coefficients entries.
        coefficients: Vec<R>,
    },
}

/// A branch representation with no implicit coverage theorem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionBranchKind<R = Reference> {
    /// Finite syntax; admission does not prove its mathematical claims.
    Finite {
        /// Ordered tuples entries.
        tuples: Vec<R>,
    },
    /// Image syntax; admission does not prove its mathematical claims.
    Image {
        /// Explicit family.
        family: R,
    },
    /// Algebraic syntax; admission does not prove its mathematical claims.
    Algebraic {
        /// Explicit selection.
        selection: R,
    },
    /// Empty syntax; admission does not prove its mathematical claims.
    Empty,
    /// Local syntax; admission does not prove its mathematical claims.
    Local {
        /// Explicit branch.
        branch: R,
    },
}

/// An existential neighborhood cannot be evaluated as an explicit region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Neighborhood<R = Reference> {
    /// Existential syntax; admission does not prove its mathematical claims.
    Existential {
        /// Ordered coefficient binder entries.
        coefficient_binder: Vec<AtomicSort>,
        /// Ordered unknown binder entries.
        unknown_binder: Vec<AtomicSort>,
    },
    /// Explicit syntax; admission does not prove its mathematical claims.
    Explicit {
        /// Explicit coefficient region.
        coefficient_region: R,
        /// Explicit unknown region.
        unknown_region: R,
    },
}

/// A reversible exact row operation; premise slots are not truth flags.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RowOperation<R = Reference> {
    /// Swap syntax; admission does not prove its mathematical claims.
    Swap {
        /// Explicit left.
        left: u32,
        /// Explicit right.
        right: u32,
    },
    /// Scale syntax; admission does not prove its mathematical claims.
    Scale {
        /// Explicit row.
        row: u32,
        /// Explicit value.
        value: R,
        /// Explicit unit.
        unit: u32,
    },
    /// `AddMultiple` syntax; admission does not prove its mathematical claims.
    AddMultiple {
        /// Explicit destination.
        destination: u32,
        /// Explicit source.
        source: u32,
        /// Explicit value.
        value: R,
    },
}

/// Closed typed `SolveDraft` vocabulary; construction grants no proof authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolveDraft<R = Reference> {
    /// Typed `CoordinateSpace` payload.
    CoordinateSpace(CoordinateSpaceData<R>),
    /// Typed `Tuple` payload.
    Tuple(TupleData<R>),
    /// Typed `SolutionSet` payload.
    SolutionSet(SolutionSetData<R>),
    /// Typed `FamilyMap` payload.
    FamilyMap(FamilyMapData<R>),
    /// Typed `CoefficientDomain` payload.
    CoefficientDomain(CoefficientDomainData<R>),
    /// Typed `Polynomial` payload.
    Polynomial(PolynomialData<R>),
    /// Typed `ExtensionLevel` payload.
    ExtensionLevel(ExtensionLevelData<R>),
    /// Typed `CoefficientMap` payload.
    CoefficientMap(CoefficientMapData<R>),
    /// Typed `RootRelation` payload.
    RootRelation(RootRelationData<R>),
    /// Typed `RootSelection` payload.
    RootSelection(RootSelectionData<R>),
    /// Typed `ComponentSplit` payload.
    ComponentSplit(ComponentSplitData<R>),
    /// Typed `CoefficientValue` payload.
    CoefficientValue(CoefficientValueObject<R>),
    /// Typed `SolutionBranch` payload.
    SolutionBranch(SolutionBranchData<R>),
    /// Typed `SolveProblem` payload.
    SolveProblem(SolveProblemData<R>),
    /// Typed `LocalBranch` payload.
    LocalBranch(LocalBranchData<R>),
}

/// Closed typed `SolveStatement` vocabulary; construction grants no proof authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolveStatement<R = Handle> {
    /// Typed `Member` payload.
    Member(StatementMember<R>),
    /// Typed `Equivalent` payload.
    Equivalent(StatementEquivalent<R>),
    /// Typed `Coverage` payload.
    Coverage(StatementCoverage<R>),
    /// Typed `FamilyMembership` payload.
    FamilyMembership(StatementFamilyMembership<R>),
    /// Typed `FamilyCoverage` payload.
    FamilyCoverage(StatementFamilyCoverage<R>),
    /// Typed `Rank` payload.
    Rank(StatementRank<R>),
    /// Typed `LinearSystem` payload.
    LinearSystem(StatementLinearSystem<R>),
    /// Typed `LocalBranch` payload.
    LocalBranch(StatementLocalBranch<R>),
    /// Typed `LocalDerivative` payload.
    LocalDerivative(StatementLocalDerivative<R>),
    /// Typed `CoefficientEquality` payload.
    CoefficientEquality(StatementCoefficientEquality<R>),
    /// Typed `Unit` payload.
    Unit(StatementUnit<R>),
    /// Typed `ComponentCover` payload.
    ComponentCover(StatementComponentCover<R>),
    /// Typed `RootExistence` payload.
    RootExistence(StatementRootExistence<R>),
    /// Typed `SelectionUnique` payload.
    SelectionUnique(StatementSelectionUnique<R>),
    /// Typed `RootOrder` payload.
    RootOrder(StatementRootOrder<R>),
    /// Typed `Orbit` payload.
    Orbit(StatementOrbit<R>),
    /// Typed `Multiplicity` payload.
    Multiplicity(StatementMultiplicity<R>),
    /// Typed `RealValues` payload.
    RealValues(StatementRealValues<R>),
    /// Typed `Isolation` payload.
    Isolation(StatementIsolation<R>),
    /// Typed `Presentation` payload.
    Presentation(StatementPresentation<R>),
}

/// Closed typed `SolveCertificate` vocabulary; construction grants no proof authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolveCertificate<R = Handle> {
    /// Typed `CoefficientIdentity` payload.
    CoefficientIdentity(CertificateCoefficientIdentity<R>),
    /// Typed `RootAdjunction` payload.
    RootAdjunction(CertificateRootAdjunction<R>),
    /// Typed `ExtensionUnit` payload.
    ExtensionUnit(CertificateExtensionUnit<R>),
    /// Typed `ComponentSplit` payload.
    ComponentSplit(CertificateComponentSplit<R>),
    /// Typed `Membership` payload.
    Membership(CertificateMembership<R>),
    /// Typed `SolutionTransport` payload.
    SolutionTransport(CertificateSolutionTransport<R>),
    /// Typed `TotalPredicateSplit` payload.
    TotalPredicateSplit(CertificateTotalPredicateSplit<R>),
    /// Typed `BinomialCoverage` payload.
    BinomialCoverage(CertificateBinomialCoverage<R>),
    /// Typed `RootSelectionRefinement` payload.
    RootSelectionRefinement(CertificateRootSelectionRefinement<R>),
    /// Typed `ExactLinearEquivalence` payload.
    ExactLinearEquivalence(CertificateExactLinearEquivalence<R>),
    /// Typed `ProductCompleteness` payload.
    ProductCompleteness(CertificateProductCompleteness<R>),
    /// Typed `MultiplicityTransfer` payload.
    MultiplicityTransfer(CertificateMultiplicityTransfer<R>),
    /// Typed `ConjugationTransport` payload.
    ConjugationTransport(CertificateConjugationTransport<R>),
    /// Typed `RealRestriction` payload.
    RealRestriction(CertificateRealRestriction<R>),
    /// Typed `PermutationOrbitCoverage` payload.
    PermutationOrbitCoverage(CertificatePermutationOrbitCoverage<R>),
    /// Typed `RealIsolationAndOrder` payload.
    RealIsolationAndOrder(CertificateRealIsolationAndOrder<R>),
    /// Typed `ParameterImageCoverage` payload.
    ParameterImageCoverage(CertificateParameterImageCoverage<R>),
    /// Typed `ImplicitFunctionBranch` payload.
    ImplicitFunctionBranch(CertificateImplicitFunctionBranch<R>),
}

/// A borrowed graph-owned solving payload; all links are checked handles.
#[derive(Clone, Copy, Debug)]
pub enum SolveView<'a> {
    /// Borrowed `CoordinateSpace` payload.
    CoordinateSpace(&'a CoordinateSpaceData<Handle>),
    /// Borrowed `Tuple` payload.
    Tuple(&'a TupleData<Handle>),
    /// Borrowed `SolutionSet` payload.
    SolutionSet(&'a SolutionSetData<Handle>),
    /// Borrowed `FamilyMap` payload.
    FamilyMap(&'a FamilyMapData<Handle>),
    /// Borrowed `CoefficientDomain` payload.
    CoefficientDomain(&'a CoefficientDomainData<Handle>),
    /// Borrowed `Polynomial` payload.
    Polynomial(&'a PolynomialData<Handle>),
    /// Borrowed `ExtensionLevel` payload.
    ExtensionLevel(&'a ExtensionLevelData<Handle>),
    /// Borrowed `CoefficientMap` payload.
    CoefficientMap(&'a CoefficientMapData<Handle>),
    /// Borrowed `RootRelation` payload.
    RootRelation(&'a RootRelationData<Handle>),
    /// Borrowed `RootSelection` payload.
    RootSelection(&'a RootSelectionData<Handle>),
    /// Borrowed `ComponentSplit` payload.
    ComponentSplit(&'a ComponentSplitData<Handle>),
    /// Borrowed `CoefficientValue` payload.
    CoefficientValue(&'a CoefficientValueObject<Handle>),
    /// Borrowed `SolutionBranch` payload.
    SolutionBranch(&'a SolutionBranchData<Handle>),
    /// Borrowed `SolveProblem` payload.
    SolveProblem(&'a SolveProblemData<Handle>),
    /// Borrowed `LocalBranch` payload.
    LocalBranch(&'a LocalBranchData<Handle>),
}

/// A graph-owned problem and exact requested strength and presentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveSpec {
    /// Original graph-owned problem.
    pub problem: Handle,
    /// Candidate or complete coverage goal.
    pub goal: SolveGoal,
    /// Requested representation.
    pub form: SolveForm,
    /// Original strict-domain policy.
    pub exclusions: ExclusionPolicy,
}

/// A missing proof, rather than a mathematical contradiction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveReason {
    /// Missing `CoefficientDomainUnavailable` capability or evidence.
    CoefficientDomainUnavailable,
    /// Missing `PolynomialFragment` capability or evidence.
    PolynomialFragment,
    /// Missing `GeneralNonlinearSystem` capability or evidence.
    GeneralNonlinearSystem,
    /// Missing `MissingUnitProof` capability or evidence.
    MissingUnitProof,
    /// Missing `ComponentChoicePending` capability or evidence.
    ComponentChoicePending,
    /// Missing `SpecializationGuardPending` capability or evidence.
    SpecializationGuardPending,
    /// Missing `RootSelectionExistencePending` capability or evidence.
    RootSelectionExistencePending,
    /// Missing `RootSelectionUniquenessPending` capability or evidence.
    RootSelectionUniquenessPending,
    /// Missing `CoveragePending` capability or evidence.
    CoveragePending,
    /// Missing `RadicalFormUnavailable` capability or evidence.
    RadicalFormUnavailable,
    /// Missing `GenericRadicalRouteUnavailable` capability or evidence.
    GenericRadicalRouteUnavailable,
    /// Missing `RealIsolationUnavailable` capability or evidence.
    RealIsolationUnavailable,
    /// Missing `RealOrderingRequired` capability or evidence.
    RealOrderingRequired,
    /// Missing `IrreducibilityUnproved` capability or evidence.
    IrreducibilityUnproved,
    /// Missing `RegularityPremisePending` capability or evidence.
    RegularityPremisePending,
    /// Missing `LocalNeighborhoodPending` capability or evidence.
    LocalNeighborhoodPending,
    /// Missing `ConditionalCoverage` capability or evidence.
    ConditionalCoverage,
}

/// A bounded graph-based description of an unresolved proof obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveBlocker {
    /// Closed reason for the missing proof.
    pub reason: SolveReason,
    /// Relevant objects in semantic order.
    pub objects: Vec<Handle>,
    /// Region where this obligation remains open.
    pub region: Option<Handle>,
    /// Full guarded obligation, if one has been constructed.
    pub obligation: Option<crate::proof_model::GuardedObligation>,
}

/// Checked facts about an exact proposal; only a coverage witness proves completeness.
#[derive(Clone, Debug)]
pub struct SolveAnswer {
    /// Proposed solution set.
    pub set: Handle,
    /// Independently replayed facts.
    pub facts: Vec<crate::proof_model::VerifiedWitness>,
    /// Coverage of this exact problem, set and domain.
    pub coverage: Option<crate::proof_model::VerifiedWitness>,
    /// Presentation supported by the facts.
    pub form: SolveForm,
    /// Regions whose requested theorem remains unproved.
    pub unresolved_regions: Vec<Handle>,
}
