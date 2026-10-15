// SPDX-License-Identifier: AGPL-3.0-only
//! Checked occupation changes, including permanent retirement at generation exhaustion.

use super::HeapError;
use super::identity::Key;

/// One occupied, vacant or permanently exhausted descriptor/arena position.
#[derive(Debug)]
struct Slot<T> {
    /// Last occupation number; a vacant maximum-generation slot is never reused.
    generation: u64,
    /// Live payload, absent after retirement.
    value: Option<T>,
}

/// An owner's bounded occupation table.
#[derive(Debug)]
pub(crate) struct Slots<T> {
    /// Stable positions, including permanently retired positions.
    entries: Vec<Slot<T>>,
}

/// Checked future occupations and removals, under an uninterrupted exclusive owner borrow.
#[derive(Debug)]
pub(crate) struct SlotPlan {
    /// Future keys in the same order as the prepared payloads.
    keys: Vec<Key>,
    /// Validated current occupations to retire atomically.
    retired: Vec<Key>,
}

/// A complete occupation change with matching payload count and reserved capacity.
#[derive(Debug)]
pub(crate) struct PreparedSlots<T> {
    /// Checked locations and generations.
    plan: SlotPlan,
    /// Fully prepared records in key order.
    values: Vec<T>,
}

impl<T> PreparedSlots<T> {
    /// Future locators retained through the final preparation stage.
    pub(crate) fn keys(&self) -> &[Key] {
        &self.plan.keys
    }
}

impl SlotPlan {
    /// Future locators; callers cannot publish them without completing preparation.
    pub(crate) fn keys(&self) -> &[Key] {
        &self.keys
    }

    /// Finish the payload-count obligation before publication starts.
    pub(crate) fn bind<T>(self, values: Vec<T>) -> Result<PreparedSlots<T>, HeapError> {
        if values.len() != self.keys.len() {
            return Err(HeapError::LengthMismatch);
        }
        Ok(PreparedSlots { plan: self, values })
    }
}

impl<T> Slots<T> {
    /// Empty table without allocations.
    pub(crate) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Reserve a fresh publication, reusing eligible vacant occupations first.
    pub(crate) fn prepare(&mut self, count: usize, limit: usize) -> Result<SlotPlan, HeapError> {
        self.change(count, None, &[], limit)
    }

    /// Validate a complete change, preserving at most one surviving occupation.
    pub(crate) fn change(
        &mut self,
        count: usize,
        keep: Option<Key>,
        retired: &[Key],
        limit: usize,
    ) -> Result<SlotPlan, HeapError> {
        self.check_change(count, keep, retired)?;
        if count > limit || self.entries.len() > limit {
            return Err(HeapError::Capacity);
        }
        let mut keys = Vec::new();
        keys.try_reserve_exact(count)
            .map_err(|_| HeapError::Capacity)?;
        keys.extend(keep);
        self.reusable_keys(&mut keys, count, retired);
        let append = count.checked_sub(keys.len()).ok_or(HeapError::Capacity)?;
        let end = self
            .entries
            .len()
            .checked_add(append)
            .ok_or(HeapError::Capacity)?;
        if end > limit {
            return Err(HeapError::Capacity);
        }
        keys.extend((self.entries.len()..end).map(|slot| Key {
            slot,
            generation: 1,
        }));
        let mut removals = Vec::new();
        removals
            .try_reserve_exact(retired.len())
            .map_err(|_| HeapError::Capacity)?;
        removals.extend_from_slice(retired);
        self.entries
            .try_reserve(append)
            .map_err(|_| HeapError::Capacity)?;
        Ok(SlotPlan {
            keys,
            retired: removals,
        })
    }

    /// Reject stale, duplicated or conflicting changes before reserving any payload.
    fn check_change(
        &self,
        count: usize,
        keep: Option<Key>,
        retired: &[Key],
    ) -> Result<(), HeapError> {
        if let Some(key) = keep {
            self.get(key)?;
            if count == 0 || retired.contains(&key) {
                return Err(HeapError::InvalidLayout);
            }
        }
        for (index, key) in retired.iter().enumerate() {
            self.get(*key)?;
            if retired
                .get(..index)
                .is_some_and(|earlier| earlier.contains(key))
            {
                return Err(HeapError::InvalidLayout);
            }
        }
        Ok(())
    }

