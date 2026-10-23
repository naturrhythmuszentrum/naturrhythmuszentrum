// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical payload checks and preparation of versioned region records.

use super::identity::Key;
use super::records::{Location, Meaning, Record};
use super::{Extent, Handle, HeapError, PublishedBundle, ValueStamp};
use crate::Layout;
use crate::storage::{Format, Region};
use nrz_scalar::Scalar;

/// Schema indices and any new immutable records, still private to preparation.
#[derive(Debug)]
pub(crate) struct SchemaPlan {
    /// Value interpretation.
    pub(crate) value: usize,
    /// Independently derived radius interpretation.
    pub(crate) bound: Option<usize>,
    /// Separate numerical error descriptor when the recipe creates one.
    pub(crate) evaluation: Option<usize>,
    /// Radius schema shared by three independently stored product contributions.
    pub(crate) contributions: Option<usize>,
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
