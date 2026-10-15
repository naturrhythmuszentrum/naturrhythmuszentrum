// SPDX-License-Identifier: AGPL-3.0-only
//! Checked occupation tables; all allocation precedes publication.

use super::HeapError;
use super::identity::Key;

/// One occupation in a descriptor or precision-specific arena table.
#[derive(Debug)]
struct Slot<T> {
    /// Monotonic occupation number.
    generation: u64,
    /// Occupied payload.
    value: T,
}

/// An owner's bounded occupation table.
#[derive(Debug)]
pub(crate) struct Slots<T> {
    /// Stable slot positions.
    entries: Vec<Slot<T>>,
}

impl<T> Slots<T> {
    /// Empty table without allocations.
    pub(crate) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Reserve every append and return its checked future locator.
    pub(crate) fn prepare(&mut self, count: usize, limit: usize) -> Result<Vec<Key>, HeapError> {
        let end = self
            .entries
            .len()
            .checked_add(count)
            .ok_or(HeapError::Capacity)?;
        if end > limit {
            return Err(HeapError::Capacity);
        }
        let mut keys = Vec::new();
        keys.try_reserve_exact(count)
            .map_err(|_| HeapError::Capacity)?;
        self.entries
            .try_reserve(count)
            .map_err(|_| HeapError::Capacity)?;
        keys.extend((self.entries.len()..end).map(|slot| Key {
            slot,
            generation: 1,
        }));
        Ok(keys)
    }

    /// Install fully prepared records into previously reserved capacity.
    pub(crate) fn publish(&mut self, values: impl ExactSizeIterator<Item = T>) {
        self.entries.extend(values.map(|value| Slot {
            generation: 1,
            value,
        }));
    }

    /// Resolve an exact occupation before exposing its payload.
    pub(crate) fn get(&self, key: Key) -> Result<&T, HeapError> {
        let slot = self.entries.get(key.slot).ok_or(HeapError::StaleHandle)?;
        if slot.generation != key.generation {
            return Err(HeapError::StaleHandle);
        }
        Ok(&slot.value)
    }
}
