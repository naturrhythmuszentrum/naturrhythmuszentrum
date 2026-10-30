// SPDX-License-Identifier: AGPL-3.0-only
//! Append-only heterogeneous graph storage and atomic batch admission.
//!
//! # Atomic construction and rollback
//!
//! `Cas::admit` validates flat child-before-parent drafts and publishes them atomically. Handles carry a private store identity; public logical identifiers never authorize a lookup in another store. Graph-owned expression records share exact syntax, while each original operand use keeps a distinct occurrence, optional checked UTF-8 byte span and ordered child links. Sources with conflicting bytes under one key are refused. Failed admission preserves the published graph and all next identifiers.
//!
//! Draft admission, proof checking and replay, and producer transactions use one append-only rollback journal instead of copying the accumulated store. Each checkpoint restores node and edge indices, logical indexes and next identifiers, interning keys, policy identities, source keys, parent claims and all payload counters on refusal. An inner refusal undoes only its edits; an inner commit remains reversible by its enclosing transaction. Work covers touched payload, a fixed 256-unit counter checkpoint, logical key probes and validation, without a transaction charge proportional to unrelated records. Storage checks still include retained occupancy, pending drafts and journal reservation; nested transaction checkpoints reserve their bounded counter inventory under the existing staged-payload ceiling. Spent work is never refunded. Earlier witness bytes and future logical identifiers survive invalid input and mid-batch exhaustion unchanged. Full store copies are available only to tests needing an untouched control. Copying a checked application charges its own typed payload and optional scalar cache. Substitution prepays each reached record and copied scope, accumulating draft and memo workspace under the existing staged-payload limit; unrelated stored syntax contributes no work.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};

use crate::budget::{Budget, BudgetKind, Limits};
use crate::domain_model::{
    Boundary, BoundaryDraft, CaseArm, CaseArmDraft, ContextDraft, Header, HeaderDraft, Layer,
    Region, RegionDraft, Space, SpaceDraft,
};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::model::{Binding, Environment, Operator, PredicateKind, Scope, Sort, Span};
use crate::source_model::{SourceForm, SourceFormDraft};

mod intern;
mod lemmas;
pub(crate) use lemmas::{PendingJudgment, PiIndex, PiInventory, ScalarInventory};

pub(crate) use intern::InterningSnapshot;
use intern::{InternIndex, StoreToken};

/// A disjoint namespace for logical identifiers.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VertexKind {
    /// A store-local anonymous checked judgment.
    AnonymousLemma,
    /// Exact `ParameterBase` syntax.
    ParameterBase,
    /// Exact `SpaceFamily` syntax.
    SpaceFamily,
    /// Exact `FamilyExpression` syntax.
    FamilyExpression,
    /// Exact `FibrePiece` syntax.
    FibrePiece,
    /// Exact `RealCell` syntax.
    RealCell,
    /// Exact `CoefficientEvaluation` syntax.
    CoefficientEvaluation,
    /// Exact `ScalarSpace` syntax.
    ScalarSpace,
    /// Exact `SpacePlacement` syntax.
    SpacePlacement,
    /// Exact `ScalarPiece` syntax.
    ScalarPiece,
    /// Exact solving `CoordinateSpace` object.
    CoordinateSpace,
    /// Exact solving `Tuple` object.
    Tuple,
    /// Exact solving `SolutionSet` object.
    SolutionSet,
    /// Exact solving `FamilyMap` object.
    FamilyMap,
    /// Exact solving `CoefficientDomain` object.
    CoefficientDomain,
    /// Exact solving `Polynomial` object.
    Polynomial,
    /// Exact solving `ExtensionLevel` object.
    ExtensionLevel,
    /// Exact solving `CoefficientMap` object.
    CoefficientMap,
    /// Exact solving `RootRelation` object.
    RootRelation,
    /// Exact solving `RootSelection` object.
    RootSelection,
    /// Exact solving `ComponentSplit` object.
    ComponentSplit,
    /// Exact solving `CoefficientValue` object.
    CoefficientValue,
    /// Exact solving `SolutionBranch` object.
    SolutionBranch,
    /// Exact solving `SolveProblem` object.
    SolveProblem,
    /// Exact solving `LocalBranch` object.
    LocalBranch,

    /// Immutable source bytes.
    Source,
    /// Mathematical context and hypothesis declarations.
    Context,
    /// Typed mathematical predicate.
    Condition,
    /// Shared mathematical syntax.
    Expression,
    /// Original or generated occurrence.
    Occurrence,
    /// Binder-frame semantics.
    Binder,
    /// A first-class operation request.
    Request,
    /// A checked identity application, admitted only by the proof module.
    Application,
    /// A checked acyclic proof record, admitted only by the proof module.
    Proof,
    /// A complete immutable versioned logical-cost policy.
    Policy,
    /// Declared mathematical space.
    Space,
    /// Exact unproved set description.
    Region,
    /// Unordered region/value association.
    CaseArm,
    /// Shared boundary location.
    Boundary,
    /// Original ordered semantic form.
    SourceForm,
    /// Immutable acyclic semantic declarations.
    Header,
}

impl VertexKind {
    /// Stable encoding independent of Rust enum layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::AnonymousLemma => 40,
            Self::Source => 0,
            Self::Context => 1,
            Self::Condition => 2,
            Self::Expression => 3,
            Self::Occurrence => 4,
            Self::Binder => 5,
            Self::Request => 6,
            Self::Application => 7,
            Self::Proof => 8,
            Self::Policy => 9,
            Self::Space => 10,
            Self::Region => 11,
            Self::CaseArm => 12,
            Self::Boundary => 13,
            Self::SourceForm => 14,
            Self::Header => 15,
            Self::CoordinateSpace => 16,
            Self::Tuple => 17,
            Self::SolutionSet => 18,
            Self::FamilyMap => 19,
            Self::CoefficientDomain => 20,
            Self::Polynomial => 21,
            Self::ExtensionLevel => 22,
            Self::CoefficientMap => 23,
            Self::RootRelation => 24,
            Self::RootSelection => 25,
            Self::ComponentSplit => 26,
            Self::CoefficientValue => 27,
            Self::SolutionBranch => 28,
            Self::SolveProblem => 29,
            Self::LocalBranch => 30,
            Self::ParameterBase => 31,
            Self::SpaceFamily => 32,
            Self::FamilyExpression => 33,
            Self::FibrePiece => 34,
            Self::RealCell => 35,
            Self::CoefficientEvaluation => 36,
            Self::ScalarSpace => 37,
            Self::SpacePlacement => 38,
            Self::ScalarPiece => 39,
        }
    }
}

/// A deterministic identifier within one vertex-kind namespace.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalId {
    /// Vertex kind.
    pub kind: VertexKind,
    /// Consecutive admission serial, beginning with zero in each namespace.
    pub serial: u64,
}

/// An opaque store-scoped handle; callers cannot manufacture its store token.
#[derive(Clone)]
pub struct Handle {
    /// Shared runtime ownership identity.
    token: Arc<StoreToken>,
    /// Mathematical admission identifier.
    id: LogicalId,
}

impl Handle {
    /// Read a portable logical identifier, without conferring store ownership.
    #[must_use]
    pub const fn logical_id(&self) -> LogicalId {
        self.id
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Handle")
            .field(&self.id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Handle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.token, &other.token)
    }
}

impl Eq for Handle {}

/// A checked existing handle or an earlier index in the current draft batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reference {
    /// A live handle in the receiving store.
    Existing(Handle),
    /// A zero-based draft index, strictly earlier than its user.
    Local(u32),
}

/// A context hypothesis whose identity and predicate are both retained.
#[derive(Clone, Debug)]
pub struct AssumptionDraft {
    /// Caller hypothesis key.
    pub key: String,
    /// Earlier condition record.
    pub condition: Reference,
}

/// Untrusted ordered expression syntax.
#[derive(Clone, Debug)]
pub struct ExpressionDraft {
    /// Explicit typed space link; ordinary pointwise scalar values require absence.
    pub space: Option<Reference>,
    /// Operator and complete mathematical payload.
    pub operator: Operator,
    /// Claimed mathematical result sort, checked against operands.
    pub sort: Sort,
    /// Actual immutable semantic header, independent of assumptions.
    pub header: Reference,
    /// Explicit outer-to-inner binder frames.
    pub scope: Scope,
    /// Optional new frame for the designated body operands.
    pub binding: Option<Binding>,
    /// Ordered child expressions, each already admitted or earlier in the batch.
    pub operands: Vec<Reference>,
}

/// Untrusted source/provenance occurrence with a distinct identity for every use.
#[derive(Clone, Debug)]
pub struct OccurrenceDraft {
    /// Mathematical syntax denoted by this occurrence.
    pub expression: Reference,
    /// Original source, absent for generated occurrences.
    pub source: Option<Reference>,
    /// Optional checked source span; never present on generated occurrences.
    pub span: Option<Span>,
    /// Ordered child occurrences, exactly matching the expression operands.
    pub children: Vec<Reference>,
    /// Original expression/occurrence supplying generated provenance.
    pub provenance: Option<Reference>,
    /// Source binder display names, independent of de Bruijn identity.
    pub binder_names: Vec<String>,
}

