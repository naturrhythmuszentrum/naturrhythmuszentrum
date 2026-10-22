// SPDX-License-Identifier: AGPL-3.0-only
//! Exact immutable schema interning for homogeneous contiguous regions.

use nrz_bindings::{Descriptor, Measure};

use super::{Extent, HeapError};

/// Extents and physical interpretation, shared without merging value identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Schema {
    /// Logical rectangular extent.
    pub(crate) extent: Extent,
    /// Independent quantity descriptor.
    pub(crate) quantity: Descriptor,
}

/// Admit coherent storage while keeping discrete counts outside floating arenas.
pub(crate) fn check_quantity(quantity: &Descriptor) -> Result<(), HeapError> {
    if quantity.kind().count_domain().is_some() {
        return Err(HeapError::CountStorage);
    }
    let coherent = Measure::coherent();
    if quantity.measure().scale() != coherent.scale()
        || quantity.measure().offset() != coherent.offset()
    {
        return Err(HeapError::NonCoherentMeasure);
    }
    Ok(())
}

/// Prepare a schema index without changing the visible pool.
pub(crate) fn intern(
    pool: &[Schema],
    pending: &mut Vec<Schema>,
    schema: Schema,
    limit: usize,
) -> Result<usize, HeapError> {
    if let Some(index) = pool
        .iter()
        .chain(pending.iter())
        .position(|old| old == &schema)
    {
        return Ok(index);
    }
    let index = pool
        .len()
        .checked_add(pending.len())
        .ok_or(HeapError::Capacity)?;
    if index >= limit {
        return Err(HeapError::Capacity);
    }
    pending.try_reserve(1).map_err(|_| HeapError::Capacity)?;
    pending.push(schema);
    Ok(index)
}
