// SPDX-License-Identifier: AGPL-3.0-only
//! Store-local checked exact records and complete finite constant judgments.

mod accounting;
pub(crate) use accounting::Accounting;
mod attempt;
pub(crate) use attempt::attempt;
mod pi;
pub(crate) use pi::validate as validate_pi;

use std::sync::{Arc, MutexGuard};

use super::model::ScalarKernelCertificate;
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::exact_constant::ExactConstant;
use crate::graph::{Cas, PendingJudgment, ScalarInventory, size};

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
    /// Attempt-local checked inventories move into the graph only after final receipt copying.
    pending: Vec<PendingJudgment>,
    /// Nested receipt scopes share the outer attempt and never publish an inner prefix.
    depth: usize,
    /// Only unpublished scalar and Machin payload; committed judgments live in `State`.
    bytes: u64,
}

impl Cache {
    /// Expired checked judgments cannot retain reusable authority or storage.
    pub(crate) fn clear(&mut self) {
        self.pending = Vec::new();
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
    let snapshot = {
        let cache = records(cas)?;
        Snapshot {
            records: cache.pending.len(),
            bytes: cache.bytes,
        }
    };
    let reservation = budget.checked_records.snapshot(&cas.memo_owner);
    let result = operation(budget);
    if result.is_err() {
        let mut cache = records(cas)?;
        restore_entries(&mut cache.pending, snapshot.records);
        cache.bytes = snapshot.bytes;
        drop(cache);
        budget.checked_records.restore(&cas.memo_owner, reservation);
    }
    result
}

/// Release speculative vector capacity before restoring its earlier logical storage bound.
fn restore_entries<T>(entries: &mut Vec<T>, count: usize) {
    let mut kept = std::mem::take(entries);
    kept.truncate(count);
    *entries = kept.into_boxed_slice().into_vec();
}

/// Both retained judgment kinds roll back together when an enclosure refuses.
struct Snapshot {
    /// Number of exact scalar inventories already checked before the attempt.
    records: usize,
    /// Existing conservative store-owned retention.
    bytes: u64,
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
    let bytes = records(cas)?
        .bytes
        .saturating_add(cas.retained_judgment_bytes());
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
    let count = size(cas.state.scalar_judgments.len(), AT)?;
    budget.work(
        size(key.len(), AT)?.saturating_mul(u64::from(count.bit_width())),
        AT,
    )?;
    if let Some(values) = cas.scalar_records(key)? {
        return Ok(Some(values));
    }
    let cache = records(cas)?;
    let pending = cache
        .pending
        .iter()
        .filter(|entry| {
            matches!(
                entry,
                PendingJudgment::NewScalar(_) | PendingJudgment::RecheckedScalar(_)
            )
        })
        .count();
    budget.work(size(key.len(), AT)?.saturating_mul(size(pending, AT)?), AT)?;
    for entry in &cache.pending {
        let inventory = match entry {
            PendingJudgment::NewScalar(inventory) => inventory,
            PendingJudgment::RecheckedScalar(id) => cas.scalar_inventory(*id)?,
            PendingJudgment::NewPi(_) | PendingJudgment::RecheckedPi(_) => continue,
        };
        if inventory.key.as_ref() == key {
            return Ok(Some(Arc::clone(&inventory.values)));
        }
    }
    drop(cache);
    Ok(None)
}

/// Publish only fully checked meanings, moving their existing allocation into retention.
pub(super) fn insert(
    cas: &Cas,
    certificate: &ScalarKernelCertificate,
    key: Vec<u8>,
    values: Vec<ExactConstant>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if cas.memo_epoch == u64::MAX {
        return Ok(());
    }
    if let Some(id) = cas.state.scalar_judgments.get(key.as_slice()) {
        cas.scalar_inventory(*id)?;
        let mut cache = records(cas)?;
        cache
            .pending
            .try_reserve_exact(1)
            .map_err(|_| Fault::allocation(AT))?;
        cache.pending.push(PendingJudgment::RecheckedScalar(*id));
        drop(cache);
        return Ok(());
    }
    let scope = [
        cas.checked(&certificate.expression)?,
        cas.checked(&certificate.context)?,
        cas.checked(&certificate.input_family)?,
    ];
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
        size(cas.counts().vertices, AT)?
            .saturating_add(size(cache.pending.len(), AT)?)
            .saturating_add(1),
        AT,
    )?;
    accounting::retain(
        cas,
        total.saturating_add(cas.retained_judgment_bytes()),
        budget,
    )?;
    cache
        .pending
        .try_reserve_exact(1)
        .map_err(|_| Fault::allocation(AT))?;
    cache
        .pending
        .push(PendingJudgment::NewScalar(ScalarInventory {
            key: key.into(),
            values: Arc::new(values),
            scope,
            bytes,
        }));
    cache.bytes = total;
    drop(cache);
    Ok(())
}

#[cfg(test)]
mod tests;
