// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh contiguous backend scratch for maps without an expressible borrowed view.

use std::marker::PhantomData;

use nrz_scalar::Scalar;

use crate::storage::{Format, Region, Storage};
use crate::{Extent, HeapError, HeapLimits};

/// An owned raw snapshot packed in logical column order, without heap evidence.
///
/// Backend mutation stays private. This object cannot serve as a published value handle.
#[derive(Debug)]
pub struct PackedRegion<P: Scalar> {
    /// Fresh backend owner, sharing no allocation with the heap.
    storage: Storage,
    /// The one packed contiguous allocation.
    region: Region,
    /// Logical rectangle represented by the one-column slab.
    extent: Extent,
    /// Sealed primitive format.
    marker: PhantomData<P>,
}

impl<P: Scalar + 'static> PackedRegion<P> {
    /// Allocate backend scratch only after the caller has bounded and checked the copy.
    pub(crate) fn new(extent: Extent, values: &[P]) -> Result<Self, HeapError> {
        if values.len() != extent.entries() {
            return Err(HeapError::LengthMismatch);
        }
        let mut storage = Storage::new();
        let prepared = storage.prepare(
            std::slice::from_ref(&values),
            &[],
            HeapLimits::new(1, values.len(), 0),
        )?;
        let arena = *prepared.keys().first().ok_or(HeapError::Capacity)?;
        storage.publish(prepared);
        Ok(Self {
            storage,
            region: Region {
                arena,
                format: Format::of::<P>(),
                offset: 0,
                length: values.len(),
            },
            extent,
            marker: PhantomData,
        })
    }

    /// Original logical rectangle, now packed contiguously.
    #[must_use]
    pub const fn extent(&self) -> Extent {
        self.extent
    }

    /// Read a raw packed entry in logical column order.
    ///
    /// # Errors
    /// Refuses out-of-range indices or an invalid internal allocation.
    pub fn get(&self, row: usize, column: usize) -> Result<P, HeapError> {
        self.storage
            .resolve(self.region)?
            .entry(self.extent.index(row, column)?)
    }
}
