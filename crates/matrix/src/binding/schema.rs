// SPDX-License-Identifier: AGPL-3.0-only
//! Bindings-side homogeneous interpretation and exact schema identity.

use nrz_bindings::{Descriptor, Measure};

use crate::{Extent, HeapError};

/// Extents and physical interpretation, shared without merging value identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BindingSchema {
    /// Logical rectangular extent.
    pub(crate) extent: Extent,
    /// Independent quantity descriptor.
    pub(crate) quantity: Option<Descriptor>,
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
    pool: &[BindingSchema],
    pending: &mut Vec<BindingSchema>,
    schema: BindingSchema,
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

/// Reserve both independent tables before any numerical or meaning publication.
pub(crate) fn reserve(heap: &mut crate::Heap, count: usize) -> Result<(), HeapError> {
    heap.schemas
        .try_reserve(count)
        .map_err(|_| HeapError::Capacity)?;
    heap.meanings
        .try_reserve(count)
        .map_err(|_| HeapError::Capacity)?;
    Ok(())
}

/// Install already reserved numerical shapes and descriptor-side interpretations together.
pub(crate) fn publish(heap: &mut crate::Heap, prepared: Vec<BindingSchema>) {
    for schema in prepared {
        heap.schemas.push(crate::heap::schema::Schema {
            extent: schema.extent,
        });
        heap.meanings.push(schema);
    }
}

impl crate::Heap {
    /// Resolve descriptor-side metadata without putting meaning into a numerical region view.
    pub(crate) fn binding_quantity<C: crate::Category, P: nrz_scalar::Scalar + 'static>(
        &self,
        handle: crate::Handle<C, P>,
    ) -> Result<&Descriptor, HeapError> {
        let record = self.record(handle)?;
        self.meanings
            .get(record.schema)
            .and_then(|schema| schema.quantity.as_ref())
            .ok_or(HeapError::MissingBinding)
    }
}
