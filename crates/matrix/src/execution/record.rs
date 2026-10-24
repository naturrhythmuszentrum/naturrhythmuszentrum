// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable resource facts contain no strong numerical ownership.

use crate::{ExecutionId, PrivateAllocationTicket};

/// Original factor reservation retained by one execution without duplicating its backend.
#[derive(Clone, Debug)]
pub struct SharedFactorAllocation {
    /// Original complete allocating definitions and observations, without numerical owners.
    pub(super) inventory: std::rc::Rc<crate::FactorInventory>,
}
impl SharedFactorAllocation {
    /// Original factor allocation authority shared by every admitted consumer.
    #[must_use]
    pub fn ticket(&self) -> PrivateAllocationTicket {
        self.inventory.allocations()[1].ticket()
    }
    /// Original faer storage/workspace reservation, counted once per distinct ticket.
    #[must_use]
    pub fn backend_allowance_bytes(&self) -> usize {
        self.inventory.backend_allowance_bytes()
    }
    /// Shared complete original construction inventory, independent of numerical lifetime.
    #[must_use]
    pub fn allocation_inventory(&self) -> &crate::FactorInventory {
        &self.inventory
    }
}
/// Frozen common-lifetime observations and distinct resource associations.
#[derive(Clone, Debug)]
pub struct ExecutionInventory {
    /// Original validated execution.
    pub(super) execution: ExecutionId,
    /// Distinct original factor resources, with no numerical references.
    pub(super) factors: Vec<SharedFactorAllocation>,
    /// Actual observed allocation requests in the covered scoped plans.
    pub(super) allocations: usize,
    /// Cumulative observed vector capacities, separate from live peak.
    pub(super) bytes: usize,
    /// Largest simultaneous observed vector capacity in the common lifetime counter.
    pub(super) peak: usize,
    /// Actual aligned registry, counter and resource-association metadata.
    pub(super) metadata: usize,
    /// Distinct original factor reservations plus newly admitted additional backend work.
    pub(super) backend: usize,
}
impl ExecutionInventory {
    /// Validated execution whose resources this immutable history describes.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Distinct factor shares retain the exact original allocation ticket.
    #[must_use]
    pub fn factors(&self) -> &[SharedFactorAllocation] {
        &self.factors
    }
    /// Successful observed allocation requests in the covered private schedules.
    #[must_use]
    pub const fn observed_allocations(&self) -> usize {
        self.allocations
    }
    /// Cumulative actual vector capacities, excluding opaque backend storage.
    #[must_use]
    pub const fn cumulative_payload_bytes(&self) -> usize {
        self.bytes
    }
    /// Common observed vector peak, never a sum of overlapping scoped peaks.
    #[must_use]
    pub const fn peak_payload_bytes(&self) -> usize {
        self.peak
    }
    /// Original factor reservations counted once, plus newly admitted backend work.
    /// This is a named faer storage/workspace allowance, never measured allocator capacity.
    #[must_use]
    pub const fn backend_allowance_bytes(&self) -> usize {
        self.backend
    }
    /// Owned resource registry and receipt metadata, excluding existing scoped plans.
    #[must_use]
    pub const fn metadata_bytes(&self) -> usize {
        self.metadata
    }
}