/// One flat draft node. Arbitrary identity/proof nodes cannot be supplied here.
#[derive(Clone, Debug)]
pub enum Draft {
    /// Admit typed solving syntax without creating evidence.
    Solve(crate::solving::SolveDraft),
    /// A root slot selected jointly with all other slots.
    RootValue {
        /// Exact semantic header.
        header: Reference,
        /// Earlier joint selection.
        selection: Reference,
        /// Ordered root slot.
        slot: u32,
    },
    /// Embed an exact coefficient through an explicit checked map.
    ExactCoefficient {
        /// Exact semantic header.
        header: Reference,
        /// Earlier coefficient value.
        value: Reference,
        /// Earlier scalar-target coefficient map.
        map: Reference,
    },
    /// Retain immutable UTF-8 source bytes.
    Source {
        /// Caller source key.
        key: String,
        /// Exact source text.
        text: String,
    },
    /// Admit an immutable semantic header.
    Header(HeaderDraft),
    /// Admit a root or local hypothesis frame.
    Context(ContextDraft),
    /// Admit an exact region without proving any relation.
    Region(RegionDraft),
    /// A total family region with an exact earlier base projection.
    FamilyRegion {
        /// Ordinary exact region syntax.
        region: RegionDraft,
        /// Earlier family and original base stratum.
        lies_over: crate::solving::LiesOver<Reference>,
    },
    /// Admit a declared mathematical space.
    Space(SpaceDraft),
    /// Admit one region/value association.
    CaseArm(CaseArmDraft),
    /// Admit a boundary location without ownership facts.
    Boundary(BoundaryDraft),
    /// Retain an original ordered semantic signature.
    SourceForm(SourceFormDraft),
    /// Admit a typed predicate; this supplies no proof of its truth.
    Condition {
        /// Predicate vocabulary tag.
        kind: PredicateKind,
        /// Actual immutable semantic header.
        header: Reference,
        /// Explicit bound frames.
        scope: Scope,
        /// Ordered predicate expressions.
        arguments: Vec<Reference>,
    },
    /// Admit native case syntax without asserting coverage or compatibility.
    Cases(crate::domain_model::CasesDraft),
    /// Admit shared mathematical syntax.
    Expression(ExpressionDraft),
    /// Admit a distinct original or generated occurrence.
    Occurrence(OccurrenceDraft),
    /// Retain a typed task; this does not authorize execution without coverage receipts.
    Request {
        /// Requested mathematical operation.
        task: crate::task::Task,
        /// Immutable mathematical context.
        context: Reference,
        /// Ordered occurrence roots.
        inputs: Vec<Reference>,
        /// Earlier checked proof premises.
        premises: Vec<Reference>,
        /// Fully resolved request budgets.
        limits: Box<Limits>,
    },
}

/// An immutable source weight.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    /// Caller source identity.
    pub(crate) key: String,
    /// Exact retained UTF-8 text.
    pub(crate) text: String,
}

/// Canonical mathematical context weight.
#[derive(Clone, Debug)]
pub(crate) struct Context {
    /// Checked decision introduction, absent on caller-assumed frames.
    pub(crate) introduction: Option<LogicalId>,
    /// Actual header object, retained independently of its legacy view.
    pub(crate) header_id: LogicalId,
    /// Exact admissible region.
    pub(crate) admissible: LogicalId,
    /// Enclosing context for a local frame.
    pub(crate) parent: Option<LogicalId>,
    /// Supplied local restriction; membership is an explicit binder.
    pub(crate) restriction: Option<LogicalId>,
    /// Local predicate slots in caller order, excluding membership slot zero.
    pub(crate) frame: Vec<(String, LogicalId)>,
    /// Checked number of parent-frame edges.
    pub(crate) depth: u64,
    /// Sorted semantic declarations and policies.
    pub(crate) environment: Environment,
    /// Canonical header bytes, excluding assumptions.
    pub(crate) header: Vec<u8>,
    /// Hypothesis keys and condition IDs in canonical order.
    pub(crate) assumptions: Vec<(String, LogicalId)>,
    /// Flattened, sorted hypothesis predicates retaining their provenance keys.
    pub(crate) canonical_assumptions: Vec<u8>,
}

/// A predicate weight; truth is checked separately from syntax admission.
#[derive(Clone, Debug)]
pub(crate) struct Condition {
    /// Predicate tag.
    pub(crate) kind: PredicateKind,
    /// Actual semantic header.
    pub(crate) header: LogicalId,
    /// Explicit binder environment.
    pub(crate) scope: Scope,
    /// Ordered argument expression IDs.
    pub(crate) arguments: Vec<LogicalId>,
}

/// Extra semantic links of exact scalar literals, absent on ordinary expressions.
#[derive(Clone, Debug)]
pub(crate) enum ExpressionAttachment {
    /// An ordered slot of one joint selection.
    SelectedRoot(LogicalId),
    /// A coefficient and its explicit scalar evaluation map.
    ExactCoefficient {
        /// Earlier canonical coefficient value.
        value: LogicalId,
        /// Exact scalar interpretation map.
        map: LogicalId,
    },
}

/// Immutable expression weight and cached, checked shape statistics.
#[derive(Clone, Debug)]
pub(crate) struct Expression {
    /// Whether the supported original enclosure grammar contains a root instance.
    pub(crate) enclosure_roots: bool,
    /// Whether the original closure contains a positive-base rational power.
    pub(crate) enclosure_power: crate::enclosure::PowerShape,
    /// Whether the supported closure uses the logarithm law.
    pub(crate) enclosure_logs: bool,
    /// Present exponential family, with a conservative reduction bound when known.
    pub(crate) enclosure_exp: crate::enclosure::AnalyticShape,
    /// Highest completed saturating family in the original expression.
    pub(crate) enclosure_saturation: u8,
    /// Resident conservative bound, or a request for the declared reduction ceiling.
    pub(crate) enclosure_log_bound: Option<u32>,
    /// Closed enclosure grammar, derived from the original children at admission.
    pub(crate) supported_enclosure: bool,
    /// Checked scalar registry identity, mandatory for registered scalar sorts.
    pub(crate) ambient: Option<LogicalId>,
    /// Typed literal dependencies, never concealed in an operator.
    pub(crate) attachment: Option<ExpressionAttachment>,
    /// Complete operator metadata.
    pub(crate) operator: Operator,
    /// Checked result sort.
    pub(crate) sort: Sort,
    /// Actual semantic header; no first-context field exists.
    pub(crate) header: LogicalId,
    /// Mathematical interpretation.
    pub(crate) layer: Layer,
    /// Exact space for space-dependent syntax.
    pub(crate) space: Option<LogicalId>,
    /// Native case domain, absent for ordinary expressions.
    pub(crate) domain: Option<LogicalId>,
    /// Canonical unordered case associations.
    pub(crate) arms: Vec<LogicalId>,
    /// Explicit frames.
    pub(crate) scope: Scope,
    /// New bound frame, if any.
    pub(crate) binding: Option<Binding>,
    /// Ordered expression operand IDs.
    pub(crate) operands: Vec<LogicalId>,
    /// Exact operand depth.
    pub(crate) depth: u64,
    /// Exact unfolded syntax size, checked against its ceiling.
    pub(crate) unfolded: u64,
}

/// Immutable occurrence weight.
#[derive(Clone, Debug)]
pub(crate) struct Occurrence {
    /// Shared syntax ID.
    pub(crate) expression: LogicalId,
    /// Original source if present.
    pub(crate) source: Option<LogicalId>,
    /// Checked original byte range.
    pub(crate) span: Option<Span>,
    /// Ordered unique child occurrences.
    pub(crate) children: Vec<LogicalId>,
    /// Generated provenance when no original source exists.
    pub(crate) provenance: Option<LogicalId>,
    /// Preserved binder display names.
    pub(crate) binder_names: Vec<String>,
}

/// A first-class operation request; it contains no successful proof flag.
#[derive(Clone, Debug)]
pub(crate) struct RequestRecord {
    /// Requested mathematical relation/operation.
    pub(crate) task: crate::task::Task,
    /// Full context, including hypotheses.
    pub(crate) context: LogicalId,
    /// Original or generated occurrence roots in caller order.
    pub(crate) inputs: Vec<LogicalId>,
    /// Resolved resource policy.
    pub(crate) limits: Limits,
    /// Explicit earlier proof premises.
    pub(crate) premises: Vec<LogicalId>,
}

