// SPDX-License-Identifier: AGPL-3.0-only
//! Reserve and scatter complete native bundles, then publish without fallible work.

use super::{AT, count, plan::Bundle};
use crate::binding::{
    live::{LiveBindingError, ReadingBinding, invalidation::Invalidations},
    symbols::RegionCover,
};
use crate::heap::{
    identity::Key,
    records::{Location, Meaning},
};
use crate::storage::{Format, PreparedStorage, Region};
use crate::{Heap, HeapError};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetKind};
use nrz_scalar::Scalar;

/// Prepared occupation, numerical location and exact successor target revision.
type PublishedLocation = (Key, Region, crate::ValueStamp);

/// Every location, cover and successor target has been checked before this exists.
pub(super) struct Prepared {
    /// Private replacement allocations and the complete old allocation retirement set.
    storage: PreparedStorage,
    /// New numerical locations in value/total/error order per bundle.
    locations: Vec<PublishedLocation>,
    /// Replacement entry revisions and matching native sponsor target revisions.
    covers: Vec<RegionCover>,
}

/// Bound all retained arrays and metadata before exporting any native payload.
pub(super) fn reservation<P: Scalar>(
    heap: &Heap,
    schedule: &[Bundle],
    budget: &mut Budget,
) -> Result<u64, LiveBindingError> {
    let mut bytes = 0_u64;
    let mut work = 0_u64;
    for item in schedule {
        bytes = add(bytes, item.indices.len(), 3 * std::mem::size_of::<usize>())?;
        bytes = add(bytes, 1, std::mem::size_of::<Bundle>())?;
        for key in item.keys {
            let cover = heap.symbols.cover(key)?;
            let length = heap.descriptors.get(key)?.layout.extent().entries();
            let width = 2 * std::mem::size_of::<P>() + 2 * std::mem::size_of::<usize>();
            bytes = add(bytes, length, width)?;
            bytes = add(
                bytes,
                cover.readers.len(),
                std::mem::size_of::<std::rc::Rc<crate::binding::live::Group>>(),
            )?;
            bytes = add(bytes, 1, std::mem::size_of::<RegionCover>() + 256)?;
            work = work
                .checked_add(count(length)?)
                .ok_or(HeapError::ExtentOverflow)?;
        }
    }
    // Storage and descriptor tables are scanned during preparation and commit.
    work = work
        .checked_add(count(heap.descriptors.iter().count())?)
        .and_then(|value| value.checked_mul(count(schedule.len()).ok()?.saturating_add(1)))
        .and_then(|value| value.checked_mul(8))
        .ok_or(HeapError::ExtentOverflow)?;
    budget.work(work, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}

/// One checked byte calculation serves every simultaneously retained allocation.
fn add(bytes: u64, length: usize, width: usize) -> Result<u64, LiveBindingError> {
    count(length)?
        .checked_mul(count(width)?)
        .and_then(|value| value.checked_add(bytes))
        .ok_or_else(|| HeapError::ExtentOverflow.into())
}

/// Only numerical arrays are copied; their exact declarations remain in the retained native plan.
pub(super) fn payloads<P: Scalar + 'static>(
    heap: &Heap,
    binding: &ReadingBinding<P>,
    schedule: &[Bundle],
    channels: [&[P]; 3],
) -> Result<Vec<Vec<P>>, LiveBindingError> {
    let mut result =
        crate::binding::buffer(schedule.len().checked_mul(3).ok_or(HeapError::Capacity)?)?;
    for item in schedule {
        for key in item.keys {
            result.push(
                heap.region_view::<P>(heap.descriptors.get(key)?)?
                    .export()?,
            );
        }
    }
    for (position, entry) in binding.adapter().entries().iter().enumerate() {
        let key = binding
            .group
            .tickets
            .get(entry.region)
            .ok_or(HeapError::InvalidLayout)?
            .key;
        let item = schedule
            .iter()
            .position(|item| item.keys[0] == key)
            .ok_or(HeapError::InvalidLayout)?;
        let index = usize::try_from(entry.index).map_err(|_| HeapError::InvalidLayout)?;
        for (channel, values) in channels.iter().enumerate() {
            *result
                .get_mut(3 * item + channel)
                .and_then(|values| values.get_mut(index))
                .ok_or(HeapError::InvalidLayout)? = values[position];
        }
    }
    Ok(result)
}

