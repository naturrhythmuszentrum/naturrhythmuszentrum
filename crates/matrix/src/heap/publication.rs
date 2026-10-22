// SPDX-License-Identifier: AGPL-3.0-only
//! Import preparation and the allocation-free, callback-free publication step.

use nrz_bindings::{Descriptor, UncertaintyMode, uncertainty};
use nrz_scalar::Scalar;

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
    /// Prepared exact symbol owners, installed with their numerical regions.
    owners: Vec<crate::binding::symbols::Owner>,
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
    /// Separate numerical error descriptor when the recipe creates one.
    pub(crate) evaluation: Option<usize>,
    /// Radius schema shared by three independently stored product contributions.
    pub(crate) contributions: Option<usize>,
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
    /// Bindings derives a separate zero-offset radius descriptor; no statistic is promoted.
    ///
    /// # Errors
    /// In addition to represented-import refusals, rejects invalid bound lengths,
    /// negative/nonfinite radii and unsupported bindings uncertainty derivations.
    pub fn import_declared_bounds<P: Scalar + 'static>(
        &mut self,
        extent: Extent,
        quantity: &Descriptor,
        centers: &[P],
        bounds: &[P],
    ) -> Result<PublishedBundle<P>, HeapError> {
        self.import(extent, quantity, centers, Some(bounds))
    }

    /// Import through caller-predeclared value and companion symbols.
    ///
    /// Bounds and numerical import errors are explicit supplied assumptions. Error
    /// radii must be covered by the total; they are not added a second time.
    /// # Errors
    /// Refuses derived result symbols, incomplete symbols/payloads, nonfinite or
    /// negative radii, wrong coverage, invalid coordinates and exhausted capacities.
    pub fn import_symbols<P: Scalar + 'static>(
        &mut self,
        symbols: &nrz_bindings::BindingSymbols,
        centers: &[P],
        bounds: Option<&[P]>,
        errors: Option<&[P]>,
        coverage: super::Coverage,
    ) -> Result<crate::ValueBinding<P>, HeapError> {
        let prepared = self.prepare_symbol_import(symbols, centers, bounds, errors, coverage)?;
        let binding = imported_binding(self.scope, &prepared, symbols)?;
        self.publish_bundle(prepared);
        binding.publish();
        Ok(binding)
    }

    /// Existing convenience imports create exact declarations before numerical allocation.
    fn import<P: Scalar + 'static>(
        &mut self,
        extent: Extent,
        quantity: &Descriptor,
        centers: &[P],
        bounds: Option<&[P]>,
    ) -> Result<PublishedBundle<P>, HeapError> {
        check_quantity(quantity)?;
        let symbols = crate::binding::symbols::declare(
            quantity,
            extent,
            [bounds.is_some(), false],
            &mut crate::binding::symbols::budget()?,
        )?;
        let coverage = if bounds.is_some() {
            super::Coverage::DeclaredBounds
        } else {
            super::Coverage::RepresentedInputs
        };
        let prepared = self.prepare_symbol_import(&symbols, centers, bounds, None, coverage)?;
        Ok(self.publish_bundle(prepared))
    }

    /// Finish every fallible symbol, payload and allocation check before publication.
    fn prepare_symbol_import<P: Scalar + 'static>(
        &mut self,
        symbols: &nrz_bindings::BindingSymbols,
        centers: &[P],
        bounds: Option<&[P]>,
        errors: Option<&[P]>,
        coverage: super::Coverage,
    ) -> Result<PreparedImport<P>, HeapError> {
        symbols.check_import()?;
        let quantity = symbols.value().descriptor();
        check_quantity(quantity)?;
        let [rows, columns] = symbols.value().shape();
        let extent = Extent::new(
            usize::try_from(rows).map_err(|_| HeapError::ExtentOverflow)?,
            usize::try_from(columns).map_err(|_| HeapError::ExtentOverflow)?,
        )?;
        scan_symbols(symbols, extent, centers, bounds, errors, coverage)?;
        let mut schemas = self.prepare_schemas(extent, quantity, bounds.is_some())?;
        schemas.evaluation = errors.and(schemas.bound);
        let count = 1 + usize::from(bounds.is_some()) + usize::from(errors.is_some());
        let descriptors = self.descriptors.prepare(count, self.limits.descriptors)?;
        let payloads = [centers, bounds.unwrap_or(&[]), errors.unwrap_or(&[])];
        let inputs = payloads.get(..count).ok_or(HeapError::LengthMismatch)?;
        let storage = self.storage.prepare(inputs, &[], self.limits)?;
        let stamp = ValueStamp {
            id: ValueId(identity()?),
            version: ValueVersion(1),
        };
        let mut prepared = assemble(self.scope, extent, schemas, stamp, descriptors, storage)?;
        prepared.owners = crate::binding::symbols::prepare(self, symbols, prepared.bundle, stamp)?;
        if let Some(record) = prepared.records.values.first_mut()
            && let Meaning::Value {
                coverage: actual, ..
            } = &mut record.meaning
        {
            *actual = coverage;
        }
        Ok(prepared)
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
        let symbols = crate::binding::symbols::declare(
            &snapshot.schema.quantity,
            extent,
            [bounds.is_some(), false],
            &mut crate::binding::symbols::budget()?,
        )?;
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
        let mut prepared = assemble(self.scope, extent, schema_plan, stamp, descriptors, storage)?;
        prepared.owners = crate::binding::symbols::prepare(self, &symbols, prepared.bundle, stamp)?;
        Ok(self.publish_bundle(prepared))
    }

    /// The only visible bundle commit: no checks, allocations or caller code remain.
    fn publish_bundle<P: Scalar>(&mut self, prepared: PreparedImport<P>) -> PublishedBundle<P> {
        self.storage.publish(prepared.storage);
        self.schemas.extend(prepared.schemas);
        self.descriptors.publish(prepared.records);
        self.symbols.publish(prepared.owners);
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
            evaluation: None,
            contributions: None,
        })
    }
}

