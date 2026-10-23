// SPDX-License-Identifier: AGPL-3.0-only
//! Multiple result readings are declared beside the recipe before any evaluation.

use super::{DependencyPolicy, LiveBindingError, ReadingBinding, RegionTicket, admission::charge};
use crate::{Bound, EntryTicket, HeapError, ReadingCoverage, SymbolState};
use nrz_bindings::{Budget, readings::algebra::Cas};
use nrz_scalar::Scalar;
use std::rc::Rc;

impl<P: Scalar + 'static, Q: Scalar + 'static> Bound<'_, P, Q> {
    /// Declare a complete result family and all component readings before numerical execution.
    ///
    /// Region numbers select reserved result values. This is an initial declaration
    /// of private recipe results; it cannot attach an unrelated symbol to a live input.
    /// The numerical bundle is written once, regardless of the number of readers.
    /// # Errors
    /// Refuses incomplete coverage, changed families/maps, wrong physical meanings,
    /// post-evaluation declarations and all exact resource or dependency failures.
    pub fn predeclare_result_readings(
        &mut self,
        graph: Rc<Cas>,
        coverage: ReadingCoverage<usize>,
        policy: DependencyPolicy,
        budget: &mut Budget,
    ) -> Result<Vec<ReadingBinding<Q>>, LiveBindingError> {
        if self.issued.get().is_some() || !self.reservations.planned.readings.is_empty() {
            return Err(HeapError::UnavailableSymbol.into());
        }
        check_results(self, &coverage, budget)?;
        let tickets = result_tickets(self)?;
        let mut group = super::dependencies::prepare(
            graph,
            coverage,
            Vec::new(),
            tickets,
            policy,
            (self.heap.scope, Q::PRECISION),
            budget,
        )?;
        Rc::get_mut(&mut group).ok_or(HeapError::Capacity)?.native = native_companions(self);
        let import = super::bindings::<Q>(&group);
        for bundle in &self.reservations.bundles {
            let cover = self
                .reservations
                .covers
                .iter_mut()
                .find(|cover| cover.key == bundle.value().key)
                .ok_or(HeapError::MissingBinding)?;
            cover
                .readers
                .try_reserve(1)
                .map_err(|_| HeapError::Capacity)?;
        }
        for bundle in &self.reservations.bundles {
            if let Some(cover) = self
                .reservations
                .covers
                .iter_mut()
                .find(|cover| cover.key == bundle.value().key)
            {
                cover.readers.push(Rc::clone(&group));
            }
        }
        self.reservations
            .planned
            .readings
            .clone_from(&import.bindings);
        Ok(import.bindings)
    }

    /// Every additional pending result reading, including all shared-entry components.
    #[must_use]
    pub fn result_readings(&self) -> &[ReadingBinding<Q>] {
        &self.reservations.planned.readings
    }

    /// Read a live selected interpretation through this uninterrupted execution lease.
    /// # Errors
    /// Refuses pending outputs and every unavailable, foreign or stale reading.
    pub fn reading_entries<R: Scalar + 'static>(
        &self,
        binding: &ReadingBinding<R>,
    ) -> Result<Vec<R>, LiveBindingError> {
        self.heap.reading_entries(binding)
    }
}

/// Every result float must occur in the proposed universe, including otherwise unread suffixes.
fn check_results<P: Scalar, Q: Scalar>(
    bound: &Bound<'_, P, Q>,
    coverage: &ReadingCoverage<usize>,
    budget: &mut Budget,
) -> Result<(), LiveBindingError> {
    let mut expected = Vec::new();
    for (region, output) in bound.reservations.outputs.iter().enumerate() {
        charge::<EntryTicket<usize>>(output.extent().entries(), budget)?;
        for index in 0..output.extent().entries() {
            expected.push(EntryTicket {
                region,
                index: u64::try_from(index).map_err(|_| HeapError::ExtentOverflow)?,
            });
        }
    }
    if expected.len() != coverage.entries().len() {
        return Err(crate::ReadingAdapterError::Coverage.into());
    }
    for entry in expected {
        charge::<EntryTicket<usize>>(coverage.entries().len(), budget)?;
        if !coverage.entries().contains(&entry) {
            return Err(crate::ReadingAdapterError::Coverage.into());
        }
    }
    let mut families = Vec::new();
    for adapter in coverage.readings() {
        let family = adapter.reading().family();
        for (entry, selection) in adapter
            .entries()
            .iter()
            .zip(adapter.reading().selection().indices())
        {
            let output = bound
                .operation
                .outputs()
                .get(entry.region)
                .ok_or(HeapError::InvalidLayout)?;
            let extent = output.shape().extent();
            let logical = u64::from(selection.column) * u64::from(family.shape()[0])
                + u64::from(selection.row);
            let physical = family
                .entries()
                .iter()
                .all(|meaning| meaning.descriptor() == output.descriptor());
            if family.node() != family.root()
                || family.shape() != crate::binding::symbols::shape(extent)?
                || logical != entry.index
                || !physical
            {
                return Err(crate::ReadingAdapterError::Target.into());
            }
            if families
                .iter()
                .any(|(root, region)| root == family.root() && *region != entry.region)
            {
                return Err(crate::ReadingAdapterError::Target.into());
            }
            families.push((family.root().clone(), entry.region));
        }
    }
    Ok(())
}

/// Pending readers capture the actual reserved occupations and independent per-entry versions.
fn result_tickets<P: Scalar, Q: Scalar>(
    bound: &Bound<'_, P, Q>,
) -> Result<Vec<RegionTicket>, HeapError> {
    bound
        .reservations
        .bundles
        .iter()
        .zip(&bound.reservations.outputs)
        .map(|(bundle, output)| {
            Ok(RegionTicket {
                scope: bound.heap.scope,
                key: bundle.value().key,
                precision: Q::PRECISION,
                layout: crate::Layout::contiguous(output.extent())?,
                identity: output.stamp().id(),
                generation: output.stamp().version(),
                schema: output.schema().index,
                versions: vec![1; output.extent().entries()],
            })
        })
        .collect()
}

/// Every additional reading remains pending until the complete ordinary bundle commits.
pub(crate) fn validate_pending<P: Scalar>(readings: &[ReadingBinding<P>]) -> Result<(), HeapError> {
    for reading in readings {
        if reading.state() != SymbolState::Pending {
            return Err(HeapError::UnavailableSymbol);
        }
    }
    Ok(())
}

/// Preserve every native bound, error and contribution target beside the selected result family.
fn native_companions<P: Scalar, Q: Scalar>(
    bound: &Bound<'_, P, Q>,
) -> Vec<super::companions::NativeCompanion> {
    let mut result = Vec::new();
    for (key, record) in bound
        .reservations
        .records
        .keys()
        .iter()
        .zip(&bound.reservations.records.values)
    {
        if let crate::heap::records::Meaning::Bound { target, stamp } = record.meaning {
            result.push(super::companions::NativeCompanion {
                key: *key,
                target,
                stamp,
                schema: record.schema,
                layout: record.layout.clone(),
            });
        }
    }
    result
}
