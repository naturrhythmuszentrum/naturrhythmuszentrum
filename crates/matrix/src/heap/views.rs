// SPDX-License-Identifier: AGPL-3.0-only
//! Checked borrows, independent companion access and explicit raw copies.

use std::marker::PhantomData;

use nrz_scalar::Scalar;
use nrz_units::Descriptor;

use super::records::{Meaning, Record};
use super::schema::Schema;
use super::{
    Category, Companion, Coverage, Extent, Handle, Heap, HeapError, SchemaId, Value, ValueStamp,
};
use crate::storage::{Format, Region, SlabRef};

/// Checked, read-only contiguous physical coordinates and their own descriptor.
#[derive(Debug)]
pub struct RegionView<'h, P: Scalar> {
    /// Borrowed numerical owner.
    slab: SlabRef<'h>,
    /// Checked allocated range.
    region: Region,
    /// Immutable homogeneous interpretation.
    schema: &'h Schema,
    /// Scoped immutable schema identity.
    schema_id: SchemaId,
    /// Sealed scalar type.
    marker: PhantomData<P>,
}

impl<P: Scalar + 'static> RegionView<'_, P> {
    /// Logical extents, independent of backend padding.
    #[must_use]
    pub const fn extent(&self) -> Extent {
        self.schema.extent
    }

    /// This region's quantity descriptor, including its role and exact measure.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.schema.quantity
    }

    /// Exact interned schema identity; this is not a value or source identity.
    #[must_use]
    pub const fn schema_id(&self) -> SchemaId {
        self.schema_id
    }

    /// Read one entry through checked column-vectorization arithmetic.
    ///
    /// # Errors
    /// Refuses out-of-range indices, address overflow or a mismatched storage format.
    pub fn get(&self, row: usize, column: usize) -> Result<P, HeapError> {
        self.read_flat(self.schema.extent.index(row, column)?)
    }

    /// Export an owned column-ordered copy with no heap evidence or source independence.
    /// Reimport requires a new explicit coverage declaration.
    ///
    /// # Errors
    /// Refuses allocation failure or an invalid resolved region.
    pub fn export(&self) -> Result<Vec<P>, HeapError> {
        let mut copy = Vec::new();
        copy.try_reserve_exact(self.region.length)
            .map_err(|_| HeapError::Capacity)?;
        for index in 0..self.region.length {
            copy.push(self.read_flat(index)?);
        }
        Ok(copy)
    }

    /// Read inside the allocated region, never merely inside slab capacity.
    fn read_flat(&self, index: usize) -> Result<P, HeapError> {
        if index >= self.region.length {
            return Err(HeapError::InvalidLayout);
        }
        let address = self
            .region
            .offset
            .checked_add(index)
            .ok_or(HeapError::ExtentOverflow)?;
        self.slab.entry(address)
    }
}

/// A current value borrow with coverage and a captured dependency version.
#[derive(Debug)]
pub struct ValueView<'h, P: Scalar> {
    /// Checked center region.
    region: RegionView<'h, P>,
    /// Current value identity and version.
    stamp: ValueStamp,
    /// Current declared-bound relation, if any.
    bound: Option<Handle<Companion, P>>,
}

impl<P: Scalar> ValueView<'_, P> {
    /// Center region and its physical interpretation.
    #[must_use]
    pub const fn region(&self) -> &RegionView<'_, P> {
        &self.region
    }

    /// Snapshot of the current value identity and content version.
    #[must_use]
    pub const fn stamp(&self) -> ValueStamp {
        self.stamp
    }

    /// Explicit supported deterministic coverage.
    #[must_use]
    pub const fn coverage(&self) -> Coverage {
        if self.bound.is_some() {
            Coverage::DeclaredBounds
        } else {
            Coverage::RepresentedInputs
        }
    }

    /// Handle to the independently described bound; access must resolve it again.
    #[must_use]
    pub const fn bound(&self) -> Option<Handle<Companion, P>> {
        self.bound
    }
}

