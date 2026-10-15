// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded immutable provenance records with separate reverse-edge staleness caches.

mod access;
mod limits;
mod prepare;

pub use access::ProvenanceView;
pub(crate) use prepare::PreparedGraph;

pub use limits::{ProvenanceLimits, ProvenanceUsage};

use crate::{OperationRecord, RecordId, ValueStamp};
use std::cell::Cell;

/// Heap-scoped immutable output evidence identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvenanceId {
    /// Owning heap, checked before graph indexing.
    pub(crate) scope: u64,
    /// Append-only node position, never reused.
    pub(crate) index: usize,
}

/// Immutable output node; copy recipes have exactly one direct source dependency.
#[derive(Debug)]
pub(crate) struct Node {
    /// Operation holding the exact source and target dependency records.
    record: RecordId,
    /// Matching input/output slot in that operation.
    slot: usize,
    /// Earlier operation output, when the direct source was itself produced here.
    parent: Option<ProvenanceId>,
    /// Checked longest dependency depth, with imports at depth zero.
    depth: usize,
}

/// Explicit source-to-dependent edge, independent of numerical addresses.
#[derive(Debug)]
pub(crate) struct ReverseEdge {
    /// Exact source identity/version retained by the dependent node.
    source: ValueStamp,
    /// Dependent output evidence node, in topological append order.
    dependent: ProvenanceId,
}

/// Heap-owned graph; immutable nodes and records have separate monotone stale flags.
#[derive(Debug)]
pub(crate) struct Provenance {
    /// Explicit limits for this owner.
    limits: ProvenanceLimits,
    /// Checked retained metadata accounting.
    usage: ProvenanceUsage,
    /// Immutable output lineage nodes in topological order.
    nodes: Vec<Node>,
    /// Immutable complete operation records.
    records: Vec<OperationRecord>,
    /// Explicit source-to-dependent relations in the same order as nodes.
    reverse: Vec<ReverseEdge>,
    /// Mutable cache is separate from every immutable mathematical record.
    stale: Vec<Cell<bool>>,
}

impl Provenance {
    /// Empty owner; no records or identities are allocated by heap creation.
    pub(crate) fn new() -> Self {
        Self {
            limits: ProvenanceLimits::default(),
            usage: ProvenanceUsage::default(),
            nodes: Vec::new(),
            records: Vec::new(),
            reverse: Vec::new(),
            stale: Vec::new(),
        }
    }
}
