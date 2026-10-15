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
use crate::{Layout, PackedRegion};

/// Checked, read-only physical entry selection and its owning homogeneous descriptor.
#[derive(Debug)]
pub struct RegionView<'h, P: Scalar> {
    /// Borrowed numerical owner.
    slab: SlabRef<'h>,
    /// Checked allocated range.
    region: Region,
    /// Checked logical map into the allocated range.
    layout: &'h Layout,
    /// Maximum entries copied into raw or backend scratch.
    packing_limit: usize,
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
        self.layout.extent()
    }

    /// This region's quantity descriptor, including its role and exact measure.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.schema.quantity
    }

    /// Exact owning schema identity before selection; this is not a value or source identity.
    #[must_use]
    pub const fn schema_id(&self) -> SchemaId {
        self.schema_id
    }

    /// Read one entry through checked column-vectorization arithmetic.
    ///
    /// # Errors
    /// Refuses out-of-range indices, address overflow or a mismatched storage format.
    pub fn get(&self, row: usize, column: usize) -> Result<P, HeapError> {
        self.read_flat(self.layout.extent().index(row, column)?)
    }

    /// Export an owned column-ordered copy with no heap evidence or source independence.
    /// Reimport requires a new explicit coverage declaration.
    ///
    /// # Errors
    /// Refuses allocation failure or an invalid resolved region.
    pub fn export(&self) -> Result<Vec<P>, HeapError> {
        let mut copy = Vec::new();
        crate::layout::check_work(self.layout.extent().entries(), self.packing_limit)?;
        copy.try_reserve_exact(self.layout.extent().entries())
            .map_err(|_| HeapError::Capacity)?;
        for index in 0..self.layout.extent().entries() {
            copy.push(self.read_flat(index)?);
        }
        Ok(copy)
    }

    /// Physical relocation epoch; mathematical versions need not change when it advances.
    #[must_use]
    pub const fn layout_epoch(&self) -> u64 {
        self.region.epoch
    }

    /// The selected logical entries, relative to the owning region.
    #[must_use]
    pub const fn layout(&self) -> &Layout {
        self.layout
    }

    /// Pack into a fresh, contiguous, privately owned faer slab in logical column order.
    ///
    /// This raw snapshot has no current heap evidence and cannot mutate published storage.
    ///
    /// # Errors
    /// Refuses packing work or capacity exhaustion and invalid resolved addresses.
    pub fn pack(&self) -> Result<PackedRegion<P>, HeapError> {
        PackedRegion::new(self.extent(), &self.export()?)
    }

    /// Read inside the allocated region, never merely inside slab capacity.
    fn read_flat(&self, index: usize) -> Result<P, HeapError> {
        let selected = self.layout.flat_index(index)?;
        let address = self
            .region
            .offset
            .checked_add(selected)
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

    /// Handle to the entire base value's bound; access must resolve it again.
    /// Use `Heap::selected_bound` to apply this value selection to its bound entries.
    #[must_use]
    pub const fn bound(&self) -> Option<Handle<Companion, P>> {
        self.bound
    }
}

/// A declared absolute-bound region tied to one current value version.
///
/// Its nonnegative entries cover every selected target entry in the same logical order.
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

    /// Base target occupation; `RegionView::layout` records its selected logical entries.
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
        self.validate_provenance(handle)?;
        let base = self.base_handle(handle)?;
        let Meaning::Value { stamp, bound } = self.record(base)?.meaning else {
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
        self.validate_provenance(target_handle)?;
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
        if self.owning_region(record)?.format != Format::of::<P>() {
            return Err(HeapError::PrecisionMismatch);
        }
        Ok(record)
    }

    /// Combine checked physical storage with its own immutable entry selection.
    pub(crate) fn region_view<'h, P: Scalar + 'static>(
        &'h self,
        record: &'h Record,
    ) -> Result<RegionView<'h, P>, HeapError> {
        self.mapped_region(record, &record.layout)
    }

    /// Apply a checked selection to a value or its whole-base bound region.
    fn mapped_region<'h, P: Scalar + 'static>(
        &'h self,
        record: &'h Record,
        layout: &'h Layout,
    ) -> Result<RegionView<'h, P>, HeapError> {
        let region = self.owning_region(record)?;
        if region.format != Format::of::<P>() {
            return Err(HeapError::PrecisionMismatch);
        }
        let schema = self
            .schemas
            .get(record.schema)
            .ok_or(HeapError::InvalidLayout)?;
        if region.length != schema.extent.entries() {
            return Err(HeapError::InvalidLayout);
        }
        layout.validate(region.length, self.limits.layout_work)?;
        Ok(RegionView {
            slab: self.storage.resolve(region)?,
            region,
            layout,
            packing_limit: self.limits.layout_work.min(self.limits.elements),
            schema,
            schema_id: SchemaId {
                scope: self.scope,
                index: record.schema,
            },
            marker: PhantomData,
        })
    }

    /// Resolve the current bound through exactly the value's selected entry map.
    ///
    /// The returned target is the base value, with its current version. Its region's
    /// layout gives the corresponding base-target entry for every selected bound.
    ///
    /// # Errors
    /// Refuses invalid handles, maps or stale evidence. Returns `None` for represented coverage.
    pub fn selected_bound<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<Option<CompanionView<'_, P>>, HeapError> {
        let value = self.value(handle)?;
        let Some(bound) = value.bound() else {
            return Ok(None);
        };
        let mut companion = self.companion(bound)?;
        companion.region = self.mapped_region(self.record(bound)?, &self.record(handle)?.layout)?;
        Ok(Some(companion))
    }
}
