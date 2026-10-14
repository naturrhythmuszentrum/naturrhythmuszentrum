// SPDX-License-Identifier: AGPL-3.0-only
//! Typed untrusted applications and sealed scoped mathematical judgments.

use crate::domain_model::{HypothesisRef, Layer, Side};
use crate::graph::{Handle, LogicalId};
use crate::model::{Field, Scope};
use crate::region::RegionCertificate;
use crate::rule::Primitive;
use crate::substitution::Substitution;

/// The definedness contract of a pointwise equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainContract {
    /// Both endpoints are defined and equal on the stated region under explicit conditions.
    DefinedAndEqual,
    /// Both definedness directions and equality on the common domain are established.
    SameDefinedDomain,
}

/// Exact constructor semantics without a dummy scalar equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructorLaw {
    /// The region's checked constructor defines its membership relation.
    Membership,
}

/// Function-specific ownership at one shared boundary location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ownership {
    /// A proved value at the location.
    Present(Handle),
    /// Proved exclusion from the function's defined domain.
    Absent,
}

/// A mathematical statement with exactly the objects needed by its proposition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    /// Classical equality with explicit domain and definedness strength.
    Equality {
        /// Left value expression.
        left: Handle,
        /// Right value expression.
        right: Handle,
        /// Region on which the contract is asserted.
        domain: Handle,
        /// Required definedness strength.
        domain_contract: DomainContract,
    },
    /// One predicate is defined and true throughout a region.
    PredicateFact {
        /// Region of validity.
        region: Handle,
        /// Exact condition syntax.
        predicate: Handle,
    },
    /// Every point of the first region belongs to the second.
    Subset {
        /// Region being restricted.
        subset: Handle,
        /// Containing region.
        superset: Handle,
    },
    /// Two regions have empty intersection.
    Disjoint {
        /// First region.
        left: Handle,
        /// Second region.
        right: Handle,
    },
    /// A complete finite family covers the stated domain.
    Cover {
        /// Covered region.
        domain: Handle,
        /// Unordered cover members, including any singleton endpoints.
        members: Vec<Handle>,
    },
    /// A region has no points.
    Empty {
        /// Empty region.
        region: Handle,
    },
    /// Exact constructor membership semantics.
    Constructor {
        /// Constructed region.
        region: Handle,
        /// Closed defining law.
        law: ConstructorLaw,
    },
    /// The exact domain on which a function has a value.
    DefinedDomain {
        /// Function expression.
        function: Handle,
        /// Exact defined domain within the context's admissible region.
        domain: Handle,
    },
    /// Applicable arms have matching values and definedness.
    Compatibility {
        /// First association.
        left: Handle,
        /// Second association.
        right: Handle,
        /// Region on which compatibility is claimed.
        region: Handle,
    },
    /// An original semantic form constructs the specified native case.
    Elaboration {
        /// Ordered source-form signature.
        source: Handle,
        /// Resulting native expression.
        result: Handle,
        /// Claimed domain.
        domain: Handle,
    },
    /// One side of a location has an incident neighborhood in a region.
    Incidence {
        /// Incident region.
        region: Handle,
        /// Boundary location.
        boundary: Handle,
        /// Oriented side.
        side: Side,
        /// Proved incident neighborhood.
        neighborhood: Handle,
    },
    /// The function owns a value at a boundary, or is proved absent there.
    Ownership {
        /// Function whose domain is being described.
        function: Handle,
        /// Shared boundary location.
        boundary: Handle,
        /// Checked presence or checked absence.
        value: Ownership,
    },
    /// A classical scalar derivative on a stated region.
    DerivativeOn {
        /// Input function.
        function: Handle,
        /// Derivative expression.
        derivative: Handle,
        /// Exact coordinate declaration key.
        coordinate: String,
        /// Region of the theorem.
        domain: Handle,
    },
    /// Ordered Jacobian assembly from checked scalar partials.
    Jacobian {
        /// Ordered input column.
        input: Handle,
        /// Ordered result matrix.
        output: Handle,
        /// Ordered distinct coordinate keys.
        variables: Vec<String>,
        /// Region of the theorem.
        domain: Handle,
    },
    /// Certified exact rational multiplicity decomposition.
    SquareFree {
        /// Polynomial input.
        input: Handle,
        /// Displayed factor expression.
        output: Handle,
        /// Exact declared variable.
        variable: String,
        /// Coefficient field.
        field: Field,
        /// Region retaining all strict input requirements.
        domain: Handle,
    },
    /// A represented side jet; its producer belongs to the following lane.
    Jet {
        /// Function with the stated trace.
        function: Handle,
        /// Region carrying that function germ.
        region: Handle,
        /// Boundary location.
        boundary: Handle,
        /// Oriented side.
        side: Side,
        /// Derivative values in order, beginning with the trace.
        values: Vec<Handle>,
    },
    /// A represented jump; no missing side is treated as zero.
    Jump {
        /// Function whose jump is stated.
        function: Handle,
        /// Boundary location.
        boundary: Handle,
        /// Derivative order.
        order: u32,
        /// Right trace minus left trace.
        value: Handle,
    },
}

