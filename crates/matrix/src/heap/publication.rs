// SPDX-License-Identifier: AGPL-3.0-only
//! Import preparation and the allocation-free, callback-free publication step.

use nrz_scalar::Scalar;
use nrz_units::{Descriptor, UncertaintyMode, uncertainty};

use super::identity::{Key, identity};
use super::records::{Meaning, Record};
use super::schema::{Schema, check_quantity, intern};
use super::{Extent, Handle, Heap, HeapError, PublishedBundle, ValueId, ValueStamp, ValueVersion};
use crate::storage::{Format, PreparedStorage, Region};

/// Fully prepared import, inaccessible until its complete bundle is linked.
#[derive(Debug)]
struct PreparedImport<P: Scalar> {
    /// Private backend allocations.
    storage: PreparedStorage,
    /// New immutable interpretations.
    schemas: Vec<Schema>,
    /// Complete value and optional companion descriptors.
    records: Vec<Record>,
    /// Future locators, returned only after publication.
    bundle: PublishedBundle<P>,
}

/// Schema indices and any new immutable records, still private to preparation.
#[derive(Debug)]
struct SchemaPlan {
    /// Exact schemas not already in the heap.
    pending: Vec<Schema>,
    /// Value interpretation.
    value: usize,
    /// Independently derived radius interpretation.
    bound: Option<usize>,
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
        self.storage.publish(prepared.storage);
        self.schemas.extend(prepared.schemas);
        self.descriptors.publish(prepared.records.into_iter());
        Ok(prepared.bundle)
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
        let keys = self.descriptors.prepare(count, self.limits.descriptors)?;
        let arenas = self
            .storage
            .reserve::<P>(count, extent.entries(), self.limits)?;
        let stamp = ValueStamp {
            id: ValueId(identity()?),
            version: ValueVersion(1),
        };
        let (records, bundle) =
            prepare_records::<P>(self.scope, extent, &keys, &arenas, &schema_plan, stamp)?;
        let storage = PreparedStorage::new(centers, bounds)?;
        Ok(PreparedImport {
            storage,
            schemas: schema_plan.pending,
            records,
            bundle,
        })
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
fn prepare_records<P: Scalar + 'static>(
    scope: u64,
    extent: Extent,
    keys: &[Key],
    arenas: &[Key],
    schemas: &SchemaPlan,
    stamp: ValueStamp,
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
        region: Region {
            arena: value_arena,
            format: Format::of::<P>(),
            offset: 0,
            length: extent.entries(),
        },
        meaning: Meaning::Value {
            stamp,
            bound: bound_key,
        },
    });
    if let Some(schema) = schemas.bound {
        let arena = *arenas.get(1).ok_or(HeapError::Capacity)?;
        records.push(Record {
            schema,
            region: Region {
                arena,
                format: Format::of::<P>(),
                offset: 0,
                length: extent.entries(),
            },
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
