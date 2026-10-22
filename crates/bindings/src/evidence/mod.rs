// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable declaration evidence and context-scoped semantic revisions.
//!
//! # Frozen declarations and semantic revisions
//!
//! `Context::freeze` checks every numerical declaration through `check_all`, including unused bodies, original operands hidden under zero or cancellation, and companion relationships. It consumes the builder into `CheckedDeclarations`, which retains the actual CAS store, original occurrences, registry, function signatures, metadata, declared provenance and immutable checking results. Read-only accessors expose that inventory and permit original-syntax inspection through `cas`; no mutable semantic context escapes. A snapshot establishes descriptor validity, not numerical assurance or source independence.
//!
//! `SnapshotId` combines the CAS context identity with a monotonic semantic revision. The context exclusively owns its initial operation registry; each successful declaration or function addition advances the revision covering its rules, descriptors and evidence. Failed additions leave it unchanged, and exhaustion refuses before mutation. Parsing and checking do not change the semantic revision. Every `CheckedExpression` captures this ID. `validate_checked` distinguishes foreign contexts from stale revisions, and `prove_equal` refuses both before constructing a new request. Equal schemas in another context never identify the same snapshot. IDs and cloned checks alone do not retain graph storage: clients keep the frozen owner while inspecting originals or witnesses. This adds a reusable evidence boundary without changing existing parse, declaration, checking or docgen call signatures (see `docs/design/matrix-core-design.md`).

mod context;
mod frozen;
mod identity;

pub(crate) use context::next_revision;
pub use frozen::CheckedDeclarations;
pub use identity::{SnapshotError, SnapshotId};
