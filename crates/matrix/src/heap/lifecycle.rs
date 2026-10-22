// SPDX-License-Identifier: AGPL-3.0-only
//! Checked retirement and replacement over the supported whole-region dependency graph.

use nrz_scalar::Scalar;

use super::identity::Key;
use super::records::Meaning;
use super::schema::Schema;
use super::{Handle, Heap, HeapError, PublishedBundle, Value, ValueStamp, ValueVersion};
use crate::storage::Region;

/// Validated current bundle; its private snapshot is used only under the exclusive change borrow.
#[derive(Debug)]
pub(crate) struct Snapshot {
    /// Surviving value occupation.
    pub(crate) key: Key,
    /// Captured identity and version.
    pub(crate) stamp: ValueStamp,
    /// Immutable physical interpretation, unchanged by supported replacements.
    pub(crate) schema: Schema,
    /// Current value allocation.
    pub(crate) region: Region,
    /// Reverse dependency edge and allocation of the current bound.
    pub(crate) bound: Option<(Key, Region)>,
    /// Additional evaluation allocation retired with its complete target bundle.
    pub(crate) evaluation: Option<(Key, Region)>,
    /// Product contribution allocations retire with the complete target bundle.
    pub(crate) contributions: Option<[(Key, Region); 3]>,
}

impl Snapshot {
    /// Complete old allocation set, validated before any retirement takes effect.
    pub(crate) fn regions(&self) -> Result<Vec<Region>, HeapError> {
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(6)
            .map_err(|_| HeapError::Capacity)?;
        regions.push(self.region);
        regions.extend(self.bound.map(|(_, region)| region));
        regions.extend(self.evaluation.map(|(_, region)| region));
        regions.extend(
            self.contributions
                .into_iter()
                .flatten()
                .map(|(_, region)| region),
        );
        Ok(regions)
    }

    /// Retire the bound edge, and optionally the base value occupation.
    pub(crate) fn retire_keys(&self, include_value: bool) -> Result<Vec<Key>, HeapError> {
        let mut keys = Vec::new();
        keys.try_reserve_exact(6).map_err(|_| HeapError::Capacity)?;
        if include_value {
            keys.push(self.key);
        }
        keys.extend(self.bound.map(|(key, _)| key));
        keys.extend(self.evaluation.map(|(key, _)| key));
        keys.extend(self.contributions.into_iter().flatten().map(|(key, _)| key));
        Ok(keys)
    }
}

impl Heap {
    /// Replace selected centers while reducing the entire base to represented coverage.
    ///
    /// The handle and value identity survive; the version advances even for identical bits.
    /// All aliases resolve the advanced base version. Old bound occupations are retired
    /// atomically. Extent, quantity and format stay fixed; overlapping destinations refuse.
    ///
    /// # Errors
    /// Refuses invalid handles, a mismatched expected version, exhausted counters,
    /// nonfinite/wrong-length payloads or insufficient preparation capacity.
    /// Every refusal leaves the old value, version and companion readable.
    pub fn replace_represented<P: Scalar + 'static>(
        &mut self,
        handle: Handle<Value, P>,
        expected: ValueVersion,
        centers: &[P],
    ) -> Result<PublishedBundle<P>, HeapError> {
        self.replace(handle, expected, centers, None)
    }

    /// Atomically replace selected centers and their declared componentwise bounds.
    ///
    /// Bounds are fresh assumptions for the new version, including with identical centers.
    /// Unselected entries keep their current centers and bounds. A partial selection of
    /// a represented-only base refuses because bounds for the remaining entries are unknown.
    /// The entire base advances once; unrelated allocations retain their versions.
    ///
    /// # Errors
    /// In addition to represented-replacement refusals, rejects invalid radii and
    /// unsupported radius derivation. No old evidence survives as evidence for the new version.
    pub fn replace_declared_bounds<P: Scalar + 'static>(
        &mut self,
        handle: Handle<Value, P>,
        expected: ValueVersion,
        centers: &[P],
        bounds: &[P],
    ) -> Result<PublishedBundle<P>, HeapError> {
        self.replace(handle, expected, centers, Some(bounds))
    }

    /// Retire a value and its entire supported companion set, releasing their arenas.
    ///
    /// Copies of all retired handles remain stale after slot reuse. Exhausted generations
    /// retire permanently; immutable schemas remain in the heap's lifetime-bounded pool.
    ///
    /// # Errors
    /// Refuses foreign/stale handles, invalid evidence or a failed preparation allocation.
    /// Retirement is atomic, including when both value and bound have to be released.
    pub fn retire<P: Scalar + 'static>(
        &mut self,
        handle: Handle<Value, P>,
    ) -> Result<(), HeapError> {
        if matches!(self.record(handle)?.meaning, Meaning::Alias { .. }) {
            let key = handle.key;
            let plan = self.descriptors.change(
                0,
                None,
                std::slice::from_ref(&key),
                self.limits.descriptors,
            )?;
            self.descriptors.publish(plan.bind(Vec::new())?);
            return Ok(());
        }
        let snapshot = self.retirement_snapshot(handle)?;
        let mut keys = snapshot.retire_keys(true)?;
        for (key, record) in self.descriptors.iter() {
            if matches!(record.meaning, Meaning::Alias { base } if base == snapshot.key) {
                keys.try_reserve(1).map_err(|_| HeapError::Capacity)?;
                keys.push(key);
            }
        }
        let regions = snapshot.regions()?;
        let plan = self
            .descriptors
            .change(0, None, &keys, self.limits.descriptors)?;
        let descriptors = plan.bind(Vec::new())?;
        let storage = self.storage.prepare::<P>(&[], &regions, self.limits)?;
        self.storage.publish(storage);
        self.descriptors.publish(descriptors);
        Ok(())
    }

    /// Resolve every dependency before capturing the private retirement set.
    pub(crate) fn snapshot<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
        expected: Option<ValueVersion>,
    ) -> Result<Snapshot, HeapError> {
        let view = self.value(handle)?;
        let stamp = view.stamp();
        if let Some(expected) = expected
            && expected != stamp.version
        {
            return Err(HeapError::VersionMismatch {
                expected,
                actual: stamp.version,
            });
        }
        let record = self.record(handle)?;
        let Meaning::Value {
            bound,
            contributions,
            ..
        } = record.meaning
        else {
            return Err(HeapError::CategoryMismatch);
        };
        let companion = bound
            .map(|key| {
                self.descriptors
                    .get(key)
                    .and_then(|entry| self.owning_region(entry).map(|region| (key, region)))
            })
            .transpose()?;
        Ok(Snapshot {
            key: handle.key,
            stamp,
            schema: Schema {
                extent: view.region().extent(),
                quantity: view.region().descriptor().clone(),
            },
            region: self.owning_region(record)?,
            bound: companion,
            contributions: self.contribution_regions::<P>(
                contributions,
                handle.key,
                stamp,
                view.region().extent(),
            )?,
            evaluation: view
                .evaluation()
                .map(|handle| {
                    self.record(handle).and_then(|entry| {
                        self.owning_region(entry).map(|region| (handle.key, region))
                    })
                })
                .transpose()?,
        })
    }
}

