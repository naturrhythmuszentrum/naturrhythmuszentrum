// SPDX-License-Identifier: AGPL-3.0-only
//! Direct base-value indirection for composed, read-only entry selections.

use nrz_scalar::Scalar;

use super::records::{Location, Meaning, Record};
use super::{Handle, Heap, HeapError, Value};
use crate::Layout;
use crate::storage::Region;

impl Heap {
    /// Create a descriptor selecting the parent's logical column-ordered entries.
    ///
    /// Nested selections compose to offsets in the original base region. The alias
    /// shares its identity, current version and coverage; it does not create a source.
    /// Read-only broadcasts and repeated gathers are admitted.
    ///
    /// # Errors
    /// Refuses invalid handles, mismatched selection lengths, work or descriptor capacity
    /// exhaustion. Failure leaves the parent and all existing descriptors unchanged.
    pub fn alias<P: Scalar + 'static>(
        &mut self,
        parent: Handle<Value, P>,
        selection: &Layout,
    ) -> Result<Handle<Value, P>, HeapError> {
        self.value(parent)?;
        let base = self.base_handle(parent)?;
        let source = self.record(parent)?;
        let layout = source.layout.compose(selection, self.limits.layout_work)?;
        let record = Record {
            schema: source.schema,
            location: Location::Alias,
            layout,
            meaning: Meaning::Alias { base: base.key },
        };
        let plan = self.descriptors.prepare(1, self.limits.descriptors)?;
        let key = *plan.keys().first().ok_or(HeapError::Capacity)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(1)
            .map_err(|_| HeapError::Capacity)?;
        records.push(record);
        self.descriptors.publish(plan.bind(records)?);
        Ok(Handle::new(self.scope, key))
    }

    /// Check that all destinations are injective and mutually disjoint.
    ///
    /// Dense regions use an exact range proof. Other maps enumerate and sort their
    /// combined destinations under the heap's layout work limit. This check grants
    /// no mutable borrow; publication rechecks destinations under its exclusive borrow.
    ///
    /// # Errors
    /// Refuses invalid handles, output overlap, work exhaustion or preparation capacity.
    pub fn validate_outputs<P: Scalar + 'static>(
        &self,
        handles: &[Handle<Value, P>],
    ) -> Result<(), HeapError> {
        if handles.len() > self.limits.descriptors {
            return Err(HeapError::Capacity);
        }
        let mut destinations = Vec::new();
        destinations
            .try_reserve_exact(handles.len())
            .map_err(|_| HeapError::Capacity)?;
        for &handle in handles {
            self.value(handle)?;
            let record = self.record(handle)?;
            destinations.push((self.owning_region(record)?, &record.layout));
        }
        crate::layout::validate_outputs(&destinations, self.limits.layout_work)
    }

    /// Resolve the direct base occupation without permitting alias chains or category changes.
    pub(crate) fn base_handle<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<Handle<Value, P>, HeapError> {
        match self.record(handle)?.meaning {
            Meaning::Value { .. } => Ok(handle),
            Meaning::Alias { base } => {
                let target = Handle::new(self.scope, base);
                if !matches!(self.record(target)?.meaning, Meaning::Value { .. }) {
                    return Err(HeapError::StaleHandle);
                }
                Ok(target)
            }
            Meaning::Bound { .. } => Err(HeapError::CategoryMismatch),
        }
    }

    /// Follow one alias edge, checking that the allocation still belongs to a base value.
    pub(crate) fn owning_region(&self, record: &Record) -> Result<Region, HeapError> {
        match (&record.location, &record.meaning) {
            (Location::Owned(region), Meaning::Value { .. } | Meaning::Bound { .. }) => Ok(*region),
            (Location::Alias, Meaning::Alias { base }) => {
                let owner = self.descriptors.get(*base)?;
                match (&owner.location, &owner.meaning) {
                    (Location::Owned(region), Meaning::Value { .. })
                        if owner.schema == record.schema =>
                    {
                        Ok(*region)
                    }
                    _ => Err(HeapError::StaleHandle),
                }
            }
            _ => Err(HeapError::InvalidLayout),
        }
    }
}
