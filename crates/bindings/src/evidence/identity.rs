// SPDX-License-Identifier: AGPL-3.0-only
//! Exact snapshot identity without hashes, addresses or portable identity claims.

use nrz_cas::Handle;

/// A process-local context and its complete physical semantic revision.
///
/// Equality includes the CAS store identity, not just its logical context number.
/// The revision covers declarations, their evidence, admitted kinds and function
/// signatures. Initial registry rules are owned exclusively by that context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotId {
    /// Store-scoped mathematical context; the handle also prevents identity reuse.
    context: Handle,
    /// Monotonic semantic revision within that context.
    revision: u64,
}

impl SnapshotId {
    /// Capture the builder's current semantic state.
    pub(crate) const fn new(context: Handle, revision: u64) -> Self {
        Self { context, revision }
    }

    /// Semantic revision; meaningful only together with this ID's context identity.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Require evidence captured under this exact context and semantic revision.
    ///
    /// # Errors
    /// Distinguishes a foreign context from an obsolete revision of the same context.
    pub fn validate(&self, captured: &Self) -> Result<(), SnapshotError> {
        if self.context != captured.context {
            Err(SnapshotError::ForeignContext)
        } else if self.revision != captured.revision {
            Err(SnapshotError::StaleContext)
        } else {
            Ok(())
        }
    }
}

/// Snapshot lifecycle refusals, independent of numerical values and heap versions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SnapshotError {
    /// Equal declarations in another CAS context cannot authorize this context.
    #[error("checked evidence belongs to another bindings context")]
    ForeignContext,
    /// A successful semantic change requires checking the original expression again.
    #[error("checked evidence belongs to a stale bindings context revision")]
    StaleContext,
    /// Semantic revisions never wrap or reuse an earlier identity.
    #[error("bindings context semantic revision exhausted")]
    RevisionExhausted,
}