/// A declared absolute-bound region tied to one current value version.
///
/// Its nonnegative entries cover every target entry in the same logical order.
/// The bounds remain caller assumptions; no statistical or numerical solve claim follows.
#[derive(Debug)]
pub struct CompanionView<'h, P: Scalar> {
    /// Bound region with its own radius descriptor.
    region: RegionView<'h, P>,
    /// Validated entire-region target.
    target: Handle<Value, P>,
    /// Captured and rechecked target version.
    stamp: ValueStamp,
}

impl<P: Scalar> CompanionView<'_, P> {
    /// Independent bound storage and quantity descriptor.
    #[must_use]
    pub const fn region(&self) -> &RegionView<'_, P> {
        &self.region
    }

    /// Target value occupation; every entry is selected in logical column order.
    #[must_use]
    pub const fn target(&self) -> Handle<Value, P> {
        self.target
    }

    /// The precise target identity and version validated for this borrow.
    #[must_use]
    pub const fn target_stamp(&self) -> ValueStamp {
        self.stamp
    }
}

impl Heap {
    /// Resolve a value's scope, occupation, category, format and allocated range.
    ///
    /// # Errors
    /// Refuses foreign/stale handles, wrong categories or formats, and invalid regions.
    pub fn value<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<ValueView<'_, P>, HeapError> {
        let record = self.record(handle)?;
        let Meaning::Value { stamp, bound } = record.meaning else {
            return Err(HeapError::CategoryMismatch);
        };
        if let Some(key) = bound {
            self.companion(Handle::<Companion, P>::new(self.scope, key))?;
        }
        Ok(ValueView {
            region: self.region_view(record)?,
            stamp,
            bound: bound.map(|key| Handle::new(self.scope, key)),
        })
    }

    /// Resolve an absolute bound and recheck its exact target occupation and version.
    ///
    /// # Errors
    /// Refuses invalid handles/layouts and any stale or inconsistent target relation.
    pub fn companion<P: Scalar + 'static>(
        &self,
        handle: Handle<Companion, P>,
    ) -> Result<CompanionView<'_, P>, HeapError> {
        let record = self.record(handle)?;
        let Meaning::Bound { target, stamp } = record.meaning else {
            return Err(HeapError::CategoryMismatch);
        };
        let target_handle = Handle::<Value, P>::new(self.scope, target);
        let target_record = self
            .record(target_handle)
            .map_err(|_| HeapError::StaleEvidence)?;
        if !matches!(target_record.meaning, Meaning::Value { stamp: current, bound: Some(key) }
            if current == stamp && key == handle.key)
        {
            return Err(HeapError::StaleEvidence);
        }
        let target_view = self.region_view::<P>(target_record)?;
        let region = self.region_view(record)?;
        if region.extent() != target_view.extent() {
            return Err(HeapError::StaleEvidence);
        }
        Ok(CompanionView {
            region,
            target: target_handle,
            stamp,
        })
    }

    /// Refuse foreign ownership before touching any local slot.
    pub(crate) fn record<C: Category, P: Scalar + 'static>(
        &self,
        handle: Handle<C, P>,
    ) -> Result<&Record, HeapError> {
        if handle.scope != self.scope {
            return Err(HeapError::ForeignHandle);
        }
        let record = self.descriptors.get(handle.key)?;
        if record.region.format != Format::of::<P>() {
            return Err(HeapError::PrecisionMismatch);
        }
        Ok(record)
    }

    /// Combine checked physical storage with its immutable interpretation.
    fn region_view<P: Scalar + 'static>(
        &self,
        record: &Record,
    ) -> Result<RegionView<'_, P>, HeapError> {
        if record.region.format != Format::of::<P>() {
            return Err(HeapError::PrecisionMismatch);
        }
        let schema = self
            .schemas
            .get(record.schema)
            .ok_or(HeapError::InvalidLayout)?;
        if record.region.length != schema.extent.entries() {
            return Err(HeapError::InvalidLayout);
        }
        Ok(RegionView {
            slab: self.storage.resolve(record.region)?,
            region: record.region,
            schema,
            schema_id: SchemaId {
                scope: self.scope,
                index: record.schema,
            },
            marker: PhantomData,
        })
    }
}
