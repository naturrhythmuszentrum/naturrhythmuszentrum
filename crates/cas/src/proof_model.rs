// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted proof inputs and sealed checked witnesses.

use std::fmt;

use crate::conditions::{Conditions, GuardKind};
use crate::graph::{Handle, LogicalId};
use crate::rule::Primitive;
use crate::substitution::Substitution;

/// Admitted mathematical relations; a derivative is never an equality edge.
#[derive(Clone, Eq, PartialEq)]
pub enum Relation {
    /// Equality on the retained common defined domain.
    EqualOnDomain,
    /// Scalar derivative with respect to one declared real variable.
    Derivative {
        /// Declaration key, not its display name.
        variable: String,
    },
    /// Reserved square-free product relation; no primitive checker is installed yet.
    SquareFree {
        /// Declared polynomial variable.
        variable: String,
        /// Exact coefficient field required by a future certificate.
        field: crate::model::Field,
    },
    /// Jacobian of a column of scalar outputs, with ordered independent variables.
    Jacobian {
        /// Distinct declaration keys in column order.
        variables: Vec<String>,
    },
}

/// A visible condition on an exact expression, including its bound scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guard {
    /// Required mathematical predicate.
    pub kind: GuardKind,
    /// Expression to which it applies.
    pub term: Handle,
}

/// The exact theorem a caller intends to accept, including residual obligations.
#[derive(Clone, Eq, PartialEq)]
pub struct Claim {
    /// Complete immutable context, including hypothesis provenance.
    pub context: Handle,
    /// Mathematical relation and its ordered payload.
    pub relation: Relation,
    /// Function/input or equality's left endpoint.
    pub left: Handle,
    /// Output or equality's right endpoint.
    pub right: Handle,
    /// Exact residual set; comparison is independent of this vector's order.
    pub residuals: Vec<Guard>,
}

/// One untrusted application; every referenced premise must precede it.
#[derive(Clone, Eq, PartialEq)]
pub struct ProofStep {
    /// Compiled schema, never an arbitrary assertion callback.
    pub primitive: Primitive,
    /// Claimed relation reconstructed by the checker.
    pub relation: Relation,
    /// Claimed input endpoint.
    pub left: Handle,
    /// Claimed output endpoint.
    pub right: Handle,
    /// Ordered zero-based proof indices, strictly backward.
    pub premises: Vec<u32>,
    /// Congruence operand path; empty for every other schema.
    pub path: Vec<u32>,
    /// Simultaneous replacements; empty except for the substitution schema.
    pub substitutions: Vec<Substitution>,
}

impl fmt::Debug for Relation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EqualOnDomain => formatter.write_str("EqualOnDomain"),
            Self::Derivative { variable } => formatter
                .debug_struct("Derivative")
                .field("variable_bytes", &variable.len())
                .finish_non_exhaustive(),
            Self::Jacobian { variables } => formatter
                .debug_struct("Jacobian")
                .field("variables", &variables.len())
                .finish_non_exhaustive(),
            Self::SquareFree { variable, .. } => formatter
                .debug_struct("SquareFree")
                .field("variable_bytes", &variable.len())
                .finish_non_exhaustive(),
        }
    }
}

impl fmt::Debug for Claim {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Claim")
            .field("context", &self.context)
            .field("relation", &self.relation)
            .field("left", &self.left)
            .field("right", &self.right)
            .field("residuals", &self.residuals.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ProofStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProofStep")
            .field("primitive", &self.primitive)
            .field("relation", &self.relation)
            .field("left", &self.left)
            .field("right", &self.right)
            .field("premises", &self.premises.len())
            .field("path_depth", &self.path.len())
            .field("substitutions", &self.substitutions.len())
            .finish_non_exhaustive()
    }
}

/// A checked step whose mathematical payload is owned by an Application graph vertex.
#[derive(Clone, Debug)]
pub(crate) struct CheckedStep {
    /// Primitive schema.
    pub(crate) primitive: Primitive,
    /// Reconstructed relation.
    pub(crate) relation: Relation,
    /// Full context.
    pub(crate) context: LogicalId,
    /// Input expression.
    pub(crate) left: LogicalId,
    /// Output expression.
    pub(crate) right: LogicalId,
    /// Earlier proof vertices, filled only during atomic publication.
    pub(crate) premises: Vec<LogicalId>,
    /// Checked ordered operand path.
    pub(crate) path: Vec<u32>,
    /// Checked simultaneous replacement identities.
    pub(crate) substitutions: Vec<(String, LogicalId)>,
    /// Independently reconstructed residuals and hypothesis keys.
    pub(crate) conditions: Conditions,
    /// Checked proof depth.
    pub(crate) depth: u64,
    /// Positive known-route cost under logical-v1.
    pub(crate) cost: u64,
}

/// A compiled batch before logical proof IDs are assigned.
#[derive(Clone, Debug)]
pub(crate) struct Compiled {
    /// Mathematically checked applications.
    pub(crate) steps: Vec<CheckedStep>,
    /// Earlier local indices corresponding to each step's ordered premises.
    pub(crate) premises: Vec<Vec<u32>>,
}

/// Sealed evidence minted only after complete replay and atomic graph publication.
#[derive(Clone)]
pub struct VerifiedWitness {
    /// Store-scoped proof root, inaccessible to untrusted constructors.
    pub(crate) proof: Handle,
    /// Exact accepted theorem.
    pub(crate) claim: Claim,
    /// Explicit hypotheses used to discharge required domain predicates.
    pub(crate) hypotheses: Vec<String>,
    /// Canonical independently reconstructible proof bytes.
    pub(crate) bytes: Vec<u8>,
}

impl fmt::Debug for VerifiedWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWitness")
            .field("proof", &self.proof)
            .field("residual_count", &self.claim.residuals.len())
            .field("hypothesis_count", &self.hypotheses.len())
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl VerifiedWitness {
    /// Borrow the exact checked theorem and all residual obligations.
    #[must_use]
    pub const fn claim(&self) -> &Claim {
        &self.claim
    }

    /// Borrow the exact context hypotheses used; consistency remains unknown.
    #[must_use]
    pub fn hypotheses(&self) -> &[String] {
        &self.hypotheses
    }

    /// Whether all required guards were discharged, possibly relative to Gamma.
    #[must_use]
    pub const fn is_discharged(&self) -> bool {
        self.claim.residuals.is_empty()
    }

    /// General satisfiability of Gamma remains unknown even for a discharged theorem.
    #[must_use]
    pub const fn context_consistency(&self) -> crate::request::ContextConsistency {
        crate::request::ContextConsistency::Unknown
    }

    /// Read the checked proof's store-scoped graph handle.
    #[must_use]
    pub const fn proof(&self) -> &Handle {
        &self.proof
    }
}