/// Check complete declared presence and overlapping numerical error before allocation.
fn scan_symbols<P: Scalar>(
    symbols: &nrz_bindings::BindingSymbols,
    extent: Extent,
    centers: &[P],
    bounds: Option<&[P]>,
    errors: Option<&[P]>,
    coverage: super::Coverage,
) -> Result<(), HeapError> {
    let bound_matches = symbols.bound().is_some() == bounds.is_some();
    let error_matches = symbols.error().is_some() == errors.is_some();
    if !bound_matches || !error_matches {
        return Err(nrz_bindings::AdmissionError::CompanionMismatch.into());
    }
    if coverage == super::Coverage::DeclaredBounds && bounds.is_none() {
        return Err(HeapError::MissingCoverage);
    }
    scan(extent, centers, bounds)?;
    scan(extent, centers, errors)?;
    if let (Some(total), Some(error)) = (bounds, errors)
        && error.iter().zip(total).any(|(error, total)| error > total)
    {
        return Err(nrz_bindings::AdmissionError::CompanionMismatch.into());
    }
    Ok(())
}

/// Build the selected import binding while all regions are still private.
fn imported_binding<P: Scalar>(
    scope: u64,
    prepared: &PreparedImport<P>,
    symbols: &nrz_bindings::BindingSymbols,
) -> Result<crate::ValueBinding<P>, HeapError> {
    let record = prepared
        .records
        .values
        .first()
        .ok_or(HeapError::LengthMismatch)?;
    let Meaning::Value { stamp, .. } = record.meaning else {
        return Err(HeapError::CategoryMismatch);
    };
    let schemas = [0, 1, 2].map(|position| {
        prepared
            .records
            .values
            .get(position)
            .map(|record| super::SchemaId {
                scope,
                index: record.schema,
            })
    });
    crate::binding::lifecycle::planned(
        scope,
        prepared.bundle,
        stamp,
        record.layout.extent(),
        schemas,
        symbols,
    )
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
        owners: Vec::new(),
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
            evaluation: None,
            contributions: None,
            coverage: if bound_key.is_some() {
                super::Coverage::DeclaredBounds
            } else {
                super::Coverage::RepresentedInputs
            },
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
    let evaluation = prepare_evaluation::<P>(
        scope,
        extent,
        keys,
        arenas,
        schemas,
        stamp,
        epoch,
        &mut records,
    )?;
    let bundle = crate::kernels::finish_contributions::<P>(
        &crate::kernels::ContributionReservation {
            scope,
            extent,
            keys,
            arenas,
            schema: schemas.contributions,
            stamp,
            epoch,
        },
        bound_key,
        evaluation,
        &mut records,
    )?;
    Ok((records, bundle))
}

/// Allocate the distinct evaluation region with the same exact converted-value target.
#[expect(
    clippy::too_many_arguments,
    reason = "the helper forwards an existing checked storage reservation"
)]
fn prepare_evaluation<P: Scalar + 'static>(
    scope: u64,
    extent: Extent,
    keys: &[Key],
    arenas: &[Key],
    schemas: &SchemaPlan,
    stamp: ValueStamp,
    epoch: u64,
    records: &mut Vec<Record>,
) -> Result<Option<Handle<super::Companion, P>>, HeapError> {
    let Some(schema) = schemas.evaluation else {
        return Ok(None);
    };
    let key = *keys.get(2).ok_or(HeapError::Capacity)?;
    let arena = *arenas.get(2).ok_or(HeapError::Capacity)?;
    let target = *keys.first().ok_or(HeapError::Capacity)?;
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
    Ok(Some(Handle::new(scope, key)))
}
