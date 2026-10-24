// SPDX-License-Identifier: AGPL-3.0-only
//! Prepare numerical writes and their complete invalidation closure before an atomic commit.

use super::{LiveBindingError, ReadingBinding, admission::charge};
use crate::heap::{
    identity::Key,
    records::{Location, Meaning},
};
use crate::storage::{Format, Region};
use crate::{Heap, HeapError, ValueVersion};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;

/// One private whole-region replacement and its injective selected write set.
struct Replacement<P: Scalar> {
    /// Surviving numerical occupation.
    key: Key,
    /// Complete previous allocation retired at commit.
    old: Region,
    /// Fully validated private replacement payload.
    values: Vec<P>,
    /// Injective exact entry write set.
    indices: Vec<usize>,
    /// Checked successor base revision for ordinary numerical evidence.
    version: ValueVersion,
}

impl Heap {
    /// Replace selected stored coordinates and invalidate every affected reading and proof.
    ///
    /// This writes the representation's ordered storage entries, not the physical
    /// map's output coordinates. A None representation has no writable entries.
    /// # Errors
    /// Refuses stale bindings, incomplete/nonfinite payloads, exhausted entry versions,
    /// failed dependency queries and resource limits without changing values or readers.
    pub fn replace_reading_entries<P: Scalar + 'static>(
        &mut self,
        binding: &ReadingBinding<P>,
        values: &[P],
        budget: &mut Budget,
    ) -> Result<(), LiveBindingError> {
        self.validate_reading(binding)?;
        if values.len() != binding.adapter().entries().len() || values.is_empty() {
            return Err(HeapError::LengthMismatch.into());
        }
        if binding.reading().family().node() != binding.reading().family().root() {
            if binding.adapter().representation().kind()
                != nrz_bindings::readings::RepresentationKind::ComplexBlock
            {
                return Err(HeapError::MissingCoverage.into());
            }
            super::admission::block(values)?;
        }
        replace(
            self,
            &binding.group,
            binding.adapter().entries(),
            values,
            budget,
        )
    }
}

/// The same checked transaction updates companion parameters without inventing a value reading.
pub(super) fn replace<P: Scalar + 'static>(
    heap: &mut Heap,
    group: &super::Group,
    entries: &[crate::EntryTicket<usize>],
    values: &[P],
    budget: &mut Budget,
) -> Result<(), LiveBindingError> {
    charge::<P>(values.len(), budget)?;
    let replacements = prepare(heap, group, entries, values, budget)?;
    let writes = replacements
        .iter()
        .map(|item| (item.key, item.indices.clone()))
        .collect::<Vec<_>>();
    let invalidated = super::invalidation::prepare(heap, &writes, budget)?;
    let payloads = replacements
        .iter()
        .map(|item| item.values.as_slice())
        .collect::<Vec<_>>();
    let retired = replacements.iter().map(|item| item.old).collect::<Vec<_>>();
    let storage = heap.storage.prepare(&payloads, &retired, heap.limits)?;
    let locations = locations::<P>(&replacements, storage.keys(), storage.epoch())?;
    heap.storage.publish(storage);
    advance_entries(heap, &replacements);
    commit(heap, &replacements, &locations);
    super::invalidation::publish(invalidated);
    Ok(())
}

/// Copy only affected bases; reject every invalid element before any publication.
fn prepare<P: Scalar + 'static>(
    heap: &Heap,
    group: &super::Group,
    entries: &[crate::EntryTicket<usize>],
    values: &[P],
    budget: &mut Budget,
) -> Result<Vec<Replacement<P>>, LiveBindingError> {
    let mut replacements: Vec<Replacement<P>> = Vec::new();
    for (entry, &value) in entries.iter().zip(values) {
        let index = usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout)?;
        if !value.is_finite() {
            return Err(HeapError::NonFinite { index }.into());
        }
        let ticket = group
            .tickets
            .get(entry.region)
            .ok_or(HeapError::InvalidLayout)?;
        let position =
            if let Some(position) = replacements.iter().position(|item| item.key == ticket.key) {
                position
            } else {
                replacements.push(base::<P>(heap, ticket, budget)?);
                replacements.len() - 1
            };
        let replacement = replacements
            .get_mut(position)
            .ok_or(HeapError::InvalidLayout)?;
        if replacement.indices.contains(&index) {
            return Err(HeapError::OutputOverlap.into());
        }
        *replacement
            .values
            .get_mut(index)
            .ok_or(HeapError::InvalidLayout)? = value;
        replacement.indices.push(index);
        heap.symbols
            .cover(ticket.key)?
            .versions
            .get(index)
            .ok_or(HeapError::InvalidLayout)?
            .get()
            .checked_add(1)
            .ok_or(HeapError::CounterExhausted)?;
    }
    Ok(replacements)
}

/// Whole-base numeric revisions invalidate ordinary numerical evidence independently.
fn base<P: Scalar + 'static>(
    heap: &Heap,
    ticket: &super::RegionTicket,
    budget: &mut Budget,
) -> Result<Replacement<P>, LiveBindingError> {
    let handle = crate::Handle::<crate::Value, P>::new(ticket.scope, ticket.key);
    let view = heap.value(handle)?;
    charge::<P>(view.region().extent().entries(), budget)?;
    let record = heap.record(handle)?;
    if matches!(
        record.meaning,
        Meaning::Value { bound: Some(_), .. }
            | Meaning::Value {
                evaluation: Some(_),
                ..
            }
            | Meaning::Value {
                contributions: Some(_),
                ..
            }
    ) {
        return Err(HeapError::MissingCoverage.into());
    }
    let Location::Owned(old) = record.location else {
        return Err(HeapError::InvalidLayout.into());
    };
    let version = ValueVersion(
        view.stamp()
            .version()
            .0
            .checked_add(1)
            .ok_or(HeapError::CounterExhausted)?,
    );
    Ok(Replacement {
        key: ticket.key,
        old,
        values: view.region().export()?,
        indices: Vec::new(),
        version,
    })
}

/// Every selected entry version was checked for overflow in preparation.
fn advance_entries<P: Scalar>(heap: &Heap, replacements: &[Replacement<P>]) {
    for item in replacements {
        if let Ok(cover) = heap.symbols.cover(item.key) {
            for &index in &item.indices {
                if let Some(version) = cover.versions.get(index) {
                    version.set(version.get() + 1);
                }
            }
        }
    }
}

/// Match all allocated region keys before entering the allocation-free commit.
fn locations<P: Scalar + 'static>(
    replacements: &[Replacement<P>],
    arenas: &[Key],
    epoch: u64,
) -> Result<Vec<Region>, HeapError> {
    if replacements.len() != arenas.len() {
        return Err(HeapError::LengthMismatch);
    }
    Ok(replacements
        .iter()
        .zip(arenas)
        .map(|(item, &arena)| Region {
            arena,
            epoch,
            format: Format::of::<P>(),
            offset: 0,
            length: item.values.len(),
        })
        .collect())
}

/// All mutable records were resolved under the same uninterrupted exclusive heap lease.
fn commit<P: Scalar>(heap: &mut Heap, replacements: &[Replacement<P>], locations: &[Region]) {
    for (key, record) in heap.descriptors.iter_mut() {
        if let Some(index) = replacements.iter().position(|item| item.key == key) {
            record.location = Location::Owned(locations[index]);
            if let Meaning::Value { stamp, .. } = &mut record.meaning {
                stamp.version = replacements[index].version;
            }
        }
    }
}