/// One authoritative heterogeneous vertex weight.
#[derive(Clone, Debug)]
pub(crate) enum Vertex {
    /// Checked content-keyed evidence, retained in the authoritative graph.
    AnonymousLemma(Box<lemmas::AnonymousLemma>),
    /// Immutable exact solving payload with checked handles.
    Solve(Box<crate::solving::SolveDraft<Handle>>),
    /// Immutable semantic declarations.
    Header(Header),
    /// Exact set syntax.
    Region(Region),
    /// Declared mathematical space.
    Space(Space),
    /// Region/value association.
    CaseArm(CaseArm),
    /// Shared location without mutable function facts.
    Boundary(Boundary),
    /// Original ordered semantic signature.
    SourceForm(SourceForm),
    /// Retained source.
    Source(Source),
    /// Mathematical context.
    Context(Context),
    /// Unproved condition syntax.
    Condition(Condition),
    /// Shared mathematical expression.
    Expression(Expression),
    /// Distinct original/generated use.
    Occurrence(Occurrence),
    /// Semantic binder frame.
    Binder(Binding),
    /// A task whose algorithm may not be admitted yet.
    Request(RequestRecord),
    /// Immutable positive weights for the complete primitive registry.
    Policy(crate::cost::CostPolicy),
    /// A kernel-checked instantiated primitive.
    Application(crate::scalar_model::CheckedStep),
    /// Checked scoped application without dummy scalar endpoints.
    ScopedApplication(Box<crate::proof_model::CheckedApplication>),
    /// A sealed proof root pointing to its authoritative application.
    Proof(LogicalId),
    /// A replayed strict improvement under one immutable policy identity.
    PolicyShortcut {
        /// Older checked mathematical proof.
        proof: LogicalId,
        /// Positive improved weight.
        cost: u64,
        /// Complete pinned table stored in the same graph.
        policy: LogicalId,
        /// Complete older snapshot used for the strict-improvement comparison.
        baseline: Vec<u8>,
    },
    /// A strictly cheaper reusable route backed by an older complete proof.
    Shortcut {
        /// Existing checked proof, never a forward dependency.
        proof: LogicalId,
        /// Positive the current logical policy cost after strict improvement checking.
        cost: u64,
    },
}

/// Typed graph edge roles; only internal validated admission can create edges.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Edge {
    /// Checked original-expression evidence pointing to an anonymous judgment.
    CheckedEvidence,
    /// Exact scalar-space and family dependency.
    Scalar(ScalarEdge),
    /// Typed solving `Coordinates` dependency.
    Coordinates,
    /// Typed solving `CoefficientCoordinates` dependency.
    CoefficientCoordinates,
    /// Typed solving `UnknownCoordinates` dependency.
    UnknownCoordinates,
    /// Typed solving `FamilyCoordinates` dependency.
    FamilyCoordinates,
    /// Typed solving `TupleEntry` dependency.
    TupleEntry(u32),
    /// Typed solving `FamilyBody` dependency.
    FamilyBody,
    /// Typed solving `Problem` dependency.
    Problem,
    /// Typed solving `BranchMember` dependency.
    BranchMember(u32),
    /// Typed solving `ParameterRegion` dependency.
    ParameterRegion,
    /// Typed solving `SolutionDomain` dependency.
    SolutionDomain,
    /// Typed solving `CoefficientBase` dependency.
    CoefficientBase,
    /// Typed solving `DefiningPolynomial` dependency.
    DefiningPolynomial,
    /// Typed solving `TowerParent` dependency.
    TowerParent,
    /// Typed solving `TowerLevel` dependency.
    TowerLevel,
    /// Typed solving `CoefficientTerm` dependency.
    CoefficientTerm(u32),
    /// Typed solving `MapSource` dependency.
    MapSource,
    /// Typed solving `MapTarget` dependency.
    MapTarget,
    /// Typed solving `GeneratorImage` dependency.
    GeneratorImage(u32),
    /// Typed solving `RootSlot` dependency.
    RootSlot(u32),
    /// Typed solving `SelectionPredicate` dependency.
    SelectionPredicate,
    /// Typed solving `SelectedBy` dependency.
    SelectedBy,
    /// Typed solving `CoefficientLiteralValue` dependency.
    CoefficientLiteralValue,
    /// Typed solving `CoefficientLiteralMap` dependency.
    CoefficientLiteralMap,
    /// Typed solving `SplitParent` dependency.
    SplitParent,
    /// Typed solving `SplitChild` dependency.
    SplitChild(u32),
    /// Typed solving `SplitMap` dependency.
    SplitMap(u32),
    /// Typed solving `OriginalPredicate` dependency.
    OriginalPredicate,
    /// Typed solving `OriginalDenominator` dependency.
    OriginalDenominator(u32),
    /// Typed solving `LocalBase` dependency.
    LocalBase(u32),
    /// Typed solving `LocalJacobian` dependency.
    LocalJacobian,
    /// Typed solving `LocalInverse` dependency.
    LocalInverse,
    /// Typed solving `SplitFactor` dependency.
    SplitFactor(u32),
    /// Typed solving `DenominatorFactor` dependency.
    DenominatorFactor(u32),
    /// Typed solving `TupleMember` dependency.
    TupleMember(u32),
    /// Typed solving `FamilyImage` dependency.
    FamilyImage,
    /// Typed solving `LocalGerm` dependency.
    LocalGerm,
    /// Typed solving `FractionNumerator` dependency.
    FractionNumerator,
    /// Typed solving `FractionDenominator` dependency.
    FractionDenominator,

    /// An ordered mathematical operand.
    Operand(u32),
    /// A source occurrence's mathematical meaning.
    Denotes,
    /// Original retained source.
    Source,
    /// Ordered source-occurrence operand.
    Child(u32),
    /// Context interpretation.
    InContext,
    /// Ordered hypothesis reference.
    Assumption(u32),
    /// Ordered predicate expression argument.
    PredicateArgument(u32),
    /// An expression's semantic binder frame.
    Binder,
    /// A binder's scoped body position.
    Body(u32),
    /// Generated expression or occurrence provenance.
    Provenance,
    /// Ordered operation input root.
    Input(u32),
    /// Checked conclusion input.
    ConclusionLeft,
    /// Checked conclusion output.
    ConclusionRight,
    /// Ordered prerequisite proof.
    Premise(u32),
    /// Required typed domain condition.
    Obligation(u32),
    /// The checked application underlying a proof.
    Justifies,
    /// An older proof expanded by a derived shortcut.
    DependsOn,
    /// Immutable logical-cost policy for an admitted shortcut.
    CostPolicy,
    /// Typed `InHeader` relationship.
    InHeader,
    /// Typed `ParentHeader` relationship.
    ParentHeader,
    /// Typed `DeclarationSpace` relationship.
    DeclarationSpace(u32),
    /// Typed `SpaceDomain` relationship.
    SpaceDomain,
    /// Typed `RegionInput` relationship.
    RegionInput(u32),
    /// Typed `MembershipPredicate` relationship.
    MembershipPredicate,
    /// Typed `ArmRegion` relationship.
    ArmRegion,
    /// Typed `ArmValue` relationship.
    ArmValue,
    /// Typed `BoundaryLocation` relationship.
    BoundaryLocation,
    /// Typed `SourceArgument` relationship.
    SourceArgument(u32),
    /// Typed `CaseDomain` relationship.
    CaseDomain,
    /// Typed `CaseMember` relationship.
    CaseMember(u32),
    /// Typed `InSpace` relationship.
    InSpace,
    /// Typed `AdmissibleRegion` relationship.
    AdmissibleRegion,
    /// Typed `ParentContext` relationship.
    ParentContext,
    /// Typed `ConclusionObject` relationship.
    ConclusionObject(u32),
    /// Typed `IncidentRegion` relationship.
    IncidentRegion,
    /// Typed `AtBoundary` relationship.
    AtBoundary,
    /// Typed `OfFunction` relationship.
    OfFunction,
    /// Typed `SideRegion` relationship.
    SideRegion(crate::domain_model::Side),
    /// Typed `JetValue` relationship.
    JetValue(u32),
    /// Typed `ComparedRegion` relationship.
    ComparedRegion(u32),
    /// Typed `CoverMember` relationship.
    CoverMember(u32),
    /// Typed `ArgumentRegion` relationship.
    ArgumentRegion(u32),
}

/// Typed scalar-space dependencies with a stable graph encoding.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ScalarEdge {
    /// Exact `CheckedAmbient` dependency.
    CheckedAmbient,
    /// Exact `DeclarationScalarSpace` dependency.
    DeclarationScalarSpace(u32),
    /// Exact `PlacementSource` dependency.
    PlacementSource,
    /// Exact `PlacementAmbient` dependency.
    PlacementAmbient,
    /// Exact `PlacementDefinition` dependency.
    PlacementDefinition,
    /// Exact `LocalFrame` dependency.
    LocalFrame,
    /// Exact `Base` dependency.
    Base,
    /// Exact `Family` dependency.
    Family,
    /// Exact `BaseStratum` dependency.
    BaseStratum,
    /// Exact `FibreSpace` dependency.
    FibreSpace,
    /// Exact `LeftEndpoint` dependency.
    LeftEndpoint,
    /// Exact `RightEndpoint` dependency.
    RightEndpoint,
    /// Exact `EvaluationSlot` dependency.
    EvaluationSlot(u32),
    /// Exact `Localization` dependency.
    Localization(u32),
    /// Exact `PlacedOperand` dependency.
    PlacedOperand(u32),
    /// Exact `PlacementPiece` dependency.
    PlacementPiece(u32),
    /// Exact `BaseArgument` dependency.
    BaseArgument(u32),
    /// Exact `FibreArgument` dependency.
    FibreArgument(u32),
    /// Exact `FamilyArgument` dependency.
    FamilyArgument(u32),
    /// Exact `SelectionCell` dependency.
    SelectionCell,
}

