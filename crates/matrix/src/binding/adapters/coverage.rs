// SPDX-License-Identifier: AGPL-3.0-only
//! Total coverage requires a checked selected-coordinate equation for every intersecting pair.

use super::{AT, EntryTicket, ReadingAdapter, ReadingAdapterError, capacity, entry, reserve};
use nrz_bindings::{
    Budget,
    readings::{ProjectionSpace, ReadingProjection, ReadingRelation},
};

/// Read-only total value-entry coverage with every pairwise overlap proved on its exact family.
#[derive(Clone, Debug)]
pub struct ReadingCoverage<T> {
    /// Complete required numerical entries; no uncovered suffix can be omitted after checking.
    entries: Vec<EntryTicket<T>>,
    /// All actual selected reading adapters, including exact no-storage values.
    readings: Vec<ReadingAdapter<T>>,
    /// Exact selected-coordinate equations consumed once per intersecting adapter pair.
    overlaps: Vec<ReadingRelation>,
}

impl<T: Eq> ReadingCoverage<T> {
    /// Check complete coverage and every actual intersection without granting live access.
    /// # Errors
    /// Refuses missing coverage, entries outside the universe, absent/wrong-copy overlap proofs and duplicates.
    pub fn new(
        entries: Vec<EntryTicket<T>>,
        readings: Vec<ReadingAdapter<T>>,
        overlaps: Vec<ReadingRelation>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingAdapterError> {
        entry::check(&entries, entries.len(), budget)?;
        capacity::<ReadingAdapter<T>>(readings.len(), budget)?;
        capacity::<ReadingRelation>(overlaps.len(), budget)?;
        covered(&entries, &readings, budget)?;
        intersections(&readings, &overlaps, budget)?;
        Ok(Self {
            entries,
            readings,
            overlaps,
        })
    }
}

impl<T> ReadingCoverage<T> {
    /// Exact complete entry universe, to be matched against the owner's resolved region inventory.
    #[must_use]
    pub fn entries(&self) -> &[EntryTicket<T>] {
        &self.entries
    }
    /// Every admitted covering reading, with no unique-owner interpretation.
    #[must_use]
    pub fn readings(&self) -> &[ReadingAdapter<T>] {
        &self.readings
    }
    /// Exact projected equations for all intersecting pairs.
    #[must_use]
    pub fn overlaps(&self) -> &[ReadingRelation] {
        &self.overlaps
    }
}

/// Match both directions so neither uncovered entries nor foreign covered entries are hidden.
fn covered<T: Eq>(
    entries: &[EntryTicket<T>],
    readings: &[ReadingAdapter<T>],
    budget: &mut Budget,
) -> Result<(), ReadingAdapterError> {
    let mut found = reserve(entries.len(), budget)?;
    found.resize(entries.len(), false);
    for reading in readings {
        for selected in reading.entries() {
            budget.work(
                u64::try_from(entries.len()).map_err(|_| ReadingAdapterError::Extent)?,
                AT,
            )?;
            let position = entries
                .iter()
                .position(|entry| entry == selected)
                .ok_or(ReadingAdapterError::Coverage)?;
            *found
                .get_mut(position)
                .ok_or(ReadingAdapterError::Coverage)? = true;
        }
    }
    if found.contains(&false) {
        return Err(ReadingAdapterError::Coverage);
    }
    Ok(())
}

/// Each pair is independently checked; one compatible view cannot conceal another contradiction.
fn intersections<T: Eq>(
    readings: &[ReadingAdapter<T>],
    relations: &[ReadingRelation],
    budget: &mut Budget,
) -> Result<(), ReadingAdapterError> {
    let mut used = reserve(relations.len(), budget)?;
    used.resize(relations.len(), false);
    for (position, left) in readings.iter().enumerate() {
        for right in &readings[position + 1..] {
            let (first, second) = overlap(left, right, budget)?;
            if first.is_empty() {
                continue;
            }
            let mut found = false;
            for (index, relation) in relations.iter().enumerate() {
                budget.work(
                    u64::try_from(
                        first
                            .len()
                            .checked_add(second.len())
                            .ok_or(ReadingAdapterError::Extent)?,
                    )
                    .map_err(|_| ReadingAdapterError::Extent)?,
                    AT,
                )?;
                for reading in [
                    left.reading(),
                    right.reading(),
                    relation.left().reading(),
                    relation.right().reading(),
                ] {
                    super::reading_work(reading, budget)?;
                }
                if used[index] {
                    continue;
                }
                let forward = matches(relation.left(), left, &first)
                    && matches(relation.right(), right, &second);
                let reverse = matches(relation.right(), left, &first)
                    && matches(relation.left(), right, &second);
                if forward || reverse {
                    used[index] = true;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(ReadingAdapterError::Overlap);
            }
        }
    }
    if used.contains(&false) {
        return Err(ReadingAdapterError::Overlap);
    }
    Ok(())
}

/// Intersections retain left selection order and the corresponding exact right positions.
fn overlap<T: Eq>(
    left: &ReadingAdapter<T>,
    right: &ReadingAdapter<T>,
    budget: &mut Budget,
) -> Result<(Vec<u32>, Vec<u32>), ReadingAdapterError> {
    let count = left.entries().len().min(right.entries().len());
    let mut first = reserve(count, budget)?;
    let mut second = reserve(count, budget)?;
    for (position, entry) in left.entries().iter().enumerate() {
        budget.work(
            u64::try_from(right.entries().len()).map_err(|_| ReadingAdapterError::Extent)?,
            AT,
        )?;
        if let Some(other) = right
            .entries()
            .iter()
            .position(|candidate| candidate == entry)
        {
            first.push(u32::try_from(position).map_err(|_| ReadingAdapterError::Extent)?);
            second.push(u32::try_from(other).map_err(|_| ReadingAdapterError::Extent)?);
        }
    }
    Ok((first, second))
}

/// Evidence must name selected storage coordinates and the complete original physical reading.
fn matches<T>(
    projection: &ReadingProjection,
    reading: &ReadingAdapter<T>,
    coordinates: &[u32],
) -> bool {
    projection.space() == ProjectionSpace::Selected
        && projection.reading() == reading.reading()
        && projection.coordinates() == coordinates
}
