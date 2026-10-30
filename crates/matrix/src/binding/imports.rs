// SPDX-License-Identifier: AGPL-3.0-only
//! Import preparation and the allocation-free, callback-free publication step.
//!
//! # Import preparation and complete publication
//!
//! `Heap::import_represented` admits finite coherent coordinates with explicitly represented-only coverage. `Heap::import_declared_bounds` atomically admits finite centers and nonnegative finite componentwise radius entries, deriving the radius descriptor through bindings. These supplied bounds remain caller assumptions. Imports use logical column order, canonicalize signed zero, and retain point/difference roles. Floating count storage and noncoherent coordinate imports refuse. Unsupported nested relative-radius derivation preserves the bindings error.
//!
//! `HeapLimits` bounds descriptor slots, elements per precision (including companions), and immutable schemas. Preparation scans and validates the complete bundle, reserves metadata capacity and builds every slab before an allocation-free publication step. Replacement scratch is bounded by the same element limit as the new live bundle; old and prepared storage may coexist, so peak heap-owned addressable elements can reach twice that limit, in addition to backend padding and metadata. A refusal publishes no partial value or companion. Rust allocator aborts remain outside the recoverable-capacity contract. Process-local identities never wrap and make no persistence promise.

mod coordinates;

use nrz_bindings::{Descriptor, UncertaintyMode, uncertainty};
use nrz_scalar::Scalar;

use super::schema::{BindingSchema, check_quantity, intern};
use crate::heap::identity::identity;
use crate::heap::publication::{SchemaPlan, prepare_records, scan};
use crate::heap::records::{Meaning, Record};
use crate::heap::slots::{PreparedSlots, SlotPlan};
use crate::storage::PreparedStorage;
use crate::{
    Extent, Handle, Heap, HeapError, PublishedBundle, Value, ValueId, ValueStamp, ValueVersion,
};

