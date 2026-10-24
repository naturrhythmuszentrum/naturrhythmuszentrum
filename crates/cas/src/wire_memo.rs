// SPDX-License-Identifier: AGPL-3.0-only
//! Request-local immutable typed record sharing; graph edges retain the complete original content.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::graph::{Cas, size};
use crate::wire_build::{Entity, Key, Record};
use crate::wire_read::invalid;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Record sharing is a storage optimization and never grants proof authority.
const AT: Location = Location::new("typed witness record memo");

/// One ledger owns all retained records and their simultaneous payload allowance.
#[derive(Debug, Default)]
pub(crate) struct Cache {
    /// Reading and dependency services opt into reuse; unrelated one-shot exports retain nothing.
    pub(crate) enabled: bool,
    /// A different store or test fork cannot interpret these canonical graph references.
    owner: Option<Arc<()>>,
    /// Any rollback invalidates references before their serials can be reused.
    epoch: u64,
    /// Exact local canonical records; their graph edges include every original dependency.
    entries: BTreeMap<Key, Arc<Record>>,
    /// Conservative retained payload charged beside every temporary allocation.
    pub(crate) bytes: u64,
    /// Crate-test count of reused records.
    #[cfg(test)]
    pub(crate) hits: u64,
    /// Crate-test count of independently described records.
    #[cfg(test)]
    pub(crate) misses: u64,
}

/// Establish the immutable graph frame before consulting any retained record.
pub(crate) fn prepare(cas: &Cas, budget: &mut Budget) {
    let cache = &mut budget.witness_records;
    let same_owner = cache
        .owner
        .as_ref()
        .is_some_and(|owner| Arc::ptr_eq(owner, &cas.memo_owner));
    if !same_owner || cache.epoch != cas.memo_epoch {
        cache.entries.clear();
        cache.bytes = 0;
        cache.owner = Some(Arc::clone(&cas.memo_owner));
        cache.epoch = cas.memo_epoch;
    }
}

/// Retain exactly the record that ordinary canonical discovery describes, including evidence edges.
pub(crate) fn record(
    cas: &Cas,
    entity: &Entity,
    key: &Key,
    budget: &mut Budget,
) -> Result<Arc<Record>, Fault> {
    if !budget.witness_records.enabled {
        let value = Arc::new(crate::wire_records::describe(cas, entity, budget)?);
        budget.work(footprint(&value)?, Location::new("typed witness DAG"))?;
        return Ok(value);
    }
    let comparisons = size(budget.witness_records.entries.len(), AT)?.bit_width();
    budget.work(key_bytes(key)?.saturating_mul(u64::from(comparisons)), AT)?;
    if let Some(value) = budget.witness_records.entries.get(key).map(Arc::clone) {
        budget.work(edge_copies(&value)?, AT)?;
        #[cfg(test)]
        {
            budget.witness_records.hits += 1;
        }
        return Ok(value);
    }
    #[cfg(test)]
    {
        budget.witness_records.misses += 1;
    }
    let value = Arc::new(crate::wire_records::describe(cas, entity, budget)?);
    let bytes = footprint(&value)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    if cas.memo_epoch != u64::MAX {
        budget.witness_records.bytes = budget
            .witness_records
            .bytes
            .checked_add(bytes)
            .ok_or_else(invalid)?;
        budget
            .witness_records
            .entries
            .insert(key.clone(), Arc::clone(&value));
    }
    Ok(value)
}

/// The existing gathered-DAG allowance covers record buffers, typed edges, keys and shared owners.
pub(crate) fn footprint(record: &Record) -> Result<u64, Fault> {
    size(record.payload.len(), AT)?
        .checked_mul(16)
        .and_then(|bytes| {
            bytes.checked_add(size(record.references.len(), AT).ok()?.checked_mul(256)?)
        })
        .ok_or_else(invalid)
}

/// Fixed object references name canonical immutable graph nodes; synthetic keys contain all fields.
pub(crate) fn key_bytes(key: &Key) -> Result<u64, Fault> {
    match key {
        Key::Object(_) => Ok(16),
        Key::Guard(bytes) | Key::Statement(bytes) | Key::Conclusion(bytes) => size(bytes.len(), AT),
    }
}

/// A hit borrows payload bytes, but the traversal still copies each typed dependency into its worklist.
fn edge_copies(record: &Record) -> Result<u64, Fault> {
    let mut count = record
        .references
        .len()
        .checked_add(usize::from(record.space.is_some()))
        .ok_or_else(invalid)?;
    for (_, _, members) in &record.row_sets {
        count = count.checked_add(members.len()).ok_or_else(invalid)?;
    }
    size(count, AT)?.checked_mul(256).ok_or_else(invalid)
}
