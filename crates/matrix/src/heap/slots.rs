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
    pub(crate) values: Vec<T>,
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
    #[cfg(test)]
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
        self.change_mode(count, keep, retired, limit, false, None)
    }

    /// Prepare native mask occupations inside a prechecked exact-capacity reservation.
    pub(crate) fn change_reserved(
        &mut self,
        count: usize,
        retired: &[Key],
        limit: usize,
    ) -> Result<SlotPlan, HeapError> {
        self.change_mode(count, None, retired, limit, true, None)
    }

    /// Managed admission records each successful allocation in its prepaid prefix guard.
    pub(crate) fn change_tracked(
        &mut self,
        count: usize,
        limit: usize,
        guard: &mut crate::loan::preparation::Preparation,
    ) -> Result<SlotPlan, HeapError> {
        self.change_mode(count, None, &[], limit, true, Some(guard))
    }
    /// Preserve ordinary growth while reserved owners request only the admitted capacity.
    fn change_mode(
        &mut self,
        count: usize,
        keep: Option<Key>,
        retired: &[Key],
        limit: usize,
        exact: bool,
        mut guard: Option<&mut crate::loan::preparation::Preparation>,
    ) -> Result<SlotPlan, HeapError> {
        self.check_change(count, keep, retired)?;
        if count > limit || self.entries.len() > limit {
            return Err(HeapError::Capacity);
        }
        #[cfg(test)]
        if count != 0 {
            crate::loan::fault::before().map_err(|_| HeapError::Capacity)?;
        }
        let mut keys = Vec::new();
        keys.try_reserve_exact(count)
            .map_err(|_| HeapError::Capacity)?;
        if count != 0
            && let Some(guard) = guard.as_mut()
        {
            guard.acquired(true).map_err(|_| HeapError::Capacity)?;
        }
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
        {
            let capacity = if exact {
                self.entries.capacity().max(end)
            } else {
                growth_capacity::<Slot<T>>(self.entries.capacity(), end)?
            };
            if capacity > self.entries.capacity() {
                #[cfg(test)]
                crate::loan::fault::before().map_err(|_| HeapError::Capacity)?;
                let mut replacement = Vec::new();
                replacement
                    .try_reserve_exact(capacity)
                    .map_err(|_| HeapError::Capacity)?;
                if let Some(guard) = guard.as_mut() {
                    guard.acquired(true).map_err(|_| HeapError::Capacity)?;
                }
                if replacement.capacity() != capacity {
                    return Err(HeapError::Capacity);
                }
                replacement.append(&mut self.entries);
                self.entries = replacement;
                if let Some(guard) = guard.as_mut() {
                    guard.retain_table();
                }
            }
            if self.entries.capacity() > capacity
                || keys.capacity() > count
                || removals.capacity() > retired.len()
            {
                return Err(HeapError::Capacity);
            }
        }
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
            let current = Key {
                slot: index,
                generation: slot.generation,
            };
            if keys.len() < count
                && (slot.value.is_none() || retired.contains(&current))
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
    pub(crate) fn publish<U>(&mut self, prepared: PreparedSlots<U>)
    where
        T: From<U>,
    {
        for key in prepared.plan.retired {
            if let Some(slot) = self.entries.get_mut(key.slot) {
                slot.value = None;
            }
        }
        for (key, value) in prepared.plan.keys.into_iter().zip(prepared.values) {
            let occupied = Slot {
                generation: key.generation,
                value: Some(T::from(value)),
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

    /// Visit current occupations in stable slot order without exposing vacant entries.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (Key, &T)> {
        self.entries.iter().enumerate().filter_map(|(index, slot)| {
            slot.value.as_ref().map(|value| {
                (
                    Key {
                        slot: index,
                        generation: slot.generation,
                    },
                    value,
                )
            })
        })
    }

    /// Mutate already validated occupations during an allocation-free relocation commit.
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (Key, &mut T)> {
        self.entries
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| {
                let key = Key {
                    slot: index,
                    generation: slot.generation,
                };
                slot.value.as_mut().map(|value| (key, value))
            })
    }

    /// Allocated table bytes, including vacant and permanently retired occupations.
    pub(crate) fn owned_bytes(&self) -> Result<usize, HeapError> {
        self.entries
            .capacity()
            .checked_mul(size_of::<Slot<T>>())
            .ok_or(HeapError::Capacity)
    }

    /// Reuse vacant nonexhausted slots before extending the stable table.
    pub(crate) fn planned_len(&self, count: usize) -> Result<usize, HeapError> {
        self.planned_len_retiring(count, &[])
    }

    /// Authenticated same-call removals are reusable unless their generation is exhausted.
    fn planned_len_retiring(&self, count: usize, retired: &[Key]) -> Result<usize, HeapError> {
        self.check_change(count, None, retired)?;
        let vacant = self
            .entries
            .iter()
            .filter(|slot| slot.value.is_none() && slot.generation != u64::MAX)
            .count();
        let reusable = vacant
            .checked_add(
                retired
                    .iter()
                    .filter(|key| key.generation != u64::MAX)
                    .count(),
            )
            .ok_or(HeapError::Capacity)?;
        self.entries
            .len()
            .checked_add(count.saturating_sub(reusable))
            .ok_or(HeapError::Capacity)
    }

    /// Predict one insertion into a dense replacement using the ordinary geometric policy.
    pub(crate) fn dense_backing_bytes(old: usize, count: usize) -> Result<usize, HeapError> {
        let width = size_of::<Slot<T>>();
        growth_capacity::<Slot<T>>(old / width, count)?
            .checked_mul(width)
            .ok_or(HeapError::Capacity)
    }

    /// Retained growth and the old buffer that coexists during exact replacement.
    pub(crate) fn growth_bytes_reserved(&self, count: usize) -> Result<(usize, usize), HeapError> {
        self.growth_bytes_reserved_limit(count, usize::MAX)
    }

    /// The same vacancy scan also checks the immutable occupation ceiling before allocation.
    pub(crate) fn growth_bytes_reserved_limit(
        &self,
        count: usize,
        limit: usize,
    ) -> Result<(usize, usize), HeapError> {
        let required = self.planned_len(count)?;
        if required > limit {
            return Err(HeapError::Capacity);
        }
        let old = self.owned_bytes()?;
        let next = required
            .max(self.entries.capacity())
            .checked_mul(size_of::<Slot<T>>())
            .ok_or(HeapError::Capacity)?;
        Ok((
            next.checked_sub(old).ok_or(HeapError::Capacity)?,
            if next > old { old } else { 0 },
        ))
    }

    /// Exact native replacement uses the same vacant and retired occupations as publication.
    pub(crate) fn growth_bytes_replacing(
        &self,
        count: usize,
        retired: &[Key],
    ) -> Result<(usize, usize), HeapError> {
        let old = self.owned_bytes()?;
        let next = self
            .planned_len_retiring(count, retired)?
            .max(self.entries.capacity())
            .checked_mul(size_of::<Slot<T>>())
            .ok_or(HeapError::Capacity)?;
        Ok((
            next.checked_sub(old).ok_or(HeapError::Capacity)?,
            if next > old { old } else { 0 },
        ))
    }

    /// Return retained growth and the excess of the largest actual backing peak over it.
    /// Each assignment drops its old backing before the next reservation can replace it.
    pub(crate) fn growth_bytes_sequence(
        &self,
        first: usize,
        last: usize,
    ) -> Result<(usize, usize), HeapError> {
        let original = self.entries.capacity();
        let middle = growth_capacity::<Slot<T>>(original, self.planned_len(first)?)?;
        let final_capacity = growth_capacity::<Slot<T>>(middle, self.planned_len(last)?)?;
        let retained = final_capacity
            .checked_sub(original)
            .ok_or(HeapError::Capacity)?;
        let peak = if final_capacity > middle {
            middle
                .checked_add(final_capacity)
                .ok_or(HeapError::Capacity)?
        } else if middle > original {
            original.checked_add(middle).ok_or(HeapError::Capacity)?
        } else {
            0
        };
        Ok((
            retained
                .checked_mul(size_of::<Slot<T>>())
                .ok_or(HeapError::Capacity)?,
            peak.checked_sub(retained)
                .and_then(|rows| rows.checked_mul(size_of::<Slot<T>>()))
                .ok_or(HeapError::Capacity)?,
        ))
    }

    /// Include same-call retirement reuse before predicting an actual backing replacement.
    pub(crate) fn growth_bytes_for(
        &self,
        count: usize,
        keep: Option<Key>,
        retired: impl Fn(Key) -> bool,
    ) -> Result<(usize, usize), HeapError> {
        let reusable = self
            .entries
            .iter()
            .enumerate()
            .filter(|(slot, entry)| {
                let key = Key {
                    slot: *slot,
                    generation: entry.generation,
                };
                (entry.value.is_none() || retired(key)) && entry.generation != u64::MAX
            })
            .count();
        let reused = reusable
            .checked_add(usize::from(keep.is_some()))
            .ok_or(HeapError::Capacity)?;
        let end = self
            .entries
            .len()
            .checked_add(count.saturating_sub(reused))
            .ok_or(HeapError::Capacity)?;
        let old = self.owned_bytes()?;
        let next = growth_capacity::<Slot<T>>(self.entries.capacity(), end)?
            .checked_mul(size_of::<Slot<T>>())
            .ok_or(HeapError::Capacity)?;
        Ok((
            next.checked_sub(old).ok_or(HeapError::Capacity)?,
            if next > old { old } else { 0 },
        ))
    }

    /// Slots inspected by an inventory scan, including vacant positions.
    pub(crate) const fn scan_work(&self) -> usize {
        self.entries.len()
    }

    /// Remove an already authenticated occupation without allocating a retirement plan.
    pub(crate) fn remove(&mut self, key: Key) -> Result<T, HeapError> {
        let slot = self
            .entries
            .get_mut(key.slot)
            .ok_or(HeapError::StaleHandle)?;
        if slot.generation != key.generation {
            return Err(HeapError::StaleHandle);
        }
        slot.value.take().ok_or(HeapError::StaleHandle)
    }

    /// Inspect one slot, including vacancies, for bounded serialized scans.
    pub(crate) fn at(&self, index: usize) -> Option<(Key, Option<&T>)> {
        self.entries.get(index).map(|slot| {
            (
                Key {
                    slot: index,
                    generation: slot.generation,
                },
                slot.value.as_ref(),
            )
        })
    }

    /// Test-only corruption access for inaccessible version/dependency guards.
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

/// Preplan amortized capacity while preserving exact physical reservation accounting.
pub(crate) fn growth_capacity<T>(old: usize, required: usize) -> Result<usize, HeapError> {
    if required <= old {
        return Ok(old);
    }
    let minimum = if size_of::<T>() == 1 {
        8
    } else if size_of::<T>() <= 1024 {
        4
    } else {
        1
    };
    Ok(required
        .max(old.checked_mul(2).ok_or(HeapError::Capacity)?)
        .max(minimum))
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

    /// A full table pays one replacement peak; its next neighbour reuses the grown capacity.
    /// ´test:unit:loan-legacy-table-replacement-peak-boundaries´
    #[test]
    fn loan_legacy_table_replacement_peak_boundaries() -> Result<(), HeapError> {
        let mut slots = Slots::<u64>::new();
        let plan = slots.prepare(4, 32)?;
        let replaced = *plan.keys().first().ok_or(HeapError::Capacity)?;
        slots.publish(plan.bind(vec![1_u64, 2, 3, 4])?);
        assert_eq!(
            slots.growth_bytes_for(1, None, |key| key == replaced)?,
            (0, 0)
        );
        let replacement = slots.change(1, None, std::slice::from_ref(&replaced), 32)?;
        slots.publish(replacement.bind(vec![9_u64])?);
        let row = size_of::<u64>() + size_of::<Option<u64>>();
        assert_eq!(slots.owned_bytes()?, 4 * row);
        let (delta, old) = slots.growth_bytes_for(1, None, |_| false)?;
        assert_eq!((delta, delta + 2 * old), (4 * row, 12 * row));
        let heap = crate::Heap::new(crate::HeapLimits::default())?;
        for (peak, success) in [(12 * row, true), (12 * row - 1, false)] {
            let result = heap.preflight_resources(
                crate::heap::resources::ResourcePlan {
                    retained: delta,
                    scratch: 2 * old,
                    ..Default::default()
                },
                crate::heap::resources::ResourcePolicy {
                    output: u64::MAX,
                    staged: u64::try_from(peak).map_err(|_| HeapError::Capacity)?,
                },
            );
            assert_eq!(result.is_ok(), success);
        }
        let plan = slots.prepare(1, 32)?;
        slots.publish(plan.bind(vec![5_u64])?);
        assert_eq!(slots.owned_bytes()?, 8 * row);
        assert_eq!(slots.growth_bytes_for(1, None, |_| false)?, (0, 0));
        let plan = slots.prepare(1, 32)?;
        slots.publish(plan.bind(vec![6_u64])?);
        assert_eq!(slots.owned_bytes()?, 8 * row);
        Ok(())
    }
}