/// Native target revisions cover whole bases, so no old sibling can retain published availability.
pub(super) fn invalidate_native(
    invalidated: &mut Invalidations,
    schedule: &[Bundle],
    budget: &mut Budget,
) -> Result<(), LiveBindingError> {
    for (group, readers) in invalidated {
        if group
            .native
            .iter()
            .any(|companion| schedule.iter().any(|item| item.keys[0] == companion.target))
        {
            budget.work(count(group.states.len())?, AT)?;
            budget.check(
                BudgetKind::StagedPayloadBytes,
                count(group.states.len())?
                    .checked_mul(count(std::mem::size_of::<usize>())?)
                    .ok_or(HeapError::ExtentOverflow)?,
                AT,
            )?;
            readers.clear();
            readers
                .try_reserve_exact(group.states.len())
                .map_err(|_| HeapError::Capacity)?;
            readers.extend(0..group.states.len());
        }
    }
    Ok(())
}

/// Validate every old location and successor cover before the storage owner allocates new slabs.
pub(super) fn prepare<P: Scalar + 'static>(
    heap: &mut Heap,
    schedule: &[Bundle],
    payloads: &[Vec<P>],
) -> Result<Prepared, LiveBindingError> {
    let mut retired = crate::binding::buffer(payloads.len())?;
    let mut covers = crate::binding::buffer(payloads.len())?;
    for item in schedule {
        for key in item.keys {
            let Location::Owned(region) = heap.descriptors.get(key)?.location else {
                return Err(HeapError::InvalidLayout.into());
            };
            retired.push(region);
            let mut cover = heap.symbols.cover(key)?.clone();
            cover.stamp = item.stamp;
            for &index in &item.indices {
                let version = cover.versions.get(index).ok_or(HeapError::InvalidLayout)?;
                version.set(
                    version
                        .get()
                        .checked_add(1)
                        .ok_or(HeapError::CounterExhausted)?,
                );
            }
            covers.push(cover);
        }
    }
    let slices = payloads.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let storage = heap.storage.prepare(&slices, &retired, heap.limits)?;
    let locations = locations::<P>(schedule, payloads, &storage)?;
    Ok(Prepared {
        storage,
        locations,
        covers,
    })
}

/// Match the complete allocation result before any numerical or validity publication.
fn locations<P: Scalar + 'static>(
    schedule: &[Bundle],
    payloads: &[Vec<P>],
    storage: &PreparedStorage,
) -> Result<Vec<PublishedLocation>, HeapError> {
    if storage.keys().len() != payloads.len()
        || schedule.len().checked_mul(3) != Some(payloads.len())
    {
        return Err(HeapError::LengthMismatch);
    }
    let mut result = crate::binding::buffer(payloads.len())?;
    for ((item, &arena), payload) in schedule
        .iter()
        .flat_map(|item| item.keys.iter().map(move |&key| (item, key)))
        .zip(storage.keys())
        .zip(payloads)
    {
        result.push((
            item.1,
            Region {
                arena,
                epoch: storage.epoch(),
                format: Format::of::<P>(),
                offset: 0,
                length: payload.len(),
            },
            item.0.stamp,
        ));
    }
    Ok(result)
}

/// The exclusive lease preserves all prechecked keys; no allocation or callback occurs here.
pub(super) fn publish(heap: &mut Heap, prepared: Prepared) {
    heap.storage.publish(prepared.storage);
    for (key, record) in heap.descriptors.iter_mut() {
        if let Some((_, region, next)) = prepared
            .locations
            .iter()
            .find(|(actual, _, _)| *actual == key)
        {
            record.location = Location::Owned(*region);
            match &mut record.meaning {
                Meaning::Value {
                    stamp, coverage, ..
                } => {
                    *stamp = *next;
                    *coverage = crate::Coverage::DeclaredBounds;
                }
                Meaning::Bound { stamp, .. } => *stamp = *next,
                Meaning::Alias { .. } | Meaning::Statistical => {}
            }
        }
    }
    heap.symbols.publish(prepared.covers);
}
