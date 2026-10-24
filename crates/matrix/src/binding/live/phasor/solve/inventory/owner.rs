// SPDX-License-Identifier: AGPL-3.0-only
//! Permits remain attached to actual fixed arrays, vector buffers and publication copies.

use super::{Permit, PhasorError, State};
use nrz_scalar::Scalar;

/// An inline numerical value and its non-cloneable original allocation authority.
#[derive(Debug)]
pub(crate) struct Owner<P: Scalar, T> {
    /// Actual fixed fields or array, never cloned by evaluation clones.
    pub(super) value: T,
    /// The sole permit is dropped only after this numerical owner.
    pub(super) permit: Permit<P>,
}
impl<P: Scalar, T> std::ops::Deref for Owner<P, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<P: Scalar, T> std::ops::DerefMut for Owner<P, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
/// Observable vector capacity with only finite slice mutation exposed to numerical procedures.
#[derive(Debug)]
pub(crate) struct Buffer<P: Scalar, T = P>(pub(super) Owner<P, Vec<T>>);
impl<P: Scalar, T> std::ops::Deref for Buffer<P, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.0.value
    }
}
impl<P: Scalar, T> std::ops::DerefMut for Buffer<P, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.value
    }
}
impl<P: Scalar> Permit<P> {
    /// Preserve common lifetimes; metadata and numerical bytes are never mixed.
    pub(super) fn allocated(&self, bytes: usize) -> Result<(), PhasorError> {
        if self.plan.states[self.slot].get() != State::Issued {
            return Err(crate::PrivateAllocationError::DuplicateTicket.into());
        }
        let live = self
            .plan
            .live
            .get()
            .checked_add(bytes)
            .ok_or(crate::HeapError::Capacity)?;
        if live
            .checked_add(super::record::metadata::<P>())
            .is_none_or(|total| total > 4096)
        {
            return Err(crate::HeapError::Capacity.into());
        }
        self.plan.bytes[self.slot].set(bytes);
        self.plan.live.set(live);
        self.plan.peak.set(self.plan.peak.get().max(live));
        self.plan.states[self.slot].set(State::Live);
        Ok(())
    }
}
impl<P: Scalar> Drop for Permit<P> {
    fn drop(&mut self) {
        if self.plan.states[self.slot].get() == State::Live {
            self.plan
                .live
                .set(self.plan.live.get() - self.plan.bytes[self.slot].get());
            self.plan.states[self.slot].set(State::Released);
        }
    }
}
/// All three publication vectors move together with their exact original permits.
#[derive(Debug)]
pub(crate) struct Regions<P: Scalar> {
    /// Immutable publication inputs; storage import borrows these while their permits remain live.
    values: [crate::ReadingRegion<P>; 3],
    /// Original center, total and error authority in the same order.
    permits: [Permit<P>; 3],
}
impl<P: Scalar> Regions<P> {
    /// Consume vector owners without creating another float allocation or detaching authority.
    pub(super) fn new(buffers: [Buffer<P>; 3]) -> Result<Self, PhasorError> {
        let [Buffer(center), Buffer(total), Buffer(error)] = buffers;
        Ok(Self {
            values: [
                crate::ReadingRegion {
                    extent: crate::Extent::new(2, 2)?,
                    values: center.value,
                },
                crate::ReadingRegion {
                    extent: crate::Extent::new(4, 1)?,
                    values: total.value,
                },
                crate::ReadingRegion {
                    extent: crate::Extent::new(4, 1)?,
                    values: error.value,
                },
            ],
            permits: [center.permit, total.permit, error.permit],
        })
    }
    /// Recheck the exact three-vector association before borrowing publication inputs.
    pub(crate) fn values(
        &self,
        run: &super::Run<P>,
    ) -> Result<&[crate::ReadingRegion<P>], PhasorError> {
        for (index, permit) in self.permits.iter().enumerate() {
            run.check_permit(permit, 46 + index)?;
        }
        Ok(&self.values)
    }
}