impl ScalarEdge {
    /// Stable append-only graph role tags.
    pub(crate) const fn tag(self) -> (u8, u32) {
        match self {
            Self::CheckedAmbient => (80, 0),
            Self::DeclarationScalarSpace(position) => (81, position),
            Self::PlacementSource => (82, 0),
            Self::PlacementAmbient => (83, 0),
            Self::PlacementDefinition => (84, 0),
            Self::LocalFrame => (85, 0),
            Self::Base => (86, 0),
            Self::Family => (87, 0),
            Self::BaseStratum => (88, 0),
            Self::FibreSpace => (89, 0),
            Self::LeftEndpoint => (90, 0),
            Self::RightEndpoint => (91, 0),
            Self::EvaluationSlot(position) => (92, position),
            Self::Localization(position) => (93, position),
            Self::PlacedOperand(position) => (94, position),
            Self::PlacementPiece(position) => (95, position),
            Self::BaseArgument(position) => (96, position),
            Self::FibreArgument(position) => (97, position),
            Self::FamilyArgument(position) => (98, position),
            Self::SelectionCell => (99, 0),
        }
    }
}

/// An insertion whose key did not exist before this transaction.
#[derive(Clone, Debug)]
enum Undo {
    /// A new graph record, logical index and optional interning key.
    Vertex {
        /// Logical identifier allocated during the batch.
        id: LogicalId,
        /// Only interned vertex kinds insert a structural key.
        key: Option<Vec<u8>>,
        /// Policy identity indexed by this insertion, if present.
        policy: Option<(String, u32)>,
    },
    /// A scalar inventory index contains only its complete canonical key and node identifier.
    ScalarJudgment {
        /// Newly allocated anonymous node identifier.
        id: LogicalId,
        /// Shared canonical key, also owned by the graph node.
        key: Arc<[u8]>,
    },
    /// Remove a complete Machin key when its graph node is rolled back.
    PiJudgment {
        /// Logical identity reserved for the checked node.
        id: LogicalId,
        /// Immutable canonical fields shared with the graph payload.
        certificate: Arc<crate::pi_bounds::PiBoundCertificate>,
    },
    /// A newly claimed source name.
    Source(String),
    /// A previously parentless occurrence assigned to a new parent.
    Parent(LogicalId),
}

/// Bounded counter checkpoint and a position in the shared insertion log.
#[derive(Clone, Debug)]
struct Journal {
    /// Every occupancy counter before the first mutation.
    counts: GraphCounts,
    /// Graph-owned anonymous payload before the first mutation.
    judgment_bytes: u64,
    /// Number of retained anonymous nodes under this checkpoint or state.
    judgment_count: usize,
    /// At most one next serial per fixed vertex kind.
    next: BTreeMap<VertexKind, u64>,
    /// First undo entry owned by this checkpoint.
    start: usize,
    /// A parent checkpoint retains committed inner edits.
    nested: bool,
}

/// Append-only storage plus derived lookup maps; the graph owns every weight.
#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct State {
    /// Authoritative heterogeneous graph.
    pub(crate) graph: DiGraph<Vertex, Edge, u32>,
    /// Logical-to-physical lookup, never serialized.
    pub(crate) indices: BTreeMap<LogicalId, NodeIndex<u32>>,
    /// Exact canonical syntax/context/condition keys.
    pub(crate) intern: InternIndex,
    /// Kind-specific scalar keys index graph nodes without retaining a second checked payload.
    pub(crate) scalar_judgments: BTreeMap<Arc<[u8]>, LogicalId>,
    /// Complete Machin keys index only the authoritative anonymous graph nodes.
    pub(crate) pi_judgments: PiIndex,
    /// Immutable policy identity lookup, independent of unrelated graph records.
    pub(crate) policies: BTreeMap<(String, u32), LogicalId>,
    /// Next serial per kind, independent of graph indices.
    next: BTreeMap<VertexKind, u64>,
    /// Source key lookup used to reject conflicting source bytes.
    sources: BTreeMap<String, LogicalId>,
    /// Parent uniqueness for all occurrence records.
    parents: BTreeMap<LogicalId, LogicalId>,
    /// Measured retained source bytes.
    source_bytes: u64,
    /// Measured original occurrence count.
    originals: u64,
    /// Conservative logical payload bytes, including owned interning keys.
    payload: u64,
    /// One retained-payload account for anonymous graph judgments.
    judgment_bytes: u64,
    /// Number of retained anonymous nodes under this checkpoint or state.
    judgment_count: usize,
    /// Shared insertion log for all active transaction checkpoints.
    journal: Option<Vec<Undo>>,
}

impl State {
    /// Start an empty graph without allocating the requested maximum capacities.
    pub(crate) fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            indices: BTreeMap::new(),
            intern: InternIndex::default(),
            scalar_judgments: BTreeMap::new(),
            pi_judgments: PiIndex::default(),
            policies: BTreeMap::new(),
            next: BTreeMap::new(),
            sources: BTreeMap::new(),
            parents: BTreeMap::new(),
            source_bytes: 0,
            originals: 0,
            payload: 0,
            judgment_bytes: 0,
            judgment_count: 0,
            journal: None,
        }
    }

    /// Capture the bounded counter inventory before any new record is inserted.
    fn begin(&mut self) -> Journal {
        let checkpoint = Journal {
            counts: GraphCounts {
                vertices: self.graph.node_count(),
                edges: self.graph.edge_count(),
                source_bytes: self.source_bytes,
                original_occurrences: self.originals,
                payload_bytes: self.payload,
            },
            judgment_bytes: self.judgment_bytes,
            judgment_count: self.judgment_count,
            next: self.next.clone(),
            start: self.journal.as_ref().map_or(0, Vec::len),
            nested: self.journal.is_some(),
        };
        self.journal.get_or_insert_with(Vec::new);
        checkpoint
    }

    /// Reserve the undo entry before performing the corresponding mutation.
    fn record(&mut self, entry: Undo) -> Result<(), Fault> {
        if let Some(journal) = &mut self.journal {
            journal
                .try_reserve(1)
                .map_err(|_| Fault::allocation(Location::new("admission journal")))?;
            journal.push(entry);
        }
        Ok(())
    }

    /// Commit inner edits into the parent log, or reverse only this checkpoint's edits.
    fn finish(&mut self, checkpoint: Journal, commit: bool) {
        let nested = checkpoint.nested;
        if !commit {
            self.rollback(checkpoint);
        }
        if !nested {
            self.journal = None;
        }
    }

    /// Restore exact graph indices, maps and counters without fallible validation or copying.
    fn rollback(&mut self, checkpoint: Journal) {
        while self
            .journal
            .as_ref()
            .is_some_and(|log| log.len() > checkpoint.start)
        {
            let Some(entry) = self.journal.as_mut().and_then(Vec::pop) else {
                break;
            };
            self.undo(entry);
        }
        while self.graph.edge_count() > checkpoint.counts.edges {
            let index = EdgeIndex::new(self.graph.edge_count() - 1);
            let _removed = self.graph.remove_edge(index);
        }
        while self.graph.node_count() > checkpoint.counts.vertices {
            let index = NodeIndex::new(self.graph.node_count() - 1);
            let _removed = self.graph.remove_node(index);
        }
        self.judgment_bytes = checkpoint.judgment_bytes;
        self.judgment_count = checkpoint.judgment_count;
        self.next = checkpoint.next;
        self.payload = checkpoint.counts.payload_bytes;
        self.source_bytes = checkpoint.counts.source_bytes;
        self.originals = checkpoint.counts.original_occurrences;
    }

    /// Remove the derived index entries reserved before each graph mutation.
    fn undo(&mut self, entry: Undo) {
        match entry {
            Undo::Vertex { id, key, policy } => {
                self.indices.remove(&id);
                if let Some(key) = key {
                    self.intern.remove(key);
                }
                if let Some(identity) = policy {
                    self.policies.remove(&identity);
                }
            }
            Undo::ScalarJudgment { id, key } => {
                self.indices.remove(&id);
                self.scalar_judgments.remove(key.as_ref());
            }
            Undo::PiJudgment { id, certificate } => {
                self.indices.remove(&id);
                self.pi_judgments.remove(certificate);
            }
            Undo::Source(key) => {
                self.sources.remove(&key);
            }
            Undo::Parent(child) => {
                self.parents.remove(&child);
            }
        }
    }

    /// Append-only proof serials count retained proofs without walking unrelated objects.
    pub(crate) fn proof_count(&self) -> u64 {
        self.next.get(&VertexKind::Proof).copied().unwrap_or(0)
    }

    /// Meter one structural-key probe by its own bytes, independent of retained payload.
    pub(crate) fn interned(
        &self,
        key: &[u8],
        budget: &mut Budget,
    ) -> Result<Option<LogicalId>, Fault> {
        lookup_work(key.len(), budget, Location::new("interning lookup"))?;
        Ok(self.intern.get(key).copied())
    }

    /// Look up a logical ID without indexing or trusting its kind.
    pub(crate) fn node(&self, id: LogicalId) -> Result<&Vertex, Fault> {
        self.indices
            .get(&id)
            .and_then(|index| self.graph.node_weight(*index))
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, Location::new("node lookup")))
    }

    /// Read an expression of the correct vertex kind.
    pub(crate) fn expression(&self, id: LogicalId) -> Result<&Expression, Fault> {
        match self.node(id)? {
            Vertex::Expression(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("expression lookup"),
            )),
        }
    }

    /// Read an immutable header of the correct graph kind.
    pub(crate) fn header(&self, id: LogicalId) -> Result<&Header, Fault> {
        match self.node(id)? {
            Vertex::Header(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("header lookup"),
            )),
        }
    }

    /// Read an immutable region of the correct graph kind.
    pub(crate) fn region(&self, id: LogicalId) -> Result<&Region, Fault> {
        match self.node(id)? {
            Vertex::Region(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("region lookup"),
            )),
        }
    }

    /// Read an immutable space of the correct graph kind.
    pub(crate) fn space(&self, id: LogicalId) -> Result<&Space, Fault> {
        match self.node(id)? {
            Vertex::Space(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("space lookup"),
            )),
        }
    }

    /// Read an immutable arm of the correct graph kind.
    pub(crate) fn arm(&self, id: LogicalId) -> Result<&CaseArm, Fault> {
        match self.node(id)? {
            Vertex::CaseArm(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("arm lookup"),
            )),
        }
    }

    /// Read an immutable boundary of the correct graph kind.
    pub(crate) fn boundary(&self, id: LogicalId) -> Result<&Boundary, Fault> {
        match self.node(id)? {
            Vertex::Boundary(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("boundary lookup"),
            )),
        }
    }

    /// Read an immutable source form of the correct graph kind.
    pub(crate) fn source_form(&self, id: LogicalId) -> Result<&SourceForm, Fault> {
        match self.node(id)? {
            Vertex::SourceForm(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("source_form lookup"),
            )),
        }
    }

    /// Read an immutable context of the correct kind.
    pub(crate) fn context(&self, id: LogicalId) -> Result<&Context, Fault> {
        match self.node(id)? {
            Vertex::Context(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("context lookup"),
            )),
        }
    }

    /// Read an occurrence of the correct kind.
    pub(crate) fn occurrence(&self, id: LogicalId) -> Result<&Occurrence, Fault> {
        match self.node(id)? {
            Vertex::Occurrence(value) => Ok(value),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("occurrence lookup"),
            )),
        }
    }
}

