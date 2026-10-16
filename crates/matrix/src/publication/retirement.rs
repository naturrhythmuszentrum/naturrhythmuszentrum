// SPDX-License-Identifier: AGPL-3.0-only
//! Structural retirement preparation never grants authority to read stale evidence.

use crate::heap::identity::Key;
use crate::heap::lifecycle::Snapshot;
use crate::heap::records::Meaning;
use crate::storage::Region;
use crate::{Companion, Extent, Handle, Heap, HeapError, Value, ValueStamp};
use nrz_scalar::Scalar;

impl Heap {
    /// Capture storage ownership for retirement without consulting upstream mathematical claims.
    /// This helper is crate-private and is called only by the existing retirement path.
    pub(crate) fn retirement_snapshot<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<Snapshot, HeapError> {
        let record = self.record(handle)?;
        let Meaning::Value {
            stamp,
            bound,
            evaluation,
            contributions,
            ..
        } = record.meaning
        else {
            return Err(HeapError::CategoryMismatch);
        };
        let view = self.region_view::<P>(record)?;
        let schema = self
            .schemas
            .get(record.schema)
            .ok_or(HeapError::InvalidLayout)?;
        if view.extent() != schema.extent {
            return Err(HeapError::InvalidLayout);
        }
        let companion = bound
            .map(|key| self.retired_companion::<P>(key, handle.key, stamp, view.extent()))
            .transpose()?;
        Ok(Snapshot {
            key: handle.key,
            stamp,
            schema: schema.clone(),
            region: self.owning_region(record)?,
            bound: companion,
            contributions: self.contribution_regions::<P>(
                contributions,
                handle.key,
                stamp,
                view.extent(),
            )?,
            evaluation: evaluation
                .map(|key| self.retired_companion::<P>(key, handle.key, stamp, view.extent()))
                .transpose()?,
        })
    }

    /// Check the exact direct target and reverse edge, format, schema and live allocation.
    fn retired_companion<P: Scalar + 'static>(
        &self,
        key: Key,
        target: Key,
        stamp: ValueStamp,
        extent: Extent,
    ) -> Result<(Key, Region), HeapError> {
        let record = self.record(Handle::<Companion, P>::new(self.scope, key))?;
        if !matches!(record.meaning, Meaning::Bound { target: owner, stamp: version }
            if owner == target && version == stamp)
        {
            return Err(HeapError::StaleEvidence);
        }
        let view = self.region_view::<P>(record)?;
        if view.extent() != extent {
            return Err(HeapError::StaleEvidence);
        }
        Ok((key, self.owning_region(record)?))
    }
}

/// Direct storage relation faults must refuse even though upstream assurance is irrelevant.
#[cfg(test)]
mod tests {
    use crate::binding_testing::TestResult;
    use crate::heap::records::Meaning;
    use crate::{Extent, Heap, HeapError, HeapLimits};
    use nrz_units::Descriptor;

    /// Raw retirement cannot follow a forged companion edge into another value's bundle.
    /// ´test:unit:retirement-refuses-a-foreign-direct-companion-target´
    #[test]
    fn retirement_refuses_a_foreign_direct_companion_target() -> TestResult {
        let mut heap = Heap::new(HeapLimits::default())?;
        let extent = Extent::new(1, 1)?;
        let first =
            heap.import_declared_bounds(extent, &Descriptor::ratio(), &[2.0_f64], &[0.25])?;
        let second =
            heap.import_declared_bounds(extent, &Descriptor::ratio(), &[3.0_f64], &[0.5])?;
        let key = first.bound().ok_or("missing bound")?.key;
        let Meaning::Bound { target, .. } = &mut heap.descriptors.get_mut(key)?.meaning else {
            return Err("missing companion".into());
        };
        *target = second.value().key;
        assert_eq!(heap.retire(first.value()), Err(HeapError::StaleEvidence));
        assert_eq!(heap.descriptors.iter().count(), 4);
        assert_eq!(
            heap.value(second.value())?.region().get(0, 0)?.to_bits(),
            3.0_f64.to_bits()
        );
        Ok(())
    }
}
