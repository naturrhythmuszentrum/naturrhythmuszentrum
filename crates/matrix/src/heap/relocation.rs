// SPDX-License-Identifier: AGPL-3.0-only
//! Between-borrow relocation, prepared in a separate owner before atomic indirection changes.
//!
//! # Compaction contract
//!
//! `Heap::compact` requires an exclusive heap borrow, so live views and future exclusively borrowed evaluations prevent compaction. It prepares fresh precision-separated slabs and dense arena slot tables, validates every affected value, companion and alias map, then installs all indirection and the successor physical epoch together. It retains one slab per owned region and stable descriptor slot indices. Value versions, schemas, bound handles and target identities remain unchanged when only storage moves. `RegionView::layout_epoch` exposes the refreshed physical revision; cached internal regions with an older epoch refuse even if an arena slot/generation coincides. Epoch exhaustion and all preparation failures leave the current heap unchanged. Compaction work counts copied entries across both precisions plus explicit gather entries. Old and new slabs may coexist during preparation, with one additional raw region copy; compaction introduces no numerical or source-independence claim.

use nrz_scalar::Scalar;

use super::identity::Key;
use super::records::{Location, Meaning, Record};
use super::{Companion, Handle, Heap, HeapError, Value};
use crate::HeapLimits;
use crate::storage::{Format, Region, Storage};

/// Complete private replacement slabs and their validated descriptor indirections.
#[derive(Debug)]
struct Relocation {
    /// Fresh, densely indexed typed slab tables under the successor epoch.
    storage: Storage,
    /// New owned locations, in descriptor slot order; aliases continue through their base.
    locations: Vec<(Key, Region)>,
}

impl Heap {
    /// Relocate live allocations into fresh slabs and compact the typed arena slot tables.
    ///
    /// The exclusive borrow permits this only between evaluations and live region borrows.
    /// Every affected map and companion relation is validated before indirection changes.
    /// Handles, value versions, schemas and bound targets survive; physical epochs advance.
    /// Individual owned regions remain separate slabs, and descriptor slot indices stay stable.
    ///
    /// # Errors
    /// Refuses stale dependencies, invalid maps, exhausted epochs, capacity or layout work.
    /// Work counts copied entries in both precisions plus every gathered-map entry.
    /// Every refusal leaves all published locations, epochs and values unchanged.
    pub fn compact(&mut self) -> Result<(), HeapError> {
        self.preflight_relocation()?;
        let prepared = self.prepare_relocation()?;
        self.validate_relocation(&prepared)?;
        self.publish_relocation(prepared);
        Ok(())
    }

    /// Validate current evidence and the complete copy/map work budget before backend entry.
    fn preflight_relocation(&self) -> Result<(), HeapError> {
        let mut remaining = self.limits.layout_work;
        for (key, record) in self.descriptors.iter() {
            let region = self.owning_region(record)?;
            let copies = if matches!(record.location, Location::Owned(_)) {
                region.length
            } else {
                0
            };
            remaining = remaining.checked_sub(copies).ok_or(HeapError::WorkLimit)?;
            remaining = remaining
                .checked_sub(record.layout.validation_work())
                .ok_or(HeapError::WorkLimit)?;
            match region.format {
                Format::Binary32 => self.validate_live_record::<f32>(key, record)?,
                Format::Binary64 => self.validate_live_record::<f64>(key, record)?,
            }
        }
        Ok(())
    }

    /// Use the same public-access checks for base values, aliases and companions.
    fn validate_live_record<P: Scalar + 'static>(
        &self,
        key: Key,
        record: &Record,
    ) -> Result<(), HeapError> {
        match record.meaning {
            Meaning::Statistical => {
                self.region_view::<P>(record)?;
            }
            Meaning::Value { .. } | Meaning::Alias { .. } => {
                self.value(Handle::<Value, P>::new(self.scope, key))?;
            }
            Meaning::Bound { .. } => {
                self.companion(Handle::<Companion, P>::new(self.scope, key))?;
            }
        }
        Ok(())
    }

    /// Finish all typed copies in an independent owner; visible heap storage stays untouched.
    fn prepare_relocation(&self) -> Result<Relocation, HeapError> {
        let mut storage = self.storage.replacement()?;
        let mut locations = Vec::new();
        locations
            .try_reserve_exact(self.descriptors.iter().count())
            .map_err(|_| HeapError::Capacity)?;
        for (key, record) in self.descriptors.iter() {
            if let Location::Owned(region) = record.location {
                let moved = match region.format {
                    Format::Binary32 => {
                        copy_owner::<f32>(&self.storage, region, &mut storage, self.limits)?
                    }
                    Format::Binary64 => {
                        copy_owner::<f64>(&self.storage, region, &mut storage, self.limits)?
                    }
                };
                locations.push((key, moved));
            }
        }
        Ok(Relocation { storage, locations })
    }

    /// Revalidate every base, companion and composed alias against the replacement allocations.
    fn validate_relocation(&self, prepared: &Relocation) -> Result<(), HeapError> {
        for (key, record) in self.descriptors.iter() {
            let owner = match record.meaning {
                Meaning::Alias { base } => base,
                Meaning::Value { .. } | Meaning::Bound { .. } | Meaning::Statistical => key,
            };
            let index = prepared
                .locations
                .binary_search_by_key(&owner, |(entry, _)| *entry)
                .map_err(|_| HeapError::InvalidLayout)?;
            let (_, region) = prepared
                .locations
                .get(index)
                .ok_or(HeapError::InvalidLayout)?;
            let old = self.owning_region(record)?;
            if region.length != old.length || region.format != old.format {
                return Err(HeapError::InvalidLayout);
            }
            prepared.storage.resolve(*region)?;
            record
                .layout
                .validate(region.length, self.limits.layout_work)?;
        }
        Ok(())
    }

    /// No allocation, fallible checks or caller callbacks remain under the exclusive borrow.
    fn publish_relocation(&mut self, prepared: Relocation) {
        let owners = self.descriptors.iter_mut().filter_map(|(_, record)| {
            if matches!(record.location, Location::Owned(_)) {
                Some(record)
            } else {
                None
            }
        });
        for (record, (_, region)) in owners.zip(prepared.locations) {
            record.location = Location::Owned(region);
        }
        self.storage = prepared.storage;
    }
}