/// Occupancy counters independent of petgraph's internal index layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphCounts {
    /// Published vertices.
    pub vertices: usize,
    /// Published edges.
    pub edges: usize,
    /// Retained original source bytes.
    pub source_bytes: u64,
    /// Original occurrence count.
    pub original_occurrences: u64,
    /// Accounted logical payload bytes.
    pub payload_bytes: u64,
}

/// The independently owned symbolic store; it has no engine-package dependency.
pub struct Cas {
    /// Checked receipt judgments are local runtime evidence, excluded from canonical graph bytes.
    pub(crate) receipt_records: std::sync::Mutex<crate::decision_kernel::cache::Cache>,
    /// Serialize one store's receipt validation and publication without changing its thread traits.
    pub(crate) receipt_guard: std::sync::Mutex<()>,
    /// Runtime owner of request-local canonical records, excluded from mathematical identity.
    pub(crate) memo_owner: Arc<()>,
    /// Cache namespace generation advances on rollback independently of mathematical state.
    pub(crate) memo_epoch: u64,
    /// Runtime identity excluded from canonical outputs.
    token: Arc<StoreToken>,
    /// Permanent storage ceilings.
    pub(crate) limits: Limits,
    /// Last completely committed state.
    pub(crate) state: State,
}

impl fmt::Debug for Cas {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Cas")
            .field("counts", &self.counts())
            .field("interning", &self.interning_snapshot())
            .finish_non_exhaustive()
    }
}

impl Cas {
    /// Create an empty store under explicit permanent ceilings.
    ///
    /// # Errors
    /// Refuses inconsistent limits before constructing store state.
    pub fn new(limits: Limits) -> Result<Self, Fault> {
        limits.validate()?;
        Ok(Self {
            receipt_records: std::sync::Mutex::default(),
            receipt_guard: std::sync::Mutex::default(),
            memo_owner: Arc::new(()),
            memo_epoch: 0,
            token: Arc::new(StoreToken::default()),
            limits,
            state: State::new(),
        })
    }

    /// Read cumulative comparator diagnostics without charging or resetting any budget.
    /// Includes lookup, insertion and rollback removal; failed work is never refunded.
    pub(crate) fn interning_snapshot(&self) -> InterningSnapshot {
        self.token.snapshot()
    }

    /// Journal touched records, retaining spent budget on both commit and rollback.
    /// Inner commits remain reversible by an enclosing transaction.
    pub(crate) fn transaction<T>(
        &mut self,
        budget: &mut Budget,
        f: impl FnOnce(&mut Self, &mut Budget) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        // Only the fixed vertex-kind counter inventory is copied at a checkpoint.
        let at = Location::new("proof transaction");
        budget.work(256, at)?;
        let bytes = size(self.state.next.len(), at)?
            .checked_mul(64)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        let previous = budget.reserve_temporary(bytes, at)?;
        let checkpoint = self.state.begin();
        let result = crate::decision_kernel::cache::attempt(self, budget, f);
        self.state.finish(checkpoint, result.is_ok());
        if result.is_err() {
            self.memo_epoch = self.memo_epoch.saturating_add(1);
            self.receipt_records
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
            budget.checked_records.forget(&self.memo_owner);
        }
        budget.restore_limits(previous);
        result
    }

    /// Temporarily separate the store identity from its state, restoring it on every result.
    pub(crate) fn with_state<T>(
        &mut self,
        f: impl FnOnce(&Self, &mut State) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        let mut state = std::mem::replace(&mut self.state, State::new());
        let result = f(self, &mut state);
        self.state = state;
        result
    }

    /// Unmetered full copy for tests requiring an untouched control store.
    #[cfg(test)]
    pub(crate) fn full_copy_for_test(&self) -> Self {
        Self {
            receipt_records: std::sync::Mutex::default(),
            receipt_guard: std::sync::Mutex::default(),
            memo_owner: Arc::new(()),
            memo_epoch: 0,
            token: Arc::clone(&self.token),
            limits: self.limits.clone(),
            state: self.state.clone(),
        }
    }

    /// Read current occupancy without exposing a physical graph index.
    #[must_use]
    pub fn counts(&self) -> GraphCounts {
        GraphCounts {
            vertices: self.state.graph.node_count(),
            edges: self.state.graph.edge_count(),
            source_bytes: self.state.source_bytes,
            original_occurrences: self.state.originals,
            payload_bytes: self.state.payload,
        }
    }

    /// Validate a live store-scoped handle and its logical reference.
    pub(crate) fn checked(&self, handle: &Handle) -> Result<LogicalId, Fault> {
        self.checked_in(&self.state, handle)
    }

    /// Validate an existing reference against the transaction's actual candidate state.
    fn checked_in(&self, state: &State, handle: &Handle) -> Result<LogicalId, Fault> {
        if !Arc::ptr_eq(&self.token, &handle.token) {
            return Err(Fault::invalid(
                InvalidCode::ForeignHandle,
                Location::new("handle lookup"),
            ));
        }
        state.node(handle.id)?;
        Ok(handle.id)
    }

    /// Mint a handle only after its logical record exists in a committed store.
    pub(crate) fn handle(&self, id: LogicalId) -> Handle {
        Handle {
            token: Arc::clone(&self.token),
            id,
        }
    }

