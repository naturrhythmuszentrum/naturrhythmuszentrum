// SPDX-License-Identifier: AGPL-3.0-only
//! One counter observes overlapping scoped allocation lifetimes in an execution.

use crate::{ExecutionId, OperationError};
use std::{cell::Cell, rc::Rc};

/// Counters retain no numerical owner and cannot extend a resource lifetime.
#[derive(Debug)]
struct Counters {
    /// Original validated execution identity.
    execution: ExecutionId,
    /// Actual currently owned observable vector capacities.
    live: Cell<usize>,
    /// Maximum observed by this common counter, never a sum of scoped peaks.
    peak: Cell<usize>,
    /// Sum of actual capacities observed over successful allocation events.
    bytes: Cell<usize>,
    /// Number of successful observed allocation requests.
    allocations: Cell<usize>,
}
/// Shared event accounting has the same inline width as the execution identity it replaces.
#[repr(transparent)]
#[derive(Clone, Debug)]
pub(crate) struct Trace(Rc<Counters>);
impl Trace {
    /// Establish one common lifetime counter before the execution's first private allocation.
    pub(crate) fn new(execution: ExecutionId) -> Self {
        Self(Rc::new(Counters {
            execution,
            live: Cell::new(0),
            peak: Cell::new(0),
            bytes: Cell::new(0),
            allocations: Cell::new(0),
        }))
    }
    /// Stable ticket origin, independent of shared resource or buffer addresses.
    pub(crate) fn execution(&self) -> ExecutionId {
        self.0.execution
    }
    /// Record a new actual capacity before another covered allocation can begin.
    pub(crate) fn allocated(&self, bytes: usize) -> Result<(), OperationError> {
        let live = self
            .0
            .live
            .get()
            .checked_add(bytes)
            .ok_or(OperationError::ResourceLimit)?;
        let total = self
            .0
            .bytes
            .get()
            .checked_add(bytes)
            .ok_or(OperationError::ResourceLimit)?;
        let count = self
            .0
            .allocations
            .get()
            .checked_add(1)
            .ok_or(OperationError::ResourceLimit)?;
        self.0.live.set(live);
        self.0.bytes.set(total);
        self.0.allocations.set(count);
        self.0.peak.set(self.0.peak.get().max(live));
        Ok(())
    }
    /// An original permit reports release once, after recording a successful allocation.
    pub(crate) fn released(&self, bytes: usize) {
        self.0.live.set(self.0.live.get() - bytes);
    }
    /// Freeze common-lifetime observations for immutable publication history.
    pub(crate) fn observations(&self) -> [usize; 3] {
        [
            self.0.allocations.get(),
            self.0.bytes.get(),
            self.0.peak.get(),
        ]
    }
    /// Actual aligned counter allocation, including its reference-count header.
    pub(crate) const fn metadata_bytes() -> usize {
        size_of::<RcLayout>()
    }
}
/// Reference-counted allocation header and aligned event counters.
#[repr(C)]
struct RcLayout {
    /// Strong reference count.
    strong: usize,
    /// Weak reference count.
    weak: usize,
    /// Counter payload, with no numerical resource references.
    value: Counters,
}