/// Inaccessible version and dependency-corruption boundaries.
#[cfg(test)]
mod tests {
    use nrz_bindings::Descriptor;

    use super::super::{Extent, HeapLimits};
    use super::{Heap, HeapError, Meaning, ValueVersion};

    /// A maximum content version refuses replacement without changing bytes or identity.
    /// ´test:unit:value-version-exhaustion-is-atomic´
    #[test]
    fn value_version_exhaustion_is_atomic() -> Result<(), HeapError> {
        let mut heap = Heap::new(HeapLimits::new(1, 1, 1))?;
        let bundle =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let record = heap.descriptors.get_mut(bundle.value().key)?;
        let Meaning::Value { stamp, .. } = &mut record.meaning else {
            return Err(HeapError::CategoryMismatch);
        };
        stamp.version = ValueVersion(u64::MAX);
        let captured = heap.value(bundle.value())?.stamp();
        assert!(matches!(
            heap.replace_represented(bundle.value(), captured.version(), &[3.0]),
            Err(HeapError::CounterExhausted)
        ));
        assert_eq!(heap.value(bundle.value())?.stamp(), captured);
        assert_eq!(
            heap.value(bundle.value())?.region().get(0, 0)?.to_bits(),
            2.0_f64.to_bits()
        );
        heap.retire(bundle.value())?;
        let fresh =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f64])?;
        assert_ne!(heap.value(fresh.value())?.stamp().id(), captured.id());
        assert_eq!(heap.value(fresh.value())?.stamp().version().number(), 1);
        Ok(())
    }

    /// Even equal bytes and live slots cannot authorize evidence for another target version.
    /// ´test:unit:companion-access-rechecks-captured-version´
    #[test]
    fn companion_access_rechecks_captured_version() -> Result<(), HeapError> {
        let mut heap = Heap::new(HeapLimits::default())?;
        let bundle = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[2.0_f64],
            &[0.5],
        )?;
        let companion = bundle.bound().ok_or(HeapError::StaleEvidence)?;
        let record = heap.descriptors.get_mut(bundle.value().key)?;
        let Meaning::Value { stamp, .. } = &mut record.meaning else {
            return Err(HeapError::CategoryMismatch);
        };
        stamp.version = ValueVersion(2);
        assert!(matches!(
            heap.companion(companion),
            Err(HeapError::StaleEvidence)
        ));
        assert!(matches!(
            heap.value(bundle.value()),
            Err(HeapError::StaleEvidence)
        ));
        Ok(())
    }

    /// A forged forward target must also agree with the target's reverse companion edge.
    /// ´test:unit:companion-access-rechecks-reverse-edge´
    #[test]
    fn companion_access_rechecks_reverse_edge() -> Result<(), HeapError> {
        let mut heap = Heap::new(HeapLimits::default())?;
        let extent = Extent::new(1, 1)?;
        let quantity = Descriptor::ratio();
        let first = heap.import_declared_bounds(extent, &quantity, &[2.0_f64], &[0.5])?;
        let second = heap.import_declared_bounds(extent, &quantity, &[2.0_f64], &[0.5])?;
        let current = heap.value(second.value())?.stamp();
        let companion = first.bound().ok_or(HeapError::StaleEvidence)?;
        let record = heap.descriptors.get_mut(companion.key)?;
        record.meaning = Meaning::Bound {
            target: second.value().key,
            stamp: current,
        };
        assert!(matches!(
            heap.companion(companion),
            Err(HeapError::StaleEvidence)
        ));
        assert_eq!(heap.value(second.value())?.stamp(), current);
        Ok(())
    }
}