    /// Read ordered mathematical operands as checked handles.
    ///
    /// # Errors
    /// Refuses a foreign/wrong-kind handle or insufficient work/allocation.
    pub fn operands(&self, handle: &Handle, budget: &mut Budget) -> Result<Vec<Handle>, Fault> {
        let id = self.checked(handle)?;
        let expression = self.state.expression(id)?;
        let at = Location::new("operand read");
        let bytes = size(expression.operands.len(), at)?
            .checked_mul(16)
            .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, at))?;
        budget.check(BudgetKind::OutputBytes, bytes, at)?;
        budget.work(size(expression.operands.len(), at)?, at)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(expression.operands.len())
            .map_err(|_| Fault::allocation(at))?;
        result.extend(
            expression
                .operands
                .iter()
                .map(|operand| self.handle(*operand)),
        );
        Ok(result)
    }

    /// Borrow an expression's operator and result sort without formatting a tree.
    ///
    /// # Errors
    /// Refuses foreign handles or handles of another vertex kind.
    pub fn expression(&self, handle: &Handle) -> Result<(&Operator, &Sort), Fault> {
        let expression = self.state.expression(self.checked(handle)?)?;
        Ok((&expression.operator, &expression.sort))
    }

    /// Borrow an immutable source key and its exact UTF-8 text.
    ///
    /// # Errors
    /// Refuses a foreign handle or a handle of another vertex kind.
    pub fn source(&self, handle: &Handle) -> Result<(&str, &str), Fault> {
        match self.state.node(self.checked(handle)?)? {
            Vertex::Source(source) => Ok((&source.key, &source.text)),
            _ => Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("source read"),
            )),
        }
    }

    /// Validate and atomically publish a child-before-parent batch.
    ///
    /// # Errors
    /// Refuses malformed drafts, foreign/forward references, or exhausted budgets;
    /// the published graph and next logical identifiers remain unchanged.
    pub fn admit(&mut self, drafts: &[Draft], budget: &mut Budget) -> Result<Vec<Handle>, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.admit_batch(drafts, budget);
        budget.restore_limits(previous);
        result
    }

    /// Apply a batch under the already intersected storage ceilings.
    fn admit_batch(&mut self, drafts: &[Draft], budget: &mut Budget) -> Result<Vec<Handle>, Fault> {
        if drafts.is_empty() {
            return Ok(Vec::new());
        }
        let at = Location::new("transaction");
        for draft in drafts {
            if let Draft::Solve(value) = draft {
                crate::solving::graph::preflight(value, budget)?;
            }
        }
        let pending = crate::validation::draft_payload(drafts, budget)?;
        let reserved = size(self.state.next.len(), at)?
            .checked_mul(64)
            .and_then(|n| n.checked_add(pending))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        let peak = self
            .state
            .payload
            .checked_add(reserved)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        for (kind, needed) in [
            (BudgetKind::StagedPayloadBytes, peak),
            (
                BudgetKind::StagedVertices,
                size(self.state.graph.node_count(), at)?,
            ),
            (
                BudgetKind::StagedEdges,
                size(self.state.graph.edge_count(), at)?,
            ),
        ] {
            storage_check(&self.limits, budget, kind, needed, at)?;
        }
        budget.work(pending, at)?;
        // The fixed vertex-kind inventory bounds checkpoint counter copying.
        budget.work(256, at)?;
        let mut staged = std::mem::replace(&mut self.state, State::new());
        let checkpoint = staged.begin();
        let result = self.admit_staged(&mut staged, drafts, reserved, budget);
        staged.finish(checkpoint, result.is_ok());
        if result.is_err() {
            self.memo_epoch = self.memo_epoch.saturating_add(1);
            self.receipt_records
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
            budget.checked_records.forget(&self.memo_owner);
        }
        self.state = staged;
        result
    }

    /// Resolve a batch before exposing any handles; every mutation is journaled.
    fn admit_staged(
        &self,
        state: &mut State,
        drafts: &[Draft],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<Vec<Handle>, Fault> {
        let at = Location::new("transaction");
        let mut resolved = Vec::new();
        resolved
            .try_reserve_exact(drafts.len())
            .map_err(|_| Fault::allocation(at))?;
        for draft in drafts {
            budget.work(1, at)?;
            let id = self.admit_one(state, draft, &resolved, pending, budget)?;
            resolved.push(id);
        }
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(resolved.len())
            .map_err(|_| Fault::allocation(at))?;
        handles.extend(resolved.iter().map(|id| self.handle(*id)));
        Ok(handles)
    }

    /// Append generated closed syntax inside a private producer transaction.
    /// The enclosing transaction owns rollback on any failure.
    pub(crate) fn generated(
        &mut self,
        context: LogicalId,
        operator: Operator,
        sort: Sort,
        operands: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("generated syntax");
        let header = match self.state.node(context)? {
            Vertex::Context(value) => value.header_id,
            Vertex::Header(_) => context,
            _ => return Err(Fault::invalid(InvalidCode::Reference, at)),
        };
        let mut locals = vec![header];
        locals
            .try_reserve(operands.len())
            .map_err(|_| Fault::allocation(at))?;
        locals.extend_from_slice(operands);
        let mut references = Vec::new();
        references
            .try_reserve_exact(operands.len())
            .map_err(|_| Fault::allocation(at))?;
        for position in 0..operands.len() {
            let index = position
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?;
            references.push(Reference::Local(position_u32(index)?));
        }
        let draft = ExpressionDraft {
            space: None,
            operator,
            sort,
            header: Reference::Local(0),
            scope: Vec::new(),
            binding: None,
            operands: references,
        };
        let pending =
            crate::validation::draft_payload(&[Draft::Expression(draft.clone())], budget)?;
        let mut staged = std::mem::replace(&mut self.state, State::new());
        let result = self.admit_expression(&mut staged, &draft, &locals, pending, budget);
        self.state = staged;
        result
    }

    /// Resolve one existing or strictly earlier batch reference.
    pub(crate) fn resolve(
        &self,
        state: &State,
        reference: &Reference,
        locals: &[LogicalId],
    ) -> Result<LogicalId, Fault> {
        match reference {
            Reference::Existing(handle) => self.checked_in(state, handle),
            Reference::Local(index) => usize::try_from(*index)
                .ok()
                .and_then(|index| locals.get(index))
                .copied()
                .ok_or_else(|| {
                    Fault::invalid(InvalidCode::Reference, Location::new("draft reference"))
                }),
        }
    }

    /// Resolve an ordered finite reference list before looking up any weights.
    pub(crate) fn resolve_all(
        &self,
        state: &State,
        references: &[Reference],
        locals: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<Vec<LogicalId>, Fault> {
        let at = Location::new("draft references");
        budget.work(size(references.len(), at)?, at)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(references.len())
            .map_err(|_| Fault::allocation(at))?;
        for reference in references {
            result.push(self.resolve(state, reference, locals)?);
        }
        Ok(result)
    }

    /// Dispatch one already bounded draft into the private candidate state.
    fn admit_one(
        &self,
        state: &mut State,
        draft: &Draft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        match draft {
            Draft::Solve(value) => self.admit_solve(state, value, locals, pending, budget),
            Draft::RootValue { .. } | Draft::ExactCoefficient { .. } => {
                self.admit_solve_literal(state, draft, locals, pending, budget)
            }
            Draft::Source { key, text } => self.admit_source(state, key, text, pending, budget),
            Draft::Header(value) => self.admit_header(state, value, locals, pending, budget),
            Draft::Context(value) => {
                self.admit_context_frame(state, value, locals, pending, budget)
            }
            Draft::Region(value) => self.admit_region(state, value, None, locals, pending, budget),
            Draft::FamilyRegion { region, lies_over } => {
                self.admit_region(state, region, Some(lies_over), locals, pending, budget)
            }
            Draft::Space(value) => self.admit_space(state, value, locals, pending, budget),
            Draft::CaseArm(value) => self.admit_arm(state, value, locals, pending, budget),
            Draft::Boundary(value) => self.admit_boundary(state, value, locals, pending, budget),
            Draft::SourceForm(value) => {
                self.admit_source_form(state, value, locals, pending, budget)
            }
            Draft::Condition { .. } => self.admit_condition(state, draft, locals, pending, budget),
            Draft::Cases(value) => self.admit_cases(state, value, locals, pending, budget),
            Draft::Expression(expression) => {
                self.admit_expression(state, expression, locals, pending, budget)
            }
            Draft::Occurrence(occurrence) => {
                self.admit_occurrence(state, occurrence, locals, pending, budget)
            }
            Draft::Request { .. } => self.admit_request(state, draft, locals, pending, budget),
        }
    }

    /// Resolve and validate one condition before its journaled graph insertion.
    fn admit_condition(
        &self,
        state: &mut State,
        draft: &Draft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let Draft::Condition {
            kind,
            header,
            scope,
            arguments,
        } = draft
        else {
            return Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("condition admission"),
            ));
        };
        let header = self.resolve(state, header, locals)?;
        let arguments = self.resolve_all(state, arguments, locals, budget)?;
        crate::validation::condition(state, kind, header, scope, &arguments, budget)?;
        let mut edges = vec![(header, Edge::InHeader)];
        for (position, argument) in arguments.iter().enumerate() {
            edges.push((*argument, Edge::PredicateArgument(position_u32(position)?)));
        }
        self.insert(
            state,
            Vertex::Condition(Condition {
                kind: kind.clone(),
                header,
                scope: scope.clone(),
                arguments,
            }),
            &edges,
            pending,
            budget,
        )
    }

    /// Resolve and validate one request before its journaled graph insertion.
    fn admit_request(
        &self,
        state: &mut State,
        draft: &Draft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let Draft::Request {
            task,
            premises,
            context,
            inputs,
            limits,
        } = draft
        else {
            return Err(Fault::invalid(
                InvalidCode::Reference,
                Location::new("request admission"),
            ));
        };
        let context = self.resolve(state, context, locals)?;
        let inputs = self.resolve_all(state, inputs, locals, budget)?;
        for handle in task.references() {
            self.checked_in(state, handle)?;
        }
        let premises = self.resolve_all(state, premises, locals, budget)?;
        for premise in &premises {
            if !matches!(state.node(*premise)?, Vertex::Proof(_)) {
                return Err(Fault::invalid(
                    InvalidCode::Reference,
                    Location::new("request admission"),
                ));
            }
        }
        crate::request_model::validate(state, task, context, &inputs, limits, budget)?;
        let mut edges = vec![(context, Edge::InContext)];
        for (position, premise) in premises.iter().enumerate() {
            edges.push((*premise, Edge::Premise(position_u32(position)?)));
        }
        for (position, input) in inputs.iter().enumerate() {
            edges.push((*input, Edge::Input(position_u32(position)?)));
        }
        self.insert(
            state,
            Vertex::Request(RequestRecord {
                task: task.clone(),
                premises,
                context,
                inputs,
                limits: limits.as_ref().clone(),
            }),
            &edges,
            pending,
            budget,
        )
    }

    /// Retain or reuse a source while rejecting a conflicting key.
    fn admit_source(
        &self,
        state: &mut State,
        key: &str,
        text: &str,
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("source admission");
        lookup_work(key.len(), budget, at)?;
        if let Some(id) = state.sources.get(key) {
            budget.work(size(text.len(), at)?, at)?;
            if let Vertex::Source(source) = state.node(*id)?
                && source.text == text
            {
                return Ok(*id);
            }
            return Err(Fault::invalid(InvalidCode::SourceConflict, at));
        }
        let next = state
            .source_bytes
            .checked_add(size(text.len(), at)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::SourceBytes, at))?;
        storage_check(&self.limits, budget, BudgetKind::SourceBytes, next, at)?;
        let id = self.insert(
            state,
            Vertex::Source(Source {
                key: key.to_owned(),
                text: text.to_owned(),
            }),
            &[],
            pending,
            budget,
        )?;
        state.record(Undo::Source(key.to_owned()))?;
        state.sources.insert(key.to_owned(), id);
        state.source_bytes = next;
        Ok(id)
    }

    /// Validate mathematical syntax before admitting its binder and expression.
    fn admit_expression(
        &self,
        state: &mut State,
        draft: &ExpressionDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("expression admission");
        let header = self.resolve(state, &draft.header, locals)?;
        let operands = self.resolve_all(state, &draft.operands, locals, budget)?;
        let (depth, unfolded) =
            crate::validation::expression(state, draft, header, &operands, budget)?;
        let explicit = draft
            .space
            .as_ref()
            .map(|value| self.resolve(state, value, locals))
            .transpose()?;
        let space = crate::header::expression_space(
            state,
            header,
            &draft.operator,
            &draft.sort,
            explicit,
            &operands,
            budget,
        )?;
        let ambient = crate::solving::sorts::ambient(state, header, &draft.sort)?;
        let enclosure_exp = crate::enclosure::exp_metadata(state, &draft.operator, &operands)?;
        let enclosure_power = crate::enclosure::power_metadata(state, &draft.operator, &operands)?;
        let (enclosure_logs, enclosure_log_bound) =
            crate::enclosure::log_metadata(state, &draft.operator, &operands)?;
        let expression = Vertex::Expression(Expression {
            enclosure_exp,
            enclosure_power,
            enclosure_saturation: crate::enclosure::saturation_metadata(
                state,
                &draft.operator,
                &operands,
            )?,
            enclosure_logs,
            enclosure_log_bound,
            enclosure_roots: crate::enclosure::has_roots(state, &draft.operator, &operands)?,
            supported_enclosure: crate::enclosure::supported_shape(
                state,
                &draft.operator,
                &operands,
            )?,
            ambient,
            attachment: None,
            operator: draft.operator.clone(),
            sort: draft.sort.clone(),
            header,
            layer: if space.is_some() && draft.sort != Sort::Test {
                Layer::Distribution
            } else if draft.sort == Sort::Value(crate::model::AtomicSort::Boolean) {
                Layer::Logic
            } else {
                Layer::Pointwise
            },
            space,
            domain: None,
            arms: Vec::new(),
            scope: draft.scope.clone(),
            binding: draft.binding.clone(),
            operands: operands.clone(),
            depth,
            unfolded,
        });
        let (_, key) = vertex_key(&expression, state, budget)?;
        if let Some(id) = state.interned(&key, budget)? {
            return Ok(id);
        }
        let mut edges = vec![(header, Edge::InHeader)];
        if let Some(ambient) = ambient {
            edges.push((ambient, Edge::Scalar(ScalarEdge::CheckedAmbient)));
        }
        if let Some(space) = space {
            edges.push((space, Edge::InSpace));
        }
        for (position, operand) in operands.iter().enumerate() {
            edges.push((*operand, Edge::Operand(position_u32(position)?)));
        }
        if let Some(binding) = &draft.binding {
            let mut bodies = Vec::new();
            bodies
                .try_reserve_exact(binding.body_operands.len())
                .map_err(|_| Fault::allocation(at))?;
            for position in &binding.body_operands {
                let index = usize::try_from(*position)
                    .map_err(|_| Fault::invalid(InvalidCode::Scope, at))?;
                let body = operands
                    .get(index)
                    .copied()
                    .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
                bodies.push((body, Edge::Body(*position)));
            }
            // Binder records intentionally remain distinct for each scoped expression.
            let binder = self.insert(
                state,
                Vertex::Binder(binding.clone()),
                &bodies,
                pending,
                budget,
            )?;
            edges.push((binder, Edge::Binder));
        }
        self.insert(state, expression, &edges, pending, budget)
    }

    /// Validate distinct occurrences and atomically reserve their parent links.
    fn admit_occurrence(
        &self,
        state: &mut State,
        draft: &OccurrenceDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("occurrence admission");
        let expression = self.resolve(state, &draft.expression, locals)?;
        let source = draft
            .source
            .as_ref()
            .map(|value| self.resolve(state, value, locals))
            .transpose()?;
        let provenance = draft
            .provenance
            .as_ref()
            .map(|value| self.resolve(state, value, locals))
            .transpose()?;
        let children = self.resolve_all(state, &draft.children, locals, budget)?;
        let occurrence = Occurrence {
            expression,
            source,
            span: draft.span,
            children,
            provenance,
            binder_names: draft.binder_names.clone(),
        };
        crate::validation::occurrence(state, &occurrence, budget)?;
        let mut unique = BTreeSet::new();
        let mut edges = vec![(expression, Edge::Denotes)];
        for (position, child) in occurrence.children.iter().enumerate() {
            budget.work(1, at)?;
            if state.parents.contains_key(child) || !unique.insert(*child) {
                return Err(Fault::invalid(InvalidCode::OccurrenceParent, at));
            }
            edges.push((*child, Edge::Child(position_u32(position)?)));
        }
        if let Some(source) = source {
            edges.push((source, Edge::Source));
        }
        if let Some(provenance) = provenance {
            edges.push((provenance, Edge::Provenance));
        }
        let originals = state
            .originals
            .checked_add(u64::from(source.is_some()))
            .ok_or_else(|| Fault::capacity(BudgetKind::OriginalOccurrences, at))?;
        storage_check(
            &self.limits,
            budget,
            BudgetKind::OriginalOccurrences,
            originals,
            at,
        )?;
        let id = self.insert(
            state,
            Vertex::Occurrence(occurrence),
            &edges,
            pending,
            budget,
        )?;
        for child in unique {
            state.record(Undo::Parent(child))?;
            state.parents.insert(child, id);
        }
        state.originals = originals;
        Ok(id)
    }

    /// Publish only into a private state after every size/index/endpoint check.
    pub(crate) fn insert(
        &self,
        state: &mut State,
        vertex: Vertex,
        edges: &[(LogicalId, Edge)],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("vertex admission");
        let (kind, key) = vertex_key(&vertex, state, budget)?;
        let interned = !matches!(kind, VertexKind::Occurrence | VertexKind::Binder);
        if interned && let Some(id) = state.interned(&key, budget)? {
            return Ok(id);
        }
        let node_count = size(state.graph.node_count(), at)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?;
        let edge_count = size(state.graph.edge_count(), at)?
            .checked_add(size(edges.len(), at)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::Edges, at))?;
        if node_count >= u64::from(u32::MAX) || edge_count >= u64::from(u32::MAX) {
            return Err(Fault::capacity(BudgetKind::Vertices, at));
        }
        for (kind, count) in [
            (BudgetKind::Vertices, node_count),
            (BudgetKind::StagedVertices, node_count),
            (BudgetKind::Edges, edge_count),
            (BudgetKind::StagedEdges, edge_count),
        ] {
            storage_check(&self.limits, budget, kind, count, at)?;
        }
        let added = vertex_payload(&vertex, key.len(), edges.len(), budget)?;
        let payload = state
            .payload
            .checked_add(added)
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
        storage_check(&self.limits, budget, BudgetKind::PayloadBytes, payload, at)?;
        storage_check(
            &self.limits,
            budget,
            BudgetKind::StagedPayloadBytes,
            payload
                .checked_add(pending)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?,
            at,
        )?;
        let serial = state.next.get(&kind).copied().unwrap_or(0);
        let next = serial
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?;
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(edges.len())
            .map_err(|_| Fault::allocation(at))?;
        for (target, role) in edges {
            let index = state
                .indices
                .get(target)
                .copied()
                .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))?;
            targets.push((index, *role));
        }
        let id = LogicalId { kind, serial };
        state.record(Undo::Vertex {
            id,
            key: interned.then(|| key.clone()),
            policy: match &vertex {
                Vertex::Policy(value) => Some((value.key.clone(), value.version)),
                _ => None,
            },
        })?;
        let index = state.graph.add_node(vertex);
        for (target, role) in targets {
            let _edge = state.graph.add_edge(index, target, role);
        }
        state.indices.insert(id, index);
        state.next.insert(kind, next);
        if interned {
            state.intern.insert(key, id, &self.token);
        }
        state.payload = payload;
        Ok(id)
    }
}

