// SPDX-License-Identifier: AGPL-3.0-only
//! Selected general readings enter homogeneous recipes without copying or losing their dependencies.

use super::{Group, LiveBindingError, ReadingBinding};
use crate::heap::records::{Location, Meaning, Record};
use crate::{Heap, HeapError, Layout, SymbolState};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Precision-erased selected reading retained in aliases, snapshots and provenance.
#[derive(Clone, Debug)]
pub(crate) struct SelectedReading {
    /// Exact immutable coverage and all retained proof dependencies.
    pub(crate) group: Rc<Group>,
    /// Selected reading within the sealed declaration inventory.
    pub(crate) index: usize,
}

impl SelectedReading {
    /// Exact homogeneous interpretation, available only for an identity-coordinate reading.
    pub(crate) fn symbols(&self) -> Option<&nrz_bindings::BindingSymbols> {
        self.group
            .homogeneous
            .get(self.index)
            .and_then(Option::as_ref)
    }
    /// Revalidate scope, occupation, schema, layout, precision and each covered entry version.
    pub(crate) fn validate(&self, heap: &Heap) -> Result<(), HeapError> {
        if self.group.scope != heap.scope {
            return Err(HeapError::ForeignHandle);
        }
        if self.group.states.get(self.index).map(std::cell::Cell::get)
            != Some(SymbolState::Published)
        {
            return Err(HeapError::UnavailableSymbol);
        }
        let adapter = self
            .group
            .coverage
            .readings()
            .get(self.index)
            .ok_or(HeapError::MissingBinding)?;
        self.validate_entries(heap, adapter.entries())?;
        for companion in &self.group.companions {
            if companion
                .targets()
                .iter()
                .any(|target| target.reading() == adapter.reading())
            {
                self.validate_entries(heap, companion.entries())?;
                for target in companion.targets() {
                    let index = self
                        .group
                        .coverage
                        .readings()
                        .iter()
                        .position(|actual| actual.reading() == target.reading())
                        .ok_or(HeapError::MissingBinding)?;
                    if self.group.states[index].get() != SymbolState::Published {
                        return Err(HeapError::UnavailableSymbol);
                    }
                    self.validate_entries(heap, target.entries())?;
                }
            }
        }
        super::companions::validate_native(&self.group, heap)?;
        Ok(())
    }

    /// Recheck actual covered entries, independently of physical interpretation.
    fn validate_entries(
        &self,
        heap: &Heap,
        entries: &[crate::EntryTicket<usize>],
    ) -> Result<(), HeapError> {
        for entry in entries {
            let ticket = self
                .group
                .tickets
                .get(entry.region)
                .ok_or(HeapError::InvalidLayout)?;
            let record = heap.descriptors.get(ticket.key)?;
            let cover = heap.symbols.cover(ticket.key)?;
            let index = usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout)?;
            let valid_stamp = matches!(record.meaning, Meaning::Value { stamp, .. } if stamp.id() == ticket.identity);
            if !valid_stamp
                || cover.stamp.id() != ticket.identity
                || cover.stamp.version() != ticket.generation
                || record.schema != ticket.schema
                || record.layout != ticket.layout
                || cover.versions.get(index).map(std::cell::Cell::get)
                    != ticket.versions.get(index).copied()
            {
                return Err(HeapError::StaleEvidence);
            }
            let region = heap.owning_region(record)?;
            let precision = match region.format {
                crate::storage::Format::Binary32 => nrz_scalar::Precision::F32,
                crate::storage::Format::Binary64 => nrz_scalar::Precision::F64,
            };
            if precision != ticket.precision || precision != self.group.precision {
                return Err(HeapError::PrecisionMismatch);
            }
            heap.storage.resolve(region)?;
        }
        Ok(())
    }
}

impl<P: Scalar> ReadingBinding<P> {
    /// Capture this exact selected interpretation without granting numerical access.
    pub(crate) fn selected(&self) -> SelectedReading {
        SelectedReading {
            group: Rc::clone(&self.group),
            index: self.index,
        }
    }
}

impl Heap {
    /// Select an identity-coordinate homogeneous reading for existing numerical recipes.
    ///
    /// The alias shares its original float entries and exact reading identity. Its
    /// snapshots retain entry versions and all structural proof dependencies.
    /// # Errors
    /// Refuses None, nonidentity or mixed maps, multiple base regions, stale bindings
    /// and descriptor capacity failures without publishing a partial selection.
    pub fn select_reading<P: Scalar + 'static>(
        &mut self,
        binding: &ReadingBinding<P>,
    ) -> Result<crate::ValueBinding<P>, LiveBindingError> {
        self.validate_reading(binding)?;
        if !binding.group.native.is_empty() || !binding.group.companions.is_empty() {
            return Err(HeapError::MissingCoverage.into());
        }
        let selected = binding.selected();
        let symbols = selected.symbols().ok_or(HeapError::MissingBinding)?;
        let first = binding
            .adapter()
            .entries()
            .first()
            .ok_or(HeapError::MissingCoverage)?;
        if binding
            .adapter()
            .entries()
            .iter()
            .any(|entry| entry.region != first.region)
        {
            return Err(HeapError::InvalidLayout.into());
        }
        let ticket = binding
            .group
            .tickets
            .get(first.region)
            .ok_or(HeapError::InvalidLayout)?;
        let indices = binding
            .adapter()
            .entries()
            .iter()
            .map(|entry| usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout))
            .collect::<Result<Vec<_>, _>>()?;
        let [rows, columns] = symbols.value().shape();
        let extent = crate::Extent::new(
            usize::try_from(rows).map_err(|_| HeapError::ExtentOverflow)?,
            usize::try_from(columns).map_err(|_| HeapError::ExtentOverflow)?,
        )?;
        let layout = Layout::gather(
            extent,
            ticket.layout.extent().entries(),
            &indices,
            self.limits.layout_work,
        )?;
        let plan = self.descriptors.prepare(1, self.limits.descriptors)?;
        let key = *plan.keys().first().ok_or(HeapError::Capacity)?;
        self.symbols
            .reserve_selection(key, self.limits.descriptors)?;
        let value =
            super::super::lifecycle::selected(self.scope, key, ticket, layout.clone(), symbols)?;
        let record = Record {
            schema: ticket.schema,
            location: Location::Alias,
            layout,
            meaning: Meaning::Alias { base: ticket.key },
        };
        let prepared = plan.bind(vec![record])?;
        self.descriptors.publish(prepared);
        self.symbols.publish_selection(key, selected);
        Ok(value)
    }
}
