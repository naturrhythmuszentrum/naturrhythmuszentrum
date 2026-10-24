// SPDX-License-Identifier: AGPL-3.0-only
//! Successful publication transfers the lease; retirement drops only that execution's share.

use super::lease::Share;
use super::{ExecutionInventory, Lease};
use crate::{ExecutionId, Heap, HeapError, OperationError, RecordId};

/// Immutable history and independently releasable numerical resource ownership.
#[derive(Debug)]
struct Entry {
    /// Exact heap-scoped operation record that completed this execution.
    record: RecordId,
    /// No numerical references survive here after retirement.
    active: Option<Vec<Share>>,
    /// Historical measurements and original tickets remain readable.
    inventory: ExecutionInventory,
}
/// Heap-owned completed executions; publication reserves capacity before its no-fail commit.
#[derive(Debug, Default)]
pub(crate) struct Registry {
    /// Completed records retain immutable observations and independently releasable shares.
    entries: Vec<Entry>,
}
impl Registry {
    /// Exact aligned inline registry entry charged to this execution's metadata budget.
    pub(crate) const fn entry_bytes() -> usize {
        size_of::<Entry>()
    }
    /// Prepare all allocation and exact ticket checks before publishing any output.
    pub(crate) fn prepare<P: nrz_scalar::Scalar>(
        &mut self,
        lease: &Lease,
        candidate: &crate::Candidate<P>,
    ) -> Result<ExecutionInventory, OperationError> {
        lease.validate(candidate)?;
        let inventory = lease.snapshot()?;
        self.entries
            .try_reserve_exact(1)
            .map_err(|_| OperationError::ResourceLimit)?;
        Ok(inventory)
    }
    /// Commit only prepared metadata and move the existing numerical shares without cloning them.
    pub(crate) fn publish(
        &mut self,
        record: RecordId,
        lease: Lease,
        inventory: ExecutionInventory,
    ) {
        self.entries.push(Entry {
            record,
            active: Some(lease.into_shares()),
            inventory,
        });
    }
    /// No fallible work follows removal of this one execution's ownership share.
    fn retire(&mut self, record: RecordId, execution: ExecutionId) -> Result<(), HeapError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.record == record)
            .ok_or(HeapError::StaleEvidence)?;
        if entry.inventory.execution() != execution {
            return Err(HeapError::StaleEvidence);
        }
        let active = entry.active.take().ok_or(HeapError::StaleHandle)?;
        drop(active);
        Ok(())
    }
}
impl Heap {
    /// Retire the completed execution's private resource shares in one call.
    ///
    /// Independent external factors and other completed consumers retain their own shares.
    /// Published value regions and immutable operation history have separate lifetimes.
    /// # Errors
    /// Refuses a foreign record, an unknown execution or repeated retirement without mutation.
    pub fn retire_execution(&mut self, record: RecordId) -> Result<(), HeapError> {
        let execution = self.historical_record(record)?.execution();
        self.executions.retire(record, execution)
    }
    /// Inspect immutable private allocation observations after publication or retirement.
    /// # Errors
    /// Refuses a foreign or unknown completed operation record.
    pub fn execution_inventory(&self, record: RecordId) -> Result<&ExecutionInventory, HeapError> {
        self.historical_record(record)?;
        self.executions
            .entries
            .iter()
            .find(|entry| entry.record == record)
            .map(|entry| &entry.inventory)
            .ok_or(HeapError::StaleEvidence)
    }
}
