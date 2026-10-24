// SPDX-License-Identifier: AGPL-3.0-only
//! Completed receipts distinguish logical entries, primitive floats and actual owned capacity.

use crate::ExecutionId;
use nrz_bindings::readings::ReciprocalAllocation;
use nrz_scalar::Precision;

/// Opaque identity drawn before a private floating allocation is created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateAllocationTicket {
    /// Validated numerical execution, independent of a storage address.
    pub(super) execution: ExecutionId,
    /// Unique evaluation attempt, including retries after a numerical refusal.
    pub(super) candidate: u64,
    /// Position in the finite planned allocation inventory.
    pub(super) slot: usize,
}

impl PrivateAllocationTicket {
    /// Execution that sponsored this allocation.
    #[must_use]
    pub const fn execution(self) -> ExecutionId {
        self.execution
    }
}

/// One allocation's exact binding and historical owned payload, including auxiliary fields.
#[derive(Clone, Debug)]
pub struct PrivateAllocation {
    /// Original bindings-owned declaration and its closed numerical stage.
    pub(super) binding: ReciprocalAllocation,
    /// Unforgeable allocation authority consumed at creation.
    pub(super) ticket: PrivateAllocationTicket,
    /// Number of primitive floating components across every logical entry.
    pub(super) floats: usize,
    /// Actual vector capacity times the element size, including padding and nonfloat fields.
    pub(super) bytes: usize,
}

impl PrivateAllocation {
    /// Allocating binding; every entry and floating component has an exact interpretation.
    #[must_use]
    pub const fn binding(&self) -> &ReciprocalAllocation {
        &self.binding
    }
    /// Historical ticket; it never grants value access or reallocation authority.
    #[must_use]
    pub const fn ticket(&self) -> PrivateAllocationTicket {
        self.ticket
    }
    /// Logical entry count; an entry can contain several primitive floating components.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.binding.entries()
    }
    /// Primitive floating entries, including all enclosure endpoints and numerical radii.
    #[must_use]
    pub const fn float_entries(&self) -> usize {
        self.floats
    }
    /// Owned allocation bytes, excluding allocator overhead and shared exact declarations.
    #[must_use]
    pub const fn owned_bytes(&self) -> usize {
        self.bytes
    }
}

/// Complete reciprocal-kernel scratch receipts retained by a successful operation record.
///
/// All eight allocations have been released before the publication commit. Reserved output
/// zero buffers and backend destination slabs have separate existing covering inventories;
/// this scoped record does not claim an execution-wide or solve allocation inventory.
#[derive(Clone, Debug)]
pub struct ReciprocalInventory {
    /// Complete fixed schedule, in bindings stage order.
    pub(super) allocations: [PrivateAllocation; 8],
    /// Scalar format of every recorded numerical buffer.
    pub(super) precision: Precision,
    /// Largest simultaneous live numerical capacity in this scoped schedule.
    pub(super) peak: usize,
    /// Actual retained receipt allocation, excluding its owning pointer and shared declarations.
    pub(super) metadata: usize,
    /// Temporary plan allocation, including its reference-counted header and alignment.
    pub(super) planning: usize,
    /// Actual private output-container allocation, including its attached optional permits.
    pub(super) candidate: usize,
}

impl ReciprocalInventory {
    /// Every private kernel allocation, including candidate center, total and error arrays.
    #[must_use]
    pub const fn allocations(&self) -> &[PrivateAllocation; 8] {
        &self.allocations
    }
    /// Scalar format; decoding metadata is included in owned bytes separately from floats.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        self.precision
    }
    /// Largest simultaneously live numerical allocation capacity, excluding metadata.
    #[must_use]
    pub const fn peak_payload_bytes(&self) -> usize {
        self.peak
    }
    /// Bytes owned by the retained receipt box; shared exact declarations are not duplicated.
    #[must_use]
    pub const fn metadata_bytes(&self) -> usize {
        self.metadata
    }
    /// Bytes owned by the private candidate container, excluding its numerical arrays.
    #[must_use]
    pub const fn candidate_metadata_bytes(&self) -> usize {
        self.candidate
    }
    /// Bytes owned by the temporary reference-counted plan, including its aligned header.
    #[must_use]
    pub const fn planning_bytes(&self) -> usize {
        self.planning
    }
}