/// Logical owned footprint of a record, shared by admission and visited-record metering.
pub(crate) fn vertex_payload(
    vertex: &Vertex,
    key_bytes: usize,
    edges: usize,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let at = Location::new("vertex payload");
    if let Vertex::AnonymousLemma(node) = vertex {
        return node.payload();
    }
    let certificate_bytes = match vertex {
        Vertex::ScopedApplication(value) => match &value.certificate {
            crate::proof_model::Certificate::SquareFree(certificate) => {
                certificate.payload(budget)?
            }
            _ => 0,
        },
        Vertex::Application(value) => match &value.certificate {
            Some(certificate) => certificate.payload(budget)?,
            None => 0,
        },
        _ => 0,
    };
    size(key_bytes, at)?
        .checked_mul(3)
        .and_then(|n| n.checked_add(256))
        .and_then(|n| size(edges, at).ok()?.checked_mul(32)?.checked_add(n))
        .and_then(|n| n.checked_add(certificate_bytes))
        .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))
}

/// Charge the key bytes and one map operation, rather than every retained entry.
fn lookup_work(bytes: usize, budget: &mut Budget, at: Location) -> Result<(), Fault> {
    let charge = size(bytes, at)?
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
    budget.work(charge, at)
}

/// Convert a host collection length without truncation.
pub(crate) fn size(value: usize, at: Location) -> Result<u64, Fault> {
    u64::try_from(value).map_err(|_| Fault::capacity(BudgetKind::PayloadBytes, at))
}

