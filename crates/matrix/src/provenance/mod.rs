// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded immutable provenance records with separate reverse-edge staleness caches.
//!
//! # Dependency graph and current evidence
//!
//! The heap owns an immutable provenance DAG with separate monotone staleness flags. Each output node links its exact source selection, target version, operation record and every earlier output parent used by that output. `ProvenanceView::sources` and `parents` expose its complete ordered source closure and parent list. Two outputs copied from the same source retain that relation. Sibling outputs have separate dependency keys. Reverse source edges are traversed in topological order on evidence access, comparing value occupations/versions, schemas, layouts, precision and companion occupations/targets, and propagating stale parent state. A companion-only input change or any alias-wide base replacement invalidates its downstream claims even with identical selected center bits. Unrelated siblings remain readable. Compaction retains mathematical lineage while refreshing physical indirection.
//!
//! `Heap::operation_record` checks the complete current operation; `Heap::provenance` checks an individual output node. `Heap::historical_record` retains immutable facts after sources change or retire and cannot authorize current access. The existing `Heap::value` and `Heap::companion` accessors also reject stale derived evidence. Record and node IDs are scoped to the heap. Reused descriptor slots cannot revive old source relations, and previously consumed candidate tokens cannot be replayed.
//!
//! `ProvenanceLimits` bounds lineage depth, retained operation records plus output nodes, explicit source/target/operation/parent edges, accounted graph metadata bytes, and traversal/parent-search work. Limits refuse before graph publication. `provenance_usage` exposes retained accounting; `set_provenance_limits` refuses policies smaller than current usage. Byte accounting includes fixed record objects and conservatively charges full logical-map offsets, while immutable checked shape/declaration owners remain governed by their separate shape and bindings limits. It is metadata accounting, not a measurement of allocator overhead or peak process memory. Retained historical records count toward lifetime capacities. Failed publication leaves these accounts unchanged.
//!
//! The public conformance target also checks offline locked Cargo dependency trees in Rust for every supported feature configuration, including development, build and target edges. The acceptance target compares numerical inspection transcripts byte for byte across repeated executions and fresh heaps. Raw record identities remain fresh. The acceptance record in `docs/records/matrix-core-acceptance.md` documents the server environment, measured costs, assurance and current admission boundaries.

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
    parents: Vec<ProvenanceId>,
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
    /// Operation-ordered source slot, independent of the output position.
    input: usize,
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