    /// Enumerate reusable slots in stable order, skipping exhausted generations.
    fn reusable_keys(&self, keys: &mut Vec<Key>, count: usize, retired: &[Key]) {
        for (index, slot) in self.entries.iter().enumerate() {
            if keys.len() == count {
                break;
            }
            let current = Key {
                slot: index,
                generation: slot.generation,
            };
            if (slot.value.is_none() || retired.contains(&current))
                && let Some(generation) = slot.generation.checked_add(1)
            {
                keys.push(Key {
                    slot: index,
                    generation,
                });
            }
        }
    }

    /// Link a prepared change with no allocation, fallible work or caller callback.
    pub(crate) fn publish(&mut self, prepared: PreparedSlots<T>) {
        for key in prepared.plan.retired {
            if let Some(slot) = self.entries.get_mut(key.slot) {
                slot.value = None;
            }
        }
        for (key, value) in prepared.plan.keys.into_iter().zip(prepared.values) {
            let occupied = Slot {
                generation: key.generation,
                value: Some(value),
            };
            if let Some(slot) = self.entries.get_mut(key.slot) {
                *slot = occupied;
            } else {
                // Preparation validated consecutive append positions and reserved their capacity.
                self.entries.push(occupied);
            }
        }
    }

    /// Resolve an exact live occupation before exposing its payload.
    pub(crate) fn get(&self, key: Key) -> Result<&T, HeapError> {
        let slot = self.entries.get(key.slot).ok_or(HeapError::StaleHandle)?;
        if slot.generation != key.generation {
            return Err(HeapError::StaleHandle);
        }
        slot.value.as_ref().ok_or(HeapError::StaleHandle)
    }

    /// Test-only corruption access for inaccessible version/dependency guards.
    #[cfg(test)]
    pub(crate) fn get_mut(&mut self, key: Key) -> Result<&mut T, HeapError> {
        let slot = self
            .entries
            .get_mut(key.slot)
            .ok_or(HeapError::StaleHandle)?;
        if slot.generation != key.generation {
            return Err(HeapError::StaleHandle);
        }
        slot.value.as_mut().ok_or(HeapError::StaleHandle)
    }
}

/// Inaccessible occupation exhaustion and no-fail link invariants.
#[cfg(test)]
mod tests {
    use super::{HeapError, Key, Slot, Slots};

    /// Retired occupations advance once; a maximum-generation vacancy never revives.
    /// ´test:unit:slot-generations-retire-without-wrap´
    #[test]
    fn slot_generations_retire_without_wrap() -> Result<(), HeapError> {
        let mut slots = Slots {
            entries: vec![Slot {
                generation: u64::MAX - 1,
                value: Some(7),
            }],
        };
        let old = Key {
            slot: 0,
            generation: u64::MAX - 1,
        };
        let plan = slots.change(1, None, std::slice::from_ref(&old), 2)?;
        let final_key = *plan.keys().first().ok_or(HeapError::Capacity)?;
        slots.publish(plan.bind(vec![11])?);
        assert_eq!(
            final_key,
            Key {
                slot: 0,
                generation: u64::MAX
            }
        );
        assert_eq!(slots.get(old), Err(HeapError::StaleHandle));
        assert_eq!(slots.get(final_key), Ok(&11));
        assert!(matches!(
            slots.change(1, None, std::slice::from_ref(&final_key), 1),
            Err(HeapError::Capacity)
        ));
        assert_eq!(slots.get(final_key), Ok(&11));
        let plan = slots.change(1, None, std::slice::from_ref(&final_key), 2)?;
        let fresh = *plan.keys().first().ok_or(HeapError::Capacity)?;
        slots.publish(plan.bind(vec![13])?);
        assert_eq!(
            fresh,
            Key {
                slot: 1,
                generation: 1
            }
        );
        assert_eq!(slots.get(final_key), Err(HeapError::StaleHandle));
        assert_eq!(slots.get(fresh), Ok(&13));
        assert!(
            slots
                .entries
                .first()
                .is_some_and(|slot| slot.value.is_none() && slot.generation == u64::MAX)
        );
        Ok(())
    }
}
