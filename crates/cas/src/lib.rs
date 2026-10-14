// SPDX-License-Identifier: AGPL-3.0-only
//! Exact mathematical graph construction and bounded conditional proof replay.
//!
//! The construction layer preserves distinct original occurrences and checked scopes.
//! Its coverage receipts never establish a mathematical identity or physical validity.
//! A closed primitive kernel checks supplied identities and derivative premises.
//! Bounded identity search, scalar derivatives and ordered Jacobians produce replayed results.
//! Square-free polynomial certificates remain a later capability.

#![expect(
    clippy::multiple_crate_versions,
    reason = "petgraph 0.8.3, the newest release, depends on hashbrown 0.15 while its indexmap dependency uses hashbrown 0.17, so no single-version resolution exists"
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod budget;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod conditions;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod context;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod derivative_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod encoding;
mod error;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod graph;
mod model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod polynomial;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod proof;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod proof_model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod proof_storage;
mod rule;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod rule_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod wire;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod wire_import;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod wire_read;

mod number;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod request;
mod snapshot;
mod substitution;
mod traversal;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod validation;

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod calculus;
mod cost;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod frontier;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod producer;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod rewrite;

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod frozen;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod known;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod search;
mod service;
mod shortcut;

pub use budget::{Budget, BudgetKind, BudgetUsage, Limits, LimitsOverride};
pub use error::{Exhaustion, Fault, FaultKind, InvalidCode, Location};
pub use graph::{
    AssumptionDraft, Cas, Draft, ExpressionDraft, GraphCounts, Handle, LogicalId, OccurrenceDraft,
    Reference, VertexKind,
};
pub use model::{
    Approach, AtomicSort, Binding, Branch, Constant, Declaration, Environment, Extent, Field,
    MatrixOperation, Operation, Operator, PredicateKind, RootSelection, ScalarOperation, Scope,
    Sort, Span, SpectralKind,
};
pub use number::Rational;
pub use request::{ContextConsistency, Outcome, Request, UnsupportedReason};
pub use substitution::Substitution;
pub use traversal::{
    CompleteWalk, SourceLocation, TraversalReceipt, Visit, WalkEvent, WalkFailure,
};

pub use conditions::{ConditionStatus, GuardKind};
pub use proof_model::{Claim, Guard, ProofStep, Relation, VerifiedWitness};
pub use rule::{PRIMITIVES, Primitive, RuleSpec};

pub use cost::CostPolicy;

pub use frozen::FrozenCertificate;
pub use search::{SearchPolicy, SearchResult, SearchStatistics};
