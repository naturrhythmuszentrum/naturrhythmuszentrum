// SPDX-License-Identifier: AGPL-3.0-only
//! Concrete ordered target maps retain versions rather than buffer addresses.

use crate::heap::{identity::Key, records::Meaning};
use crate::{Handle, Heap, HeapError, JointError, Layout, SchemaId, Value, ValueStamp};
use nrz_scalar::{Precision, Scalar};
use std::marker::PhantomData;

/// Precision-erased immutable target used only inside the heap owner.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    /// Original selected occupation, including aliases.
    pub(crate) key: Key,
    /// Owning heap identity.
    scope: u64,
    /// Base identity and version.
    pub(crate) stamp: ValueStamp,
    /// Selected logical column order.
    pub(crate) layout: Layout,
    /// Exact homogeneous base interpretation.
    schema: SchemaId,
    /// Original sealed floating format.
    pub(crate) precision: Precision,
}
/// Ordered target selection captured from a current heap value.
#[derive(Clone, Debug)]
pub struct TargetMap<P: Scalar> {
    /// Erased immutable concrete relation.
    pub(crate) target: Target,
    /// Sealed precision marker.
    marker: PhantomData<P>,
}
impl<P: Scalar + 'static> TargetMap<P> {
    /// Capture the exact current value version and selected logical entry order.
    ///
    /// # Errors
    /// Refuses foreign/stale values or invalid existing evidence.
    pub fn capture(heap: &Heap, handle: Handle<Value, P>) -> Result<Self, JointError> {
        let value = heap.value(handle)?;
        Ok(Self {
            target: Target {
                key: handle.key,
                scope: handle.scope,
                stamp: value.stamp(),
                layout: value.region().layout().clone(),
                schema: value.region().schema_id(),
                precision: P::PRECISION,
            },
            marker: PhantomData,
        })
    }
    /// Captured mathematical identity and version.
    #[must_use]
    pub const fn stamp(&self) -> ValueStamp {
        self.target.stamp
    }
    /// Ordered base-region selection.
    #[must_use]
    pub const fn layout(&self) -> &Layout {
        &self.target.layout
    }
    /// Revalidate the exact ordered selection against a proposed target.
    ///
    /// # Errors
    /// Refuses another occupation, selection, schema or version, even with equal units.
    pub fn validate(&self, heap: &Heap, handle: Handle<Value, P>) -> Result<(), JointError> {
        self.target.validate(heap)?;
        if handle.scope != self.target.scope || handle.key != self.target.key {
            return Err(JointError::WrongTarget);
        }
        Ok(())
    }
}
impl Target {
    /// Raw dependency validation avoids recursive companion-to-value access.
    pub(crate) fn validate(&self, heap: &Heap) -> Result<(), HeapError> {
        if self.scope != heap.scope {
            return Err(HeapError::ForeignHandle);
        }
        let record = heap.descriptors.get(self.key)?;
        let base = match record.meaning {
            Meaning::Value { .. } => record,
            Meaning::Alias { base } => heap.descriptors.get(base)?,
            _ => return Err(HeapError::CategoryMismatch),
        };
        if !matches!(base.meaning, Meaning::Value { stamp, .. } if stamp == self.stamp)
            || record.schema != self.schema.index
            || record.layout != self.layout
        {
            return Err(HeapError::StaleEvidence);
        }
        let format = match self.precision {
            Precision::F32 => crate::storage::Format::Binary32,
            Precision::F64 => crate::storage::Format::Binary64,
        };
        let region = heap.owning_region(record)?;
        if region.format != format {
            return Err(HeapError::PrecisionMismatch);
        }
        self.layout
            .validate(region.length, heap.limits.layout_work)?;
        heap.storage.resolve(region)?;
        Ok(())
    }
}
