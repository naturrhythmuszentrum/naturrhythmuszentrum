// SPDX-License-Identifier: AGPL-3.0-only
//! Store-local checked exact records under complete scoped certificate bytes.

mod accounting;
pub(crate) use accounting::Accounting;

use std::sync::{Arc, MutexGuard};

use super::model::ScalarKernelCertificate;
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::exact_constant::ExactConstant;
use crate::graph::{Cas, size};

/// Complete key construction, comparison and retained ownership share one location.
const AT: Location = Location::new("checked scalar records");

/// Checked record meanings never escape their original store and rollback generation.
#[derive(Debug, Default)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling graph and enclosure modules share store-local retention"
)]
pub(crate) struct Cache {
    /// Strong ownership prevents namespace recycling by the allocator.
    owner: Option<Arc<()>>,
    /// Any rollback invalidates the complete inventory before logical IDs can recur.
    epoch: u64,
    /// Append-only successful inventories permit atomic refusal without a full copy.
    entries: Vec<Entry>,
    /// Conservative complete keys, exact graphs and entry allocation.
    bytes: u64,
}

/// The complete ordered inventory also identifies every backwards record dependency.
#[derive(Debug)]
struct Entry {
    /// Existing canonical wire grammar with authenticated local scope references.
    key: Vec<u8>,
    /// Independently reconstructed meanings in exactly the original record order.
    values: Arc<Vec<ExactConstant>>,
}

impl Cache {
    /// Expired checked judgments cannot retain reusable authority or storage.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

/// Poisoned runtime state cannot authorize a cached mathematical judgment.
fn records(cas: &Cas) -> Result<MutexGuard<'_, Cache>, Fault> {
    cas.receipt_records
        .lock()
        .map_err(|_| Fault::invalid(InvalidCode::Reference, AT))
}

/// Serialize each complete store-local validation, including its final receipt copy.
pub(crate) fn synchronized<T>(
    cas: &Cas,
    budget: &mut Budget,
    operation: impl FnOnce(&mut Budget) -> Result<T, Fault>,
) -> Result<T, Fault> {
    let _guard = cas
        .receipt_guard
        .lock()
        .map_err(|_| Fault::invalid(InvalidCode::Reference, AT))?;
    operation(budget)
}

/// Establish source identity before consulting any checked record judgment.
fn prepare(cas: &Cas) -> Result<(), Fault> {
    let mut cache = records(cas)?;
    let same_owner = cache
        .owner
        .as_ref()
        .is_some_and(|owner| Arc::ptr_eq(owner, &cas.memo_owner));
    if !same_owner || cache.epoch != cas.memo_epoch || cas.memo_epoch == u64::MAX {
        cache.clear();
        cache.owner = Some(Arc::clone(&cas.memo_owner));
        cache.epoch = cas.memo_epoch;
    }
    drop(cache);
    Ok(())
}

/// A failed returned-receipt copy removes only judgments added by that attempt.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling graph and enclosure modules share store-local retention"
)]
pub(crate) fn atomic<T>(
    cas: &Cas,
    budget: &mut Budget,
    operation: impl FnOnce(&mut Budget) -> Result<T, Fault>,
) -> Result<T, Fault> {
    prepare(cas)?;
    let (count, bytes) = {
        let cache = records(cas)?;
        (cache.entries.len(), cache.bytes)
    };
    let reservation = budget.checked_records.snapshot(&cas.memo_owner);
    let result = operation(budget);
    if result.is_err() {
        let mut cache = records(cas)?;
        cache.entries.truncate(count);
        cache.bytes = bytes;
        drop(cache);
        budget.checked_records.restore(&cas.memo_owner, reservation);
    }
    result
}

/// Previously retained store payload remains reserved beside this validation's scratch.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling graph and enclosure modules share store-local retention"
)]
pub(crate) fn retained<T>(
    cas: &Cas,
    budget: &mut Budget,
    operation: impl FnOnce(&mut Budget) -> Result<T, Fault>,
) -> Result<T, Fault> {
    prepare(cas)?;
    let bytes = records(cas)?.bytes;
    accounting::retain(cas, bytes, budget)?;
    operation(budget)
}

/// Reuse the existing canonical record grammar, including all scope and source fields.
pub(super) fn key(
    cas: &Cas,
    certificate: &ScalarKernelCertificate,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::new(budget, AT);
    super::wire::encode(certificate, &mut out, &mut |handle, out| {
        crate::graph::encode_id(cas.checked(handle)?, out)
    })?;
    Ok(out.finish())
}

/// Exact byte comparison is prepaid for every possible retained inventory inspected.
pub(super) fn lookup(
    cas: &Cas,
    key: &[u8],
    budget: &mut Budget,
) -> Result<Option<Arc<Vec<ExactConstant>>>, Fault> {
    let cache = records(cas)?;
    budget.work(
        size(key.len(), AT)?.saturating_mul(size(cache.entries.len(), AT)?),
        AT,
    )?;
    Ok(cache
        .entries
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| Arc::clone(&entry.values)))
}

/// Publish only fully checked meanings, moving their existing allocation into retention.
pub(super) fn insert(
    cas: &Cas,
    key: Vec<u8>,
    values: Vec<ExactConstant>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if cas.memo_epoch == u64::MAX {
        return Ok(());
    }
    let mut bytes = size(key.capacity(), AT)?.saturating_add(256);
    for value in &values {
        bytes = bytes.saturating_add(value.retained_payload_bytes()?);
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    let mut cache = records(cas)?;
    let total = cache.bytes.saturating_add(bytes);
    budget.check(
        BudgetKind::PayloadBytes,
        cas.counts().payload_bytes.saturating_add(total),
        AT,
    )?;
    budget.check(
        BudgetKind::Vertices,
        size(cache.entries.len(), AT)?.saturating_add(1),
        AT,
    )?;
    cache
        .entries
        .try_reserve(1)
        .map_err(|_| Fault::allocation(AT))?;
    accounting::retain(cas, total, budget)?;
    cache.entries.push(Entry {
        key,
        values: Arc::new(values),
    });
    cache.bytes = total;
    drop(cache);
    Ok(())
}

#[cfg(test)]
mod tests;
