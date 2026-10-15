// SPDX-License-Identifier: AGPL-3.0-only
//! Import preparation and the allocation-free, callback-free publication step.

use nrz_scalar::Scalar;
use nrz_units::{Descriptor, UncertaintyMode, uncertainty};

use super::identity::{Key, identity};
use super::records::{Location, Meaning, Record};
use super::schema::{Schema, check_quantity, intern};
use super::slots::{PreparedSlots, SlotPlan};
use super::{
    Extent, Handle, Heap, HeapError, PublishedBundle, Value, ValueId, ValueStamp, ValueVersion,
};
use crate::Layout;
use crate::storage::{Format, PreparedStorage, Region};

/// Fully prepared import, inaccessible until its complete bundle is linked.
#[derive(Debug)]
struct PreparedImport<P: Scalar> {
    /// Private backend allocations.
    storage: PreparedStorage,
    /// New immutable interpretations.
    schemas: Vec<Schema>,
    /// Complete value and optional companion descriptors.
    records: PreparedSlots<Record>,
    /// Future locators, returned only after publication.
    bundle: PublishedBundle<P>,
}

/// Schema indices and any new immutable records, still private to preparation.
#[derive(Debug)]
pub(crate) struct SchemaPlan {
    /// Exact schemas not already in the heap.
    pub(crate) pending: Vec<Schema>,
    /// Value interpretation.
    pub(crate) value: usize,
    /// Independently derived radius interpretation.
    pub(crate) bound: Option<usize>,
}

impl Heap {
    /// Import finite represented coordinates, with no claim of physical exactness.
    ///
    /// Entries use logical column order. Signed zero is canonicalized to positive zero.
    ///
    /// # Errors
    /// Refuses noncoherent/count descriptors, wrong lengths, nonfinite centers,
    /// address overflow, capacity exhaustion or exhausted identities.
    pub fn import_represented<P: Scalar + 'static>(
        &mut self,
        extent: Extent,
        quantity: &Descriptor,
        centers: &[P],
    ) -> Result<PublishedBundle<P>, HeapError> {
        self.import(extent, quantity, centers, None)
    }

    /// Atomically import a value and its declared componentwise absolute-bound region.
    ///
    /// The nonnegative finite bounds are assumptions on every corresponding value entry.
    /// Units derives a separate zero-offset radius descriptor; no statistic is promoted.
    ///
    /// # Errors
    /// In addition to represented-import refusals, rejects invalid bound lengths,
    /// negative/nonfinite radii and unsupported units uncertainty derivations.
    pub fn import_declared_bounds<P: Scalar + 'static>(
        &mut self,
        extent: Extent,
        quantity: &Descriptor,
        centers: &[P],
        bounds: &[P],
    ) -> Result<PublishedBundle<P>, HeapError> {
        self.import(extent, quantity, centers, Some(bounds))
    }

    /// Prepare completely, then publish under the same exclusive heap borrow.
    fn import<P: Scalar + 'static>(
        &mut self,
        extent: Extent,
        quantity: &Descriptor,
        centers: &[P],
        bounds: Option<&[P]>,
    ) -> Result<PublishedBundle<P>, HeapError> {
        let prepared = self.prepare_import(extent, quantity, centers, bounds)?;
        Ok(self.publish_bundle(prepared))
    }

    /// Finish every fallible check and allocation before linking any public record.
    fn prepare_import<P: Scalar + 'static>(
        &mut self,
        extent: Extent,
        quantity: &Descriptor,
        centers: &[P],
        bounds: Option<&[P]>,
    ) -> Result<PreparedImport<P>, HeapError> {
        check_quantity(quantity)?;
        scan(extent, centers, bounds)?;
        let schema_plan = self.prepare_schemas(extent, quantity, bounds.is_some())?;
        let count = 1 + usize::from(bounds.is_some());
        let descriptors = self.descriptors.prepare(count, self.limits.descriptors)?;
        let payloads = [centers, bounds.unwrap_or(&[])];
        let inputs = payloads.get(..count).ok_or(HeapError::LengthMismatch)?;
        let storage = self.storage.prepare(inputs, &[], self.limits)?;
        let stamp = ValueStamp {
            id: ValueId(identity()?),
            version: ValueVersion(1),
        };
        assemble(self.scope, extent, schema_plan, stamp, descriptors, storage)
    }

    /// Prepare a checked replacement without touching the old value or its reverse edges.
    pub(crate) fn replace_base<P: Scalar + 'static>(
        &mut self,
        handle: Handle<Value, P>,
        expected: ValueVersion,
        centers: &[P],
        bounds: Option<&[P]>,
    ) -> Result<PublishedBundle<P>, HeapError> {
        let snapshot = self.snapshot(handle, Some(expected))?;
        let next = snapshot
            .stamp
            .version
            .0
            .checked_add(1)
            .ok_or(HeapError::CounterExhausted)?;
        let extent = snapshot.schema.extent;
        scan(extent, centers, bounds)?;
        let schema_plan =
            self.prepare_schemas(extent, &snapshot.schema.quantity, bounds.is_some())?;
        let count = 1 + usize::from(bounds.is_some());
        let retired = snapshot.retire_keys(false)?;
        let descriptors = self.descriptors.change(
            count,
            Some(snapshot.key),
            &retired,
            self.limits.descriptors,
        )?;
        let payloads = [centers, bounds.unwrap_or(&[])];
        let inputs = payloads.get(..count).ok_or(HeapError::LengthMismatch)?;
        let storage = self
            .storage
            .prepare(inputs, &snapshot.regions()?, self.limits)?;
        let stamp = ValueStamp {
            id: snapshot.stamp.id,
            version: ValueVersion(next),
        };
        let prepared = assemble(self.scope, extent, schema_plan, stamp, descriptors, storage)?;
        Ok(self.publish_bundle(prepared))
    }

    /// The only visible bundle commit: no checks, allocations or caller code remain.
    fn publish_bundle<P: Scalar>(&mut self, prepared: PreparedImport<P>) -> PublishedBundle<P> {
        self.storage.publish(prepared.storage);
        self.schemas.extend(prepared.schemas);
        self.descriptors.publish(prepared.records);
        prepared.bundle
    }

    /// Derive and intern immutable metadata without changing the visible schema pool.
    fn prepare_schemas(
        &mut self,
        extent: Extent,
        quantity: &Descriptor,
        bounded: bool,
    ) -> Result<SchemaPlan, HeapError> {
        let mut pending = Vec::new();
        let value = intern(
            &self.schemas,
            &mut pending,
            Schema {
                extent,
                quantity: quantity.clone(),
            },
            self.limits.schemas,
        )?;
        let bound = if bounded {
            let radius = uncertainty(UncertaintyMode::Radius, quantity, None, None)?;
            Some(intern(
                &self.schemas,
                &mut pending,
                Schema {
                    extent,
                    quantity: radius,
                },
                self.limits.schemas,
            )?)
        } else {
            None
        };
        self.schemas
            .try_reserve(pending.len())
            .map_err(|_| HeapError::Capacity)?;
        Ok(SchemaPlan {
            pending,
            value,
            bound,
        })
    }
}

