// SPDX-License-Identifier: AGPL-3.0-only
//! Actual owners retain their original permit until their storage is released.

use super::run::{Permit, State};
use crate::{OperationError, PrivateAllocationError as Error};

/// Opaque backend storage and its pre-allocation logical request remain one owner.
#[derive(Debug)]
pub(crate) struct Backend<T> {
    /// Backend object, accessed only through its safe public interface.
    pub(super) value: T,
    /// Original non-cloneable request ticket.
    pub(super) permit: Permit,
}
impl<T> Backend<T> {
    /// Borrow the backend object without permitting replacement or detachment from its permit.
    pub(crate) const fn get(&self) -> &T {
        &self.value
    }

    /// Original authority of this actual backend owner, used when exposing a factor.
    pub(crate) const fn ticket(&self) -> crate::PrivateAllocationTicket {
        self.permit.ticket
    }
}
impl<T> Drop for Backend<T> {
    fn drop(&mut self) {
        self.permit.plan.states[self.permit.slot].set(State::Released(self.permit.plan.entries));
    }
}
/// Matrix-owned inverse coordinates or defect entries with no route to unplanned vector growth.
#[derive(Debug)]
pub(crate) struct Buffer<T> {
    /// Actual numerical allocation, never detached from its permit.
    values: Vec<T>,
    /// Exact original owner and finite stage.
    permit: Permit,
}
impl<T> Buffer<T> {
    /// Only a previously issued typed permit reaches the allocator.
    pub(super) fn new(permit: Permit) -> Result<Self, OperationError> {
        if permit.plan.states[permit.slot].get() != State::Issued {
            return Err(Error::DuplicateTicket.into());
        }
        let values = crate::binding::buffer(permit.plan.entries)?;
        let bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(OperationError::ResourceLimit)?;
        permit.plan.trace.allocated(bytes)?;
        permit.plan.bytes[permit.slot].set(Some(bytes));
        permit.plan.states[permit.slot].set(State::Live);
        Ok(Self { values, permit })
    }
    /// Initialize only the finite planned entry set; no append can reallocate storage.
    pub(crate) fn push(&mut self, value: T) -> Result<(), OperationError> {
        if self.values.len() >= self.permit.plan.entries {
            return Err(Error::IncompleteCoverage.into());
        }
        self.values.push(value);
        Ok(())
    }
}
impl<T> std::ops::Deref for Buffer<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
impl<T> std::ops::DerefMut for Buffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}
impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        self.permit
            .plan
            .trace
            .released(self.permit.plan.bytes[self.permit.slot].get().unwrap_or(0));
        self.permit.plan.states[self.permit.slot].set(State::Released(self.values.len()));
    }
}