/// Fully prepared import, inaccessible until its complete bundle is linked.
#[derive(Debug)]
struct PreparedImport<P: Scalar> {
    /// Private backend allocations.
    storage: PreparedStorage,
    /// New immutable interpretations.
    schemas: Vec<BindingSchema>,
    /// Complete value and optional companion descriptors.
    records: PreparedSlots<Record>,
    /// Prepared exact symbol covers, installed with their numerical regions.
    covers: Vec<crate::binding::symbols::RegionCover>,
    /// Future locators, returned only after publication.
    bundle: PublishedBundle<P>,
    /// Prepared structural invalidation closure for a coarse replacement.
    invalidations: super::live::invalidation::Invalidations,
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
        coverage: crate::Coverage,
    ) -> Result<crate::ValueBinding<P>, HeapError> {
        check_quantity(symbols.value().descriptor())?;
        self.import_quantity_symbols(symbols, centers, bounds, errors, coverage)
    }

    /// Checked quantity admission may retain an explicit noncoherent coordinate measure.
    pub(crate) fn import_quantity_symbols<P: Scalar + 'static>(
        &mut self,
        symbols: &nrz_bindings::BindingSymbols,
        centers: &[P],
        bounds: Option<&[P]>,
        errors: Option<&[P]>,
        coverage: crate::Coverage,
    ) -> Result<crate::ValueBinding<P>, HeapError> {
        symbols.check_import()?;
        let prepared = self.prepare_symbol_bundle(symbols, centers, bounds, errors, coverage)?;
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
            crate::Coverage::DeclaredBounds
        } else {
            crate::Coverage::RepresentedInputs
        };
        let prepared = self.prepare_symbol_bundle(&symbols, centers, bounds, None, coverage)?;
        Ok(self.publish_bundle(prepared))
    }

    /// Finish every fallible symbol, payload and allocation check before publication.
    fn prepare_symbol_bundle<P: Scalar + 'static>(
        &mut self,
        symbols: &nrz_bindings::BindingSymbols,
        centers: &[P],
        bounds: Option<&[P]>,
        errors: Option<&[P]>,
        coverage: crate::Coverage,
    ) -> Result<PreparedImport<P>, HeapError> {
        let quantity = symbols.value().descriptor();
        if quantity.kind().count_domain().is_some() {
            return Err(HeapError::CountStorage);
        }
        let [rows, columns] = symbols.value().shape();
        let extent = Extent::new(
            usize::try_from(rows).map_err(|_| HeapError::ExtentOverflow)?,
            usize::try_from(columns).map_err(|_| HeapError::ExtentOverflow)?,
        )?;
        scan_symbols(symbols, extent, centers, bounds, errors, coverage)?;
        let (mut schemas, meanings) = self.prepare_schemas(extent, quantity, bounds.is_some())?;
        schemas.evaluation = errors.and(schemas.bound);
        let count = 1 + usize::from(bounds.is_some()) + usize::from(errors.is_some());
        let descriptors = self.descriptors.prepare(count, self.limits.descriptors)?;
        let payloads = [centers, bounds.unwrap_or(&[]), errors.unwrap_or(&[])];
        let inputs = payloads.get(..count).ok_or(HeapError::LengthMismatch)?;
        let stamp = ValueStamp {
            id: ValueId(identity()?),
            version: ValueVersion(1),
        };
        let bundle = super::symbols::planned_bundle::<P>(self.scope, descriptors.keys(), &schemas)?;
        let covers = super::symbols::prepare(self, symbols, bundle, stamp)?;
        let storage = self.storage.prepare(inputs, &[], self.limits)?;
        let mut prepared = assemble(
            self.scope,
            extent,
            &schemas,
            meanings,
            stamp,
            descriptors,
            storage,
        )?;
        prepared.covers = covers;
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
        let quantity = self.binding_quantity(handle)?.clone();
        let symbols = crate::binding::symbols::declare(
            &quantity,
            extent,
            [bounds.is_some(), false],
            &mut crate::binding::symbols::budget()?,
        )?;
        scan(extent, centers, bounds)?;
        let (schema_plan, meanings) = self.prepare_schemas(extent, &quantity, bounds.is_some())?;
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
        let stamp = ValueStamp {
            id: snapshot.stamp.id,
            version: ValueVersion(next),
        };
        let bundle =
            super::symbols::planned_bundle::<P>(self.scope, descriptors.keys(), &schema_plan)?;
        let covers = super::symbols::prepare(self, &symbols, bundle, stamp)?;
        let storage = self
            .storage
            .prepare(inputs, &snapshot.regions()?, self.limits)?;
        let mut prepared = assemble(
            self.scope,
            extent,
            &schema_plan,
            meanings,
            stamp,
            descriptors,
            storage,
        )?;
        prepared.covers = covers;
        prepared.invalidations = super::live::invalidation::whole(self, handle.key);
        Ok(self.publish_bundle(prepared))
    }

    /// The only visible bundle commit: no checks, allocations or caller code remain.
    fn publish_bundle<P: Scalar>(&mut self, prepared: PreparedImport<P>) -> PublishedBundle<P> {
        self.storage.publish(prepared.storage);
        super::schema::publish(self, prepared.schemas);
        self.descriptors.publish(prepared.records);
        self.symbols.publish(prepared.covers);
        super::live::invalidation::publish(prepared.invalidations);
        prepared.bundle
    }

    /// Derive and intern immutable metadata without changing the visible schema pool.
    fn prepare_schemas(
        &mut self,
        extent: Extent,
        quantity: &Descriptor,
        bounded: bool,
    ) -> Result<(SchemaPlan, Vec<BindingSchema>), HeapError> {
        let mut pending = Vec::new();
        let value = intern(
            &self.meanings,
            &mut pending,
            BindingSchema {
                extent,
                quantity: Some(quantity.clone()),
            },
            self.limits.schemas,
        )?;
        let bound = if bounded {
            let radius = uncertainty(UncertaintyMode::Radius, quantity, None, None)?;
            Some(intern(
                &self.meanings,
                &mut pending,
                BindingSchema {
                    extent,
                    quantity: Some(radius),
                },
                self.limits.schemas,
            )?)
        } else {
            None
        };
        super::schema::reserve(self, pending.len())?;
        Ok((
            SchemaPlan {
                value,
                bound,
                evaluation: None,
                contributions: None,
            },
            pending,
        ))
    }
}

/// Check complete declared presence and overlapping numerical error before allocation.
fn scan_symbols<P: Scalar>(
    symbols: &nrz_bindings::BindingSymbols,
    extent: Extent,
    centers: &[P],
    bounds: Option<&[P]>,
    errors: Option<&[P]>,
    coverage: crate::Coverage,
) -> Result<(), HeapError> {
    let bound_matches = symbols.bound().is_some() == bounds.is_some();
    let error_matches = symbols.error().is_some() == errors.is_some();
    if !bound_matches || !error_matches {
        return Err(nrz_bindings::AdmissionError::CompanionMismatch.into());
    }
    if coverage == crate::Coverage::DeclaredBounds && bounds.is_none() {
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
            .map(|record| crate::SchemaId {
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
    schemas: &SchemaPlan,
    meanings: Vec<BindingSchema>,
    stamp: ValueStamp,
    descriptors: SlotPlan,
    storage: PreparedStorage,
) -> Result<PreparedImport<P>, HeapError> {
    let (records, bundle) = prepare_records::<P>(
        scope,
        extent,
        descriptors.keys(),
        storage.keys(),
        schemas,
        stamp,
        storage.epoch(),
    )?;
    Ok(PreparedImport {
        covers: Vec::new(),
        invalidations: Vec::new(),
        storage,
        schemas: meanings,
        records: descriptors.bind(records)?,
        bundle,
    })
}
