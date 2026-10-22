// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered storage positions retain opaque region-version tickets supplied by the numerical owner.

use super::{AT, ReadingAdapterError, capacity};
use nrz_bindings::Budget;

/// One exact storage position; the opaque ticket must identify its full numerical region version.
///
/// Equality of tickets is only an inventory assertion until the owning heap checks
/// its scope, generations, value and target versions, layout and precision under a lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryTicket<T> {
    /// Opaque owner-supplied region-version identity; it grants no access by itself.
    pub region: T,
    /// Exact base-region logical entry index, independent of the selected view's order.
    pub index: u64,
}

/// A complete selected entry list must be injective before any overlap is compared.
pub(super) fn check<T: Eq>(
    entries: &[EntryTicket<T>],
    count: usize,
    budget: &mut Budget,
) -> Result<(), ReadingAdapterError> {
    capacity::<EntryTicket<T>>(entries.len(), budget)?;
    if entries.len() != count {
        return Err(ReadingAdapterError::Extent);
    }
    for (position, entry) in entries.iter().enumerate() {
        budget.work(
            u64::try_from(position).map_err(|_| ReadingAdapterError::Extent)?,
            AT,
        )?;
        if entries[..position].contains(entry) {
            return Err(ReadingAdapterError::Duplicate);
        }
    }
    Ok(())
}
