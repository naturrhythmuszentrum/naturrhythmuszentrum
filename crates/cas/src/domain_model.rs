// SPDX-License-Identifier: AGPL-3.0-only
//! Typed semantic headers, domains and scoped context frames.

use crate::graph::{AssumptionDraft, Handle, LogicalId, Reference};
use crate::model::{Declaration, Environment, Field, Scope, Sort};

/// Mathematical interpretation of a scoped object.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Layer {
    /// Logical predicates and set descriptions.
    Logic,
    /// Classical pointwise values.
    Pointwise,
    /// Distributional values over a declared space.
    Distribution,
}

/// A semantic coordinate domain, independent of a context's assumptions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AmbientDomain {
    /// An explicitly ordered product of declared coordinates.
    Product {
        /// Earlier admitted product space.
        coordinates: Handle,
    },
    /// All assignments to the declared scalar coordinates.
    Assignments,
    /// One declared real coordinate.
    Real(String),
    /// One declared integer coordinate.
    Integer(String),
    /// An explicitly declared domain outside the decidable fragment.
    Opaque(String),
}

/// One fresh declaration whose sort refers to an earlier mathematical space.
#[derive(Clone, Debug)]
pub struct DeclarationSpaceDraft {
    /// Fresh declaration key.
    pub key: String,
    /// Earlier space defining this declaration's value category.
    pub space: Reference,
}

/// An acyclic semantic header, never a collection of assumptions.
#[derive(Clone, Debug)]
pub enum HeaderDraft {
    /// Existing space-free declaration language.
    Core(Environment),
    /// Conservative addition of fresh declarations to an earlier header.
    Extend {
        /// Earlier header whose signatures remain unchanged.
        parent: Reference,
        /// Fresh declarations, canonicalized by key.
        declarations: Vec<Declaration>,
        /// Exactly the space-dependent declarations' earlier spaces.
        spaces: Vec<DeclarationSpaceDraft>,
    },
}

/// Exact region syntax; construction alone proves no set relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionExpr<R = Reference> {
    /// The complete stated ambient domain.
    Ambient,
    /// The empty subset of that domain.
    Empty,
    /// Points where the predicate is defined and true within the base.
    GuardTrue {
        /// Region on which the predicate is reached.
        base: R,
        /// Boolean expression or condition.
        predicate: R,
    },
    /// Points where the predicate is defined and false within the base.
    GuardFalse {
        /// Region on which the predicate is reached.
        base: R,
        /// Boolean expression or condition.
        predicate: R,
    },
    /// Unordered finite intersection; the empty family denotes Ambient.
    Meet(Vec<R>),
    /// Unordered finite union; the empty family denotes Empty.
    Union(Vec<R>),
    /// Relative difference, without assuming a guard is total.
    Difference {
        /// Stated base set.
        base: R,
        /// Points excluded from that base.
        excluded: R,
    },
}

/// Untrusted exact set description under an immutable header.
#[derive(Clone, Debug)]
pub struct RegionDraft {
    /// Actual semantic header.
    pub header: Reference,
    /// Outer-to-inner binder frames.
    pub scope: Scope,
    /// Declared coordinate interpretation.
    pub ambient: AmbientDomain,
    /// Constructor and its earlier typed dependencies.
    pub expression: RegionExpr,
}

/// A declared space of distributions and their test functions.
#[derive(Clone, Debug)]
pub struct SpaceDraft {
    /// Space-free domain header.
    pub header: Reference,
    /// Open interval described by an earlier region.
    pub domain: Reference,
    /// Declared real coordinate.
    pub coordinate: String,
    /// Coefficient field.
    pub field: Field,
    /// Exact test-family identity.
    pub test_family: String,
}

/// One region/value association in an unordered native case.
#[derive(Clone, Debug)]
pub struct CaseArmDraft {
    /// Original local operands paired with their exact placement pieces.
    pub placements: Vec<crate::solving::PlacementUse>,
    /// Actual semantic header.
    pub header: Reference,
    /// Outer-to-inner binder frames.
    pub scope: Scope,
    /// Region where this value applies.
    pub region: Reference,
    /// Earlier scalar value expression.
    pub value: Reference,
}

/// A location shared by functions without sharing their ownership facts.
#[derive(Clone, Debug)]
pub struct BoundaryDraft {
    /// Actual semantic header.
    pub header: Reference,
    /// Outer-to-inner binder frames.
    pub scope: Scope,
    /// Coordinate interpretation.
    pub ambient: AmbientDomain,
    /// Earlier scalar location expression.
    pub location: Reference,
}

