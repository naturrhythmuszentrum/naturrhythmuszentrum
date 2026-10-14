// SPDX-License-Identifier: AGPL-3.0-only
//! Exact, resource-bounded mathematical graph construction.
//!
//! The construction layer preserves distinct original occurrences and checked scopes.
//! Its coverage receipts never establish a mathematical identity or physical validity.
//! Algebraic algorithms and portable proof checking are admitted in later capability steps.

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
mod context;
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