/// Bind matching descriptor payloads to their reserved occupations before publication.
fn assemble<P: Scalar + 'static>(
    scope: u64,
    extent: Extent,
    schemas: SchemaPlan,
    stamp: ValueStamp,
    descriptors: SlotPlan,
    storage: PreparedStorage,
) -> Result<PreparedImport<P>, HeapError> {
    let (records, bundle) = prepare_records::<P>(
        scope,
        extent,
        descriptors.keys(),
        storage.keys(),
        &schemas,
        stamp,
        storage.epoch(),
    )?;
    Ok(PreparedImport {
        storage,
        schemas: schemas.pending,
        records: descriptors.bind(records)?,
        bundle,
    })
}

/// Scan all payloads before any backend allocation; indices are logical column indices.
pub(crate) fn scan<P: Scalar>(
    extent: Extent,
    centers: &[P],
    bounds: Option<&[P]>,
) -> Result<(), HeapError> {
    if centers.len() != extent.entries() || bounds.is_some_and(|data| data.len() != centers.len()) {
        return Err(HeapError::LengthMismatch);
    }
    for (index, center) in centers.iter().enumerate() {
        if !center.is_finite() {
            return Err(HeapError::NonFinite { index });
        }
    }
    if let Some(data) = bounds {
        for (index, radius) in data.iter().enumerate() {
            if !radius.is_finite() || *radius < P::ZERO {
                return Err(HeapError::InvalidBound { index });
            }
        }
    }
    Ok(())
}

/// Prepare exact forward/reverse target edges and future external locators.
pub(crate) fn prepare_records<P: Scalar + 'static>(
    scope: u64,
    extent: Extent,
    keys: &[Key],
    arenas: &[Key],
    schemas: &SchemaPlan,
    stamp: ValueStamp,
    epoch: u64,
) -> Result<(Vec<Record>, PublishedBundle<P>), HeapError> {
    let target = *keys.first().ok_or(HeapError::Capacity)?;
    let value_arena = *arenas.first().ok_or(HeapError::Capacity)?;
    let bound_key = schemas
        .bound
        .map(|_| keys.get(1).copied().ok_or(HeapError::Capacity))
        .transpose()?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(keys.len())
        .map_err(|_| HeapError::Capacity)?;
    records.push(Record {
        schema: schemas.value,
        location: Location::Owned(Region {
            epoch,
            arena: value_arena,
            format: Format::of::<P>(),
            offset: 0,
            length: extent.entries(),
        }),
        layout: Layout::contiguous(extent)?,
        meaning: Meaning::Value {
            stamp,
            bound: bound_key,
        },
    });
    if let Some(schema) = schemas.bound {
        let arena = *arenas.get(1).ok_or(HeapError::Capacity)?;
        records.push(Record {
            schema,
            location: Location::Owned(Region {
                epoch,
                arena,
                format: Format::of::<P>(),
                offset: 0,
                length: extent.entries(),
            }),
            layout: Layout::contiguous(extent)?,
            meaning: Meaning::Bound { target, stamp },
        });
    }
    Ok((
        records,
        PublishedBundle {
            value: Handle::new(scope, target),
            bound: bound_key.map(|key| Handle::new(scope, key)),
        },
    ))
}