/// A root assumption set or a strictly nested local hypothesis frame.
#[derive(Clone, Debug)]
pub enum ContextDraft {
    /// Top-level mathematical assumptions remain visibly assumed.
    Root {
        /// Complete header used by the theorem.
        header: Reference,
        /// Stated domain on which results are requested.
        admissible: Reference,
        /// Explicit assumptions with provenance keys.
        assumptions: Vec<AssumptionDraft>,
    },
    /// A child frame restricted to the exact meet with its parent's domain.
    Local {
        /// Earlier enclosing context.
        parent: Reference,
        /// Additional region restriction.
        region: Reference,
        /// Additional predicates in stable caller slot order.
        hypotheses: Vec<AssumptionDraft>,
    },
}

/// Qualified hypothesis identity; equal slot numbers in siblings are different.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HypothesisRef {
    /// Context introducing the hypothesis frame.
    pub context: Handle,
    /// Zero is the local membership binder; predicate slots follow it.
    pub slot: u32,
}

/// Immutable graph-owned header with its complete declaration signature.
#[derive(Clone, Debug)]
pub(crate) struct Header {
    /// Installed scalar specification nodes, independent of distribution spaces.
    pub(crate) scalar_spaces: Vec<LogicalId>,
    /// Full inherited and fresh scalar signatures.
    pub(crate) environment: Environment,
    /// Canonical complete declaration signature.
    pub(crate) bytes: Vec<u8>,
    /// Earlier conservative parent, absent for Core.
    pub(crate) parent: Option<LogicalId>,
    /// Fresh declaration records for this header only.
    pub(crate) declarations: Vec<Declaration>,
    /// Fresh space-dependent declaration links.
    pub(crate) spaces: Vec<(String, LogicalId)>,
}

/// Graph-owned exact region syntax.
#[derive(Clone, Debug)]
pub(crate) struct Region {
    /// Exact family and base projection, when this is a total family region.
    pub(crate) lies_over: Option<crate::solving::LiesOver<LogicalId>>,
    /// Actual header retained in the interner key.
    pub(crate) header: LogicalId,
    /// Explicit bound frames.
    pub(crate) scope: Scope,
    /// Coordinate domain.
    pub(crate) ambient: AmbientDomain,
    /// Resolved earlier dependencies.
    pub(crate) expression: RegionExpr<LogicalId>,
}

/// Graph-owned declared mathematical space.
#[derive(Clone, Debug)]
pub(crate) struct Space {
    /// Earlier domain header.
    pub(crate) header: LogicalId,
    /// Earlier open interval region.
    pub(crate) domain: LogicalId,
    /// Declared coordinate key.
    pub(crate) coordinate: String,
    /// Coefficient field.
    pub(crate) field: Field,
    /// Test-family identity.
    pub(crate) test_family: String,
}

/// Graph-owned unordered case association.
#[derive(Clone, Debug)]
pub(crate) struct CaseArm {
    /// Original operand and placement-piece identities.
    pub(crate) placements: Vec<crate::solving::PlacementUse<LogicalId>>,
    /// Actual header.
    pub(crate) header: LogicalId,
    /// Explicit bound frames.
    pub(crate) scope: Scope,
    /// Selected region.
    pub(crate) region: LogicalId,
    /// Value expression.
    pub(crate) value: LogicalId,
    /// Checked scalar result sort.
    pub(crate) sort: Sort,
}

/// Graph-owned boundary location; ownership is a separate judgment.
#[derive(Clone, Debug)]
pub(crate) struct Boundary {
    /// Actual header.
    pub(crate) header: LogicalId,
    /// Explicit bound frames.
    pub(crate) scope: Scope,
    /// Coordinate interpretation.
    pub(crate) ambient: AmbientDomain,
    /// Location expression.
    pub(crate) location: LogicalId,
}

impl Layer {
    /// Canonical wire discriminant.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Logic => 0,
            Self::Pointwise => 1,
            Self::Distribution => 2,
        }
    }
}

/// An incident side of a boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    /// Coordinates below the boundary.
    Below,
    /// Coordinates above the boundary.
    Above,
}

impl Side {
    /// Stable diagnostic position.
    pub(crate) const fn tag(self) -> u32 {
        match self {
            Self::Below => 0,
            Self::Above => 1,
        }
    }
}

/// A native unordered case expression, with a domain that still requires cover evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasesDraft {
    /// Actual immutable header.
    pub header: Reference,
    /// Explicit binder scope.
    pub scope: Scope,
    /// Claimed mathematical domain, not a proved totality flag.
    pub domain: Reference,
    /// Exact arm set; admission only deduplicates identical members.
    pub arms: Vec<Reference>,
}
