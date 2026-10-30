// SPDX-License-Identifier: AGPL-3.0-only
//! One staged attempt spans checking and every enclosing receipt's final fallible operation.

use super::{Snapshot, prepare, records, restore_entries};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Cas;

/// Defer all new scalar graph nodes until the outer operation and its final copy succeed.
pub(crate) fn attempt<T, E>(
    cas: &mut Cas,
    budget: &mut Budget,
    operation: impl FnOnce(&mut Cas, &mut Budget) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<Fault>,
{
    prepare(cas)?;
    let epoch = cas.memo_epoch;
    let snapshot = {
        let mut cache = records(cas)?;
        cache.depth += 1;
        Snapshot {
            records: cache.pending.len(),
            pi: cache.pi.len(),
            bytes: cache.bytes,
        }
    };
    let reservation = budget.checked_records.snapshot(&cas.memo_owner);
    let mut result = operation(cas, budget);
    let outer = {
        let mut cache = records(cas)?;
        cache.depth -= 1;
        cache.depth == 0
    };
    if outer
        && result.is_ok()
        && let Err(fault) = publish(cas, budget)
    {
        result = Err(E::from(fault));
    }
    if result.is_err() && cas.memo_epoch == epoch {
        let mut cache = records(cas)?;
        restore_entries(&mut cache.pending, snapshot.records);
        restore_entries(&mut cache.pi, snapshot.pi);
        cache.bytes = snapshot.bytes;
        drop(cache);
        budget.checked_records.restore(&cas.memo_owner, reservation);
    }
    if result.is_err() && cas.memo_epoch != epoch {
        records(cas)?.clear();
        budget.checked_records.forget(&cas.memo_owner);
    }
    result
}

/// Only preflight is fallible; committing the moved payload cannot expose a failed prefix.
fn publish(cas: &mut Cas, budget: &Budget) -> Result<(), Fault> {
    let cache = cas
        .receipt_records
        .get_mut()
        .map_err(|_| Fault::invalid(crate::error::InvalidCode::Reference, super::AT))?;
    let mut pending = std::mem::take(&mut cache.pending);
    let result = cas.publish_scalar_judgments(&mut pending, budget);
    // Exclusive access and the successful poison check above preclude a new poison here.
    let cache = cas
        .receipt_records
        .get_mut()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match result {
        Ok(bytes) => cache.bytes = cache.bytes.saturating_sub(bytes),
        Err(fault) => {
            cache.pending = pending;
            return Err(fault);
        }
    }
    Ok(())
}
