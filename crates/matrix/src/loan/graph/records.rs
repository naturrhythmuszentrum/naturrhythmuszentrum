// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed metadata records reserve incidence, epochs and cursors without executing them.

/// All fields are inline; links never allocate hidden adjacency vectors.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub(super) struct Node {
    /// Original occupation; zero denotes an unoccupied reserved slot.
    pub(super) generation: u64,
    /// Remaining caller-supplied attempts.
    pub(super) attempts: u64,
    /// Notification, visitation, capacity-epoch and wait-membership serials.
    pub(super) epochs: [u64; 4],
    /// Incoming/outgoing heads, wait neighbours, free link and saved transition cursors.
    pub(super) links: [usize; 8],
    /// Closed state flags, with zero meaning privately reserved.
    pub(super) flags: u64,
    /// Unsettled dependencies and retained lifetime uses, kept distinct.
    pub(super) counts: [usize; 2],
}
/// Fixed endpoint/generation pair rather than an address into a movable vector.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub(super) struct Endpoint {
    /// Node slot.
    pub(super) slot: usize,
    /// Original occupation stamp.
    pub(super) generation: u64,
}
/// Edge storage includes both incidence directions exactly once.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub(super) struct Edge {
    /// Producer and consumer identities within this reservation.
    pub(super) endpoints: [Endpoint; 2],
    /// Predecessor/successor links for each incidence direction.
    pub(super) links: [usize; 4],
    /// Index into separately owned original demand authority; no authority is fabricated here.
    pub(super) demand: usize,
    /// Dependency settlement and lifetime release remain separate flags.
    pub(super) flags: u64,
}
/// One bounded DFS frame per active node, without a host recursion stack.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub(super) struct Frame {
    /// Current node.
    pub(super) node: usize,
    /// Next outgoing edge.
    pub(super) edge: usize,
}
/// Scope control metadata; its packed membership is separately inventoried.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub(super) struct Scope {
    /// Nonwrapping scope stamp, zero before notification exists.
    pub(super) generation: u64,
    /// Root slot and resumable control cursor.
    pub(super) cursors: [usize; 2],
    /// Membership offset in the fixed packed backing.
    pub(super) membership: usize,
}

/// Fixed owner control state is funded before W can perform any transition.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub(super) struct Control {
    /// Cyclic ready-slot cursor.
    pub(super) ready: usize,
    /// Wait-list head, tail and resumed scan cursor.
    pub(super) waiting: [usize; 3],
    /// Notification, visitation, scope, capacity and membership serials.
    pub(super) epochs: [u64; 5],
    /// Upper membership serial captured by a bounded wait scan.
    pub(super) watermark: u64,
    /// Pending invalidation root and scope traversal cursor.
    pub(super) pending: [usize; 2],
    /// Node and edge free-list heads, never an implicit historical scan.
    pub(super) free: [usize; 2],
    /// Private/stopped/control-in-progress flags, initially zero.
    pub(super) flags: u64,
}