/// A predicate required throughout a region under one exact home context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardedObligation {
    /// Context under which the predicate must hold.
    pub home: Handle,
    /// Explicit binder scope of the requirement.
    pub scope: Scope,
    /// Mathematical interpretation.
    pub layer: Layer,
    /// Exact mathematical space, absent for space-free statements.
    pub space: Option<Handle>,
    /// Region on which the predicate is required.
    pub region: Handle,
    /// Exact condition syntax, including its actual header.
    pub predicate: Handle,
}

/// The complete theorem a caller intends to accept.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Judgment {
    /// Full immutable hypothesis frame.
    pub context: Handle,
    /// Mathematical interpretation.
    pub layer: Layer,
    /// Exact mathematical space, explicitly absent where unnecessary.
    pub space: Option<Handle>,
    /// Typed proposition.
    pub statement: Statement,
    /// Canonical conjunction of still-open guarded predicates.
    pub residuals: Vec<GuardedObligation>,
}

/// Independently checked evidence tied to a closed primitive schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Certificate {
    /// This primitive has no auxiliary payload.
    None,
    /// Exact interval-cell certificate, never a trusted truth flag.
    Region(RegionCertificate),
    /// One qualified ancestor/current hypothesis reference.
    Hypothesis(HypothesisRef),
    /// Exact child frame being closed.
    Close {
        /// Local context introducing the discharged slots.
        frame: Handle,
    },
    /// One ordered operand path for scalar congruence.
    Congruence(Vec<u32>),
    /// Exact rational multiplicity evidence.
    SquareFree(Box<crate::multiplicity::SquareFreeCertificate>),
}

/// One untrusted application whose local premise indices are strictly backward.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationDraft {
    /// Compiled schema, never a user callback.
    pub rule: Primitive,
    /// Exact context in which this application is checked.
    pub context: Handle,
    /// Claimed typed statement reconstructed by the checker.
    pub conclusion: Statement,
    /// Ordered earlier applications in this supplied proof DAG.
    pub premises: Vec<u32>,
    /// Simultaneous substitutions, admitted only by their schema.
    pub substitutions: Vec<Substitution>,
    /// Schema-specific untrusted evidence.
    pub certificate: Certificate,
}

/// Conditions reconstructed by the scoped kernel, retained separately from syntax.
#[derive(Clone, Debug, Default)]
pub(crate) struct ScopedConditions {
    /// Every required fact, including those already discharged.
    pub(crate) requirements: Vec<GuardedObligation>,
    /// Facts that remain open.
    pub(crate) residuals: Vec<GuardedObligation>,
    /// Qualified hypothesis identities actually used.
    pub(crate) hypotheses: Vec<HypothesisRef>,
}

/// Authoritative graph-owned application for non-scalar typed statements.
#[derive(Clone, Debug)]
pub(crate) struct CheckedApplication {
    /// Derived scalar matcher data for executable scalar rules, never wire authority.
    pub(crate) scalar: Option<crate::scalar_model::CheckedStep>,
    /// Closed registered schema.
    pub(crate) rule: Primitive,
    /// Complete reconstructed theorem.
    pub(crate) judgment: Judgment,
    /// Earlier proof graph vertices in premise order.
    pub(crate) premises: Vec<LogicalId>,
    /// Checked simultaneous substitutions.
    pub(crate) substitutions: Vec<Substitution>,
    /// Independently checked auxiliary payload.
    pub(crate) certificate: Certificate,
    /// All requirements and qualified used hypotheses.
    pub(crate) conditions: ScopedConditions,
    /// Exact proof depth.
    pub(crate) depth: u64,
}

/// Sealed evidence minted only by checking, canonical encoding and atomic publication.
#[derive(Clone, Debug)]
pub struct VerifiedWitness {
    /// Store-scoped checked proof root.
    pub(crate) proof: Handle,
    /// Complete typed theorem.
    pub(crate) judgment: Judgment,
    /// Exact qualified assumption provenance.
    pub(crate) hypotheses: Vec<HypothesisRef>,
    /// Canonical portable bytes, with no original traversal receipt.
    pub(crate) bytes: Vec<u8>,
}

impl VerifiedWitness {
    /// Borrow the complete typed theorem, including every guarded residual.
    #[must_use]
    pub const fn judgment(&self) -> &Judgment {
        &self.judgment
    }

    /// Borrow every qualified hypothesis actually used by the proof.
    #[must_use]
    pub fn hypotheses(&self) -> &[HypothesisRef] {
        &self.hypotheses
    }

    /// Whether no residual remains; root assumptions still remain assumptions.
    #[must_use]
    pub const fn is_discharged(&self) -> bool {
        self.judgment.residuals.is_empty()
    }

    /// Read the store-scoped proof root without exposing its private constructor.
    #[must_use]
    pub const fn proof(&self) -> &Handle {
        &self.proof
    }

    /// General consistency remains unproved even for discharged results.
    #[must_use]
    pub const fn context_consistency(&self) -> crate::request_model::ContextConsistency {
        crate::request_model::ContextConsistency::Unknown
    }
}
