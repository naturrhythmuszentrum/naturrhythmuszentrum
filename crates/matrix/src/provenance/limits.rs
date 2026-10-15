// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit provenance capacities, independent of numerical truth or heap addresses.

use crate::OperationError;

/// Maximum immutable graph depth, records, edges, accounted bytes and work per access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvenanceLimits {
    /// Longest operation lineage, with an imported input at depth zero.
    pub(crate) depth: usize,
    /// Operation records plus output evidence nodes retained for the heap lifetime.
    pub(crate) records: usize,
    /// Source, target, operation and parent relations retained for the heap lifetime.
    pub(crate) edges: usize,
    /// Accounted graph metadata bytes, excluding shared immutable schema owners.
    pub(crate) bytes: usize,
    /// Graph entries inspected in one preparation, validation or invalidation traversal.
    pub(crate) work: usize,
}

impl ProvenanceLimits {
    /// Set all capacities; a zero capacity refuses any request that needs that resource.
    #[must_use]
    pub const fn new(
        depth: usize,
        records: usize,
        edges: usize,
        bytes: usize,
        work: usize,
    ) -> Self {
        Self {
            depth,
            records,
            edges,
            bytes,
            work,
        }
    }

    /// Check prospective lifetime usage before reserving or publishing graph objects.
    pub(crate) const fn check(self, usage: ProvenanceUsage) -> Result<(), OperationError> {
        if usage.depth > self.depth
            || usage.records > self.records
            || usage.edges > self.edges
            || usage.bytes > self.bytes
            || usage.work > self.work
        {
            Err(OperationError::ProvenanceLimit)
        } else {
            Ok(())
        }
    }
}

impl Default for ProvenanceLimits {
    fn default() -> Self {
        Self::new(1024, 16_384, 65_536, 16_777_216, 1_048_576)
    }
}

/// Immutable graph accounting, available even when a later request refuses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProvenanceUsage {
    /// Greatest retained lineage depth.
    pub(crate) depth: usize,
    /// Operation records and output nodes.
    pub(crate) records: usize,
    /// Explicit retained relations.
    pub(crate) edges: usize,
    /// Fixed graph metadata and conservatively charged logical entry maps.
    pub(crate) bytes: usize,
    /// Largest full-graph traversal charged by the retained graph.
    pub(crate) work: usize,
}

impl ProvenanceUsage {
    /// Greatest retained operation depth.
    #[must_use]
    pub const fn depth(self) -> usize {
        self.depth
    }
    /// Number of retained operation records plus output evidence nodes.
    #[must_use]
    pub const fn records(self) -> usize {
        self.records
    }
    /// Number of explicit graph edges.
    #[must_use]
    pub const fn edges(self) -> usize {
        self.edges
    }
    /// Accounted metadata bytes; shared schema storage is bounded separately.
    #[must_use]
    pub const fn bytes(self) -> usize {
        self.bytes
    }
    /// Required full-graph traversal work.
    #[must_use]
    pub const fn work(self) -> usize {
        self.work
    }
}

/// Checked accounting arithmetic never wraps into an apparently small request.
pub(crate) fn add(left: usize, right: usize) -> Result<usize, OperationError> {
    left.checked_add(right)
        .ok_or(OperationError::ProvenanceLimit)
}
