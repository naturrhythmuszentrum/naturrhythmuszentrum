// SPDX-License-Identifier: AGPL-3.0-only
//! Reprepare one complete reservation with statistical regions appended after value keys.

use super::StatisticsError;
use crate::heap::{
    identity::Key,
    records::{Location, Meaning, Record},
    schema::{Schema, intern},
};
use crate::storage::{Format, Region};
use crate::{Bound, Candidate, Extent, OperationError};
use nrz_bindings::Descriptor;
use nrz_scalar::Scalar;

/// One independently shaped statistical region awaiting the atomic transaction commit.
#[derive(Debug)]
pub(crate) struct RegionData<P: Scalar> {
    /// Parameter-specific logical shape.
    pub(crate) extent: Extent,
    /// Homogeneous schema; paired matrices retain per-entry descriptors in their owner.
    pub(crate) quantity: Descriptor,
    /// Private finite numerical payload.
    pub(crate) values: Vec<P>,
}

/// Preserve every reserved value key while preparing all numerical storage in one change.
pub(crate) fn append<P: Scalar + 'static, Q: Scalar + 'static>(
    bound: &mut Bound<'_, P, Q>,
    candidate: &Candidate<Q>,
    regions: &[RegionData<Q>],
) -> Result<Vec<Key>, OperationError> {
    let mut payloads = ordinary(candidate);
    let prefix = payloads.len();
    payloads.extend(regions.iter().map(|region| region.values.as_slice()));
    let plan = bound
        .heap
        .descriptors
        .prepare(payloads.len(), bound.heap.limits.descriptors)?;
    if plan.keys().get(..prefix) != Some(bound.reservations.records.keys()) {
        return Err(StatisticsError::Contract.into());
    }
    let storage = bound
        .heap
        .storage
        .prepare(&payloads, &[], bound.heap.limits)?;
    if storage.keys().get(..prefix) != Some(bound.reservations.storage.keys()) {
        return Err(StatisticsError::Contract.into());
    }
    let mut appended = crate::binding::buffer(regions.len())?;
    for (index, region) in regions.iter().enumerate() {
        let schema = intern(
            &bound.heap.schemas,
            &mut bound.reservations.schemas,
            Schema {
                extent: region.extent,
                quantity: region.quantity.clone(),
            },
            bound.heap.limits.schemas,
        )?;
        let arena = *storage
            .keys()
            .get(prefix + index)
            .ok_or(StatisticsError::Contract)?;
        appended.push(Record {
            schema,
            layout: crate::Layout::contiguous(region.extent)?,
            location: Location::Owned(Region {
                arena,
                epoch: storage.epoch(),
                format: Format::of::<Q>(),
                offset: 0,
                length: region.extent.entries(),
            }),
            meaning: Meaning::Statistical,
        });
    }
    bound
        .heap
        .schemas
        .try_reserve(bound.reservations.schemas.len())
        .map_err(|_| StatisticsError::ResourceLimit)?;
    let keys = plan
        .keys()
        .get(prefix..)
        .ok_or(StatisticsError::Contract)?
        .to_vec();
    let mut records = std::mem::take(&mut bound.reservations.records.values);
    records
        .try_reserve(appended.len())
        .map_err(|_| StatisticsError::ResourceLimit)?;
    records.extend(appended);
    bound.reservations.records = plan.bind(records)?;
    bound.reservations.storage = storage;
    Ok(keys)
}

/// Numerical region order remains exactly the existing deterministic transaction order.
fn ordinary<P: Scalar>(candidate: &Candidate<P>) -> Vec<&[P]> {
    let mut result = Vec::new();
    for output in &candidate.outputs {
        result.push(output.centers.as_slice());
        result.extend(output.bounds.as_deref());
        result.extend(output.evaluation.as_deref());
        if let Some(parts) = &output.contributions {
            result.extend(parts.iter().map(Vec::as_slice));
        }
    }
    result
}

/// All fallible allocation is finished before a standalone cross-block publication.
pub(crate) struct Standalone {
    /// Complete private numerical owners.
    pub(crate) storage: crate::storage::PreparedStorage,
    /// Newly interned statistical schemas.
    pub(crate) schemas: Vec<Schema>,
    /// Complete private descriptor occupations.
    pub(crate) records: crate::heap::slots::PreparedSlots<Record>,
}
/// Prepare a separately requested statistical block without changing visible state.
pub(crate) fn standalone<P: Scalar + 'static>(
    heap: &mut crate::Heap,
    regions: &[RegionData<P>],
) -> Result<Standalone, OperationError> {
    let payloads = regions
        .iter()
        .map(|region| region.values.as_slice())
        .collect::<Vec<_>>();
    let plan = heap
        .descriptors
        .prepare(regions.len(), heap.limits.descriptors)?;
    let storage = heap.storage.prepare(&payloads, &[], heap.limits)?;
    let mut schemas = Vec::new();
    let mut records = crate::binding::buffer(regions.len())?;
    for (region, &arena) in regions.iter().zip(storage.keys()) {
        let schema = intern(
            &heap.schemas,
            &mut schemas,
            Schema {
                extent: region.extent,
                quantity: region.quantity.clone(),
            },
            heap.limits.schemas,
        )?;
        records.push(Record {
            schema,
            layout: crate::Layout::contiguous(region.extent)?,
            location: Location::Owned(Region {
                arena,
                epoch: storage.epoch(),
                format: Format::of::<P>(),
                offset: 0,
                length: region.extent.entries(),
            }),
            meaning: Meaning::Statistical,
        });
    }
    heap.schemas
        .try_reserve(schemas.len())
        .map_err(|_| StatisticsError::ResourceLimit)?;
    Ok(Standalone {
        storage,
        schemas,
        records: plan.bind(records)?,
    })
}
