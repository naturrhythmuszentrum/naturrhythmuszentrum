// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable declaration evidence and context-scoped semantic revisions.

mod context;
mod frozen;
mod identity;

pub(crate) use context::next_revision;
pub use frozen::CheckedDeclarations;
pub use identity::{SnapshotError, SnapshotId};