/// Enforce both the fixed store ceiling and the active call's possibly lower ceiling.
fn storage_check(
    limits: &Limits,
    budget: &Budget,
    kind: BudgetKind,
    needed: u64,
    at: Location,
) -> Result<(), Fault> {
    budget.check(kind, needed, at)?;
    let limit = limits.get(kind);
    if needed > limit {
        return Err(Fault::exhausted(kind, limit, needed, at));
    }
    Ok(())
}

/// Append a logical ID to a structural key without exposing graph indices.
pub(crate) fn encode_id(id: LogicalId, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.byte(id.kind.tag())?;
    out.unsigned(id.serial)
}

/// Convert a checked operand position to its wire/edge width.
pub(crate) fn position_u32(position: usize) -> Result<u32, Fault> {
    u32::try_from(position)
        .map_err(|_| Fault::capacity(BudgetKind::Edges, Location::new("operand position")))
}

/// Reconstruct a complete canonical record key from graph-owned weights.
pub(crate) fn vertex_key(
    vertex: &Vertex,
    _state: &State,
    budget: &mut Budget,
) -> Result<(VertexKind, Vec<u8>), Fault> {
    let kind = vertex_kind(vertex);
    let mut out = Encoder::new(budget, Location::new("structural key"));
    out.byte(kind.tag())?;
    match vertex {
        Vertex::AnonymousLemma(node) => node.encode(&mut out)?,
        Vertex::Solve(value) => value.encode(&mut out, &mut crate::statement::handle)?,
        Vertex::Header(_)
        | Vertex::Region(_)
        | Vertex::Space(_)
        | Vertex::CaseArm(_)
        | Vertex::Boundary(_)
        | Vertex::SourceForm(_) => {
            crate::domain_encoding::vertex(vertex, &mut out)?;
        }
        Vertex::Policy(policy) => policy.encode(&mut out)?,
        Vertex::ScopedApplication(application) => {
            out.byte(3)?;
            crate::statement::application(application, &mut out)?;
        }
        Vertex::Application(application) => {
            out.byte(0)?;
            crate::proof_storage::encode_application(application, &mut out)?;
        }
        Vertex::Proof(application) => encode_id(*application, &mut out)?,
        Vertex::PolicyShortcut {
            proof,
            cost,
            policy,
            baseline,
        } => {
            out.byte(2)?;
            encode_id(*proof, &mut out)?;
            encode_id(*policy, &mut out)?;
            out.unsigned(*cost)?;
            out.count(baseline.len())?;
            out.raw(baseline)?;
        }
        Vertex::Shortcut { proof, cost } => {
            out.byte(1)?;
            encode_id(*proof, &mut out)?;
            out.unsigned(*cost)?;
        }
        Vertex::Source(source) => {
            out.name(&source.key)?;
            out.count(source.text.len())?;
            out.raw(source.text.as_bytes())?;
        }
        Vertex::Context(context) => context_key(context, &mut out)?,
        Vertex::Condition(condition) => {
            encode_id(condition.header, &mut out)?;
            encoding::predicate(&condition.kind, &mut out)?;
            encoding::scope(&condition.scope, &mut out)?;
            out.count(condition.arguments.len())?;
            for argument in &condition.arguments {
                encode_id(*argument, &mut out)?;
            }
        }
        Vertex::Expression(expression) => expression_key(expression, &mut out)?,
        Vertex::Occurrence(occurrence) => occurrence_key(occurrence, &mut out)?,
        Vertex::Binder(binding) => encoding::binding(Some(binding), &mut out)?,
        Vertex::Request(request) => {
            crate::task::encode(&request.task, &mut out)?;
            out.count(request.premises.len())?;
            for premise in &request.premises {
                encode_id(*premise, &mut out)?;
            }
            encode_id(request.context, &mut out)?;
            out.count(request.inputs.len())?;
            for input in &request.inputs {
                encode_id(*input, &mut out)?;
            }
            for kind in crate::budget::ALL_KINDS {
                out.unsigned(request.limits.get(kind))?;
            }
        }
    }
    Ok((kind, out.finish()))
}

/// Classify a graph-owned record without exposing physical indices.
const fn vertex_kind(vertex: &Vertex) -> VertexKind {
    match vertex {
        Vertex::AnonymousLemma(_) => VertexKind::AnonymousLemma,
        Vertex::Solve(value) => value.kind(),
        Vertex::Header(_) => VertexKind::Header,
        Vertex::Region(_) => VertexKind::Region,
        Vertex::Space(_) => VertexKind::Space,
        Vertex::CaseArm(_) => VertexKind::CaseArm,
        Vertex::Boundary(_) => VertexKind::Boundary,
        Vertex::SourceForm(_) => VertexKind::SourceForm,
        Vertex::Source(_) => VertexKind::Source,
        Vertex::Context(_) => VertexKind::Context,
        Vertex::Condition(_) => VertexKind::Condition,
        Vertex::Expression(_) => VertexKind::Expression,
        Vertex::Occurrence(_) => VertexKind::Occurrence,
        Vertex::Binder(_) => VertexKind::Binder,
        Vertex::Request(_) => VertexKind::Request,
        Vertex::ScopedApplication(_)
        | Vertex::Application(_)
        | Vertex::Shortcut { .. }
        | Vertex::PolicyShortcut { .. } => VertexKind::Application,
        Vertex::Proof(_) => VertexKind::Proof,
        Vertex::Policy(_) => VertexKind::Policy,
    }
}

/// Encode every retained context identity field.
fn context_key(context: &Context, out: &mut Encoder<'_>) -> Result<(), Fault> {
    encode_id(context.header_id, out)?;
    encode_id(context.admissible, out)?;
    out.byte(u8::from(context.restriction.is_some()))?;
    if let Some(restriction) = context.restriction {
        encode_id(restriction, out)?;
    }
    out.byte(u8::from(context.parent.is_some()))?;
    if let Some(parent) = context.parent {
        encode_id(parent, out)?;
    }
    out.count(context.frame.len())?;
    for (name, predicate) in &context.frame {
        out.name(name)?;
        encode_id(*predicate, out)?;
    }
    out.raw(&context.header)?;
    out.count(context.assumptions.len())?;
    out.raw(&context.canonical_assumptions)?;
    if let Some(introduction) = context.introduction {
        out.byte(1)?;
        encode_id(introduction, out)?;
    }
    Ok(())
}

/// Encode every retained expression identity field.
fn expression_key(expression: &Expression, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.byte(u8::from(expression.ambient.is_some()))?;
    if let Some(ambient) = expression.ambient {
        encode_id(ambient, out)?;
    }
    crate::solving::graph::attachment_key(expression.attachment.as_ref(), out)?;
    encode_id(expression.header, out)?;
    out.byte(expression.layer.tag())?;
    out.byte(u8::from(expression.space.is_some()))?;
    if let Some(space) = expression.space {
        encode_id(space, out)?;
    }
    out.byte(u8::from(expression.domain.is_some()))?;
    if let Some(domain) = expression.domain {
        encode_id(domain, out)?;
    }
    out.count(expression.arms.len())?;
    for arm in &expression.arms {
        encode_id(*arm, out)?;
    }
    encoding::operator(&expression.operator, out)?;
    encoding::sort(&expression.sort, out)?;
    encoding::scope(&expression.scope, out)?;
    encoding::binding(expression.binding.as_ref(), out)?;
    out.count(expression.operands.len())?;
    for operand in &expression.operands {
        encode_id(*operand, out)?;
    }
    Ok(())
}

/// Encode every retained occurrence identity field.
fn occurrence_key(occurrence: &Occurrence, out: &mut Encoder<'_>) -> Result<(), Fault> {
    encode_id(occurrence.expression, out)?;
    for optional in [occurrence.source, occurrence.provenance] {
        out.byte(u8::from(optional.is_some()))?;
        if let Some(id) = optional {
            encode_id(id, out)?;
        }
    }
    out.byte(u8::from(occurrence.span.is_some()))?;
    if let Some(span) = occurrence.span {
        out.unsigned(span.start)?;
        out.unsigned(span.end)?;
    }
    out.count(occurrence.children.len())?;
    for child in &occurrence.children {
        encode_id(*child, out)?;
    }
    out.count(occurrence.binder_names.len())?;
    for name in &occurrence.binder_names {
        out.name(name)?;
    }
    Ok(())
}