/// Copy one complete owned allocation through safe typed access into a fresh slab.
fn copy_owner<P: Scalar + 'static>(
    source: &Storage,
    region: Region,
    replacement: &mut Storage,
    limits: HeapLimits,
) -> Result<Region, HeapError> {
    let slab = source.resolve(region)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(region.length)
        .map_err(|_| HeapError::Capacity)?;
    for index in 0..region.length {
        let address = region
            .offset
            .checked_add(index)
            .ok_or(HeapError::ExtentOverflow)?;
        entries.push(slab.entry::<P>(address)?);
    }
    let payload = entries.as_slice();
    let prepared = replacement.prepare(std::slice::from_ref(&payload), &[], limits)?;
    let arena = *prepared.keys().first().ok_or(HeapError::Capacity)?;
    let moved = Region {
        epoch: prepared.epoch(),
        arena,
        offset: 0,
        ..region
    };
    replacement.publish(prepared);
    Ok(moved)
}

/// Inaccessible preparation faults and commit-boundary checks.
#[cfg(test)]
mod tests {
    use nrz_bindings::Descriptor;

    use super::{Heap, HeapError, HeapLimits};
    use crate::{Extent, Layout};

    /// An invalid alias prevents all relocation, even when its base and other maps are valid.
    /// ´test:unit:compaction-revalidates-every-alias-map´
    #[test]
    fn compaction_revalidates_every_alias_map() -> Result<(), HeapError> {
        let mut heap = Heap::new(HeapLimits::default())?;
        let base =
            heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0_f64, 2.0])?;
        let selection = Layout::strided(Extent::new(1, 1)?, 2, 1, 0, 0)?;
        let alias = heap.alias(base.value(), &selection)?;
        let before = heap.value(base.value())?.stamp();
        let epoch = heap.value(base.value())?.region().layout_epoch();
        heap.descriptors.get_mut(alias.key)?.layout =
            Layout::strided(Extent::new(1, 1)?, 3, 2, 0, 0)?;
        assert_eq!(heap.compact(), Err(HeapError::InvalidLayout));
        assert_eq!(heap.value(base.value())?.stamp(), before);
        assert_eq!(heap.value(base.value())?.region().layout_epoch(), epoch);
        heap.descriptors.get_mut(alias.key)?.layout = selection;
        heap.compact()?;
        assert_eq!(heap.value(alias)?.region().layout_epoch(), epoch + 1);
        assert_eq!(
            heap.value(alias)?.region().get(0, 0)?.to_bits(),
            2.0_f64.to_bits()
        );
        Ok(())
    }

    /// Prepared replacement regions are checked before any visible indirection changes.
    /// ´test:unit:prepared-relocation-refuses-changed-extents´
    #[test]
    fn prepared_relocation_refuses_changed_extents() -> Result<(), HeapError> {
        let mut heap = Heap::new(HeapLimits::default())?;
        let base = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[7.0_f64])?;
        let stamp = heap.value(base.value())?.stamp();
        let epoch = heap.value(base.value())?.region().layout_epoch();
        let mut prepared = heap.prepare_relocation()?;
        let (_, region) = prepared
            .locations
            .first_mut()
            .ok_or(HeapError::InvalidLayout)?;
        region.length = 2;
        assert_eq!(
            heap.validate_relocation(&prepared),
            Err(HeapError::InvalidLayout)
        );
        assert_eq!(heap.value(base.value())?.stamp(), stamp);
        assert_eq!(heap.value(base.value())?.region().layout_epoch(), epoch);
        assert_eq!(
            heap.value(base.value())?.region().get(0, 0)?.to_bits(),
            7.0_f64.to_bits()
        );
        Ok(())
    }
}
