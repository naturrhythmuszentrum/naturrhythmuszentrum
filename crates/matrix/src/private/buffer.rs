// SPDX-License-Identifier: AGPL-3.0-only
//! Typed permits stay attached to their numerical allocations through candidate publication.

use super::PrivateAllocationError as Error;
use super::run::{Permit, Plan, State};
use crate::OperationError;
use std::rc::Rc;

/// Candidate storage can retain a typed permit; covered recipes require it at publication.
#[derive(Debug)]
pub(crate) struct Buffer<T> {
    /// Covered arrays never detach the original pre-allocation authority from their owner.
    permit: Option<Permit<T>>,
    /// One numerical allocation, exposed only through fixed-length slices.
    values: Vec<T>,
}

impl<T> Buffer<T> {
    /// Only a successfully drawn typed ticket can enter the covered numerical allocator.
    pub(super) fn allocate(permit: Permit<T>) -> Result<Self, OperationError> {
        let slot = permit.ticket.slot;
        let entry = permit
            .plan
            .specifications
            .get(slot)
            .ok_or(Error::UnplannedMeaning)?;
        if entry.state.get() != State::Issued {
            return Err(Error::DuplicateTicket.into());
        }
        let values = crate::binding::buffer(entry.binding.entries())?;
        let bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(OperationError::ResourceLimit)?;
        permit.plan.allocated(slot, bytes)?;
        Ok(Self {
            permit: Some(permit),
            values,
        })
    }

    /// Initialize a planned element without permitting vector growth or a hidden allocation.
    pub(crate) fn push(&mut self, value: T) -> Result<(), OperationError> {
        let permit = self.permit.as_ref().ok_or(Error::UnticketedBuffer)?;
        let entry = permit
            .plan
            .specifications
            .get(permit.ticket.slot)
            .ok_or(Error::UnplannedMeaning)?;
        if self.values.len() >= entry.binding.entries() {
            return Err(Error::IncompleteCoverage.into());
        }
        self.values.push(value);
        Ok(())
    }

    /// Preserve both the initialized owner and its permit when entering a private candidate.
    pub(crate) fn into_candidate(self) -> Result<Self, OperationError> {
        let permit = self.permit.as_ref().ok_or(Error::UnticketedBuffer)?;
        let entry = permit
            .plan
            .specifications
            .get(permit.ticket.slot)
            .ok_or(Error::UnplannedMeaning)?;
        if self.values.len() != entry.binding.entries() {
            return Err(Error::IncompleteCoverage.into());
        }
        entry.state.set(State::Candidate);
        Ok(self)
    }

    /// A live permit must belong to this exact plan and channel, independent of allocator addresses.
    pub(super) fn matches(&self, plan: &Rc<Plan>, slot: usize) -> bool {
        self.permit
            .as_ref()
            .is_some_and(|permit| Rc::ptr_eq(&permit.plan, plan) && permit.ticket.slot == slot)
    }

    /// Borrow fixed-length coordinates without granting access to the allocation owner.
    pub(crate) const fn as_slice(&self) -> &[T] {
        self.values.as_slice()
    }
}

impl<T> From<Vec<T>> for Buffer<T> {
    /// Existing executors retain their ordinary arrays; covered recipes refuse this representation.
    fn from(values: Vec<T>) -> Self {
        Self {
            permit: None,
            values,
        }
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

impl<'a, T> IntoIterator for &'a Buffer<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        if let Some(permit) = &self.permit
            && let Some(entry) = permit.plan.specifications.get(permit.ticket.slot)
        {
            entry.state.set(State::Released(self.values.len()));
        }
    }
}
