// SPDX-License-Identifier: AGPL-3.0-only
//! Typed permits stay attached to their numerical allocations through candidate publication.

use super::PrivateAllocationError as Error;
use super::run::{Permit, Plan, State};
use crate::OperationError;
use std::rc::Rc;

/// One closed executor supplies the original non-cloneable allocating authority.
#[derive(Debug)]
enum Authority<T> {
    /// Executors outside the covered schedules retain their existing ordinary allocations.
    Untracked,
    /// Reciprocal declarations identify each ordered heterogeneous component.
    Reciprocal(Permit<T>),
    /// Deterministic solve stages retain original source and output bindings.
    Solve(crate::solve::scratch::Permit),
}
/// Candidate storage retains its original typed permit; covered recipes require it at publication.
#[derive(Debug)]
pub(crate) struct Buffer<T> {
    /// Covered arrays never detach the original pre-allocation authority from their owner.
    permit: Authority<T>,
    /// One numerical allocation, exposed only through fixed-length slices.
    values: Vec<T>,
}
impl<T> Buffer<T> {
    /// Only a successfully drawn typed ticket can enter the covered numerical allocator.
    pub(super) fn allocate(permit: Permit<T>) -> Result<Self, OperationError> {
        let slot = permit.slot;
        let entry = permit
            .plan
            .specifications
            .get(slot)
            .ok_or(Error::UnplannedMeaning)?;
        if entry.state.get() != State::Issued || entry.binding.entries() != permit.entries {
            return Err(Error::DuplicateTicket.into());
        }
        let values = crate::binding::buffer(permit.entries)?;
        let bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(OperationError::ResourceLimit)?;
        permit.plan.allocated(slot, bytes)?;
        Ok(Self {
            permit: Authority::Reciprocal(permit),
            values,
        })
    }
    /// The solve planner checks the concrete scalar type before passing this permit.
    pub(crate) fn allocate_solve(
        permit: crate::solve::scratch::Permit,
    ) -> Result<Self, OperationError> {
        let values = crate::binding::buffer(permit.entries())?;
        let bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(OperationError::ResourceLimit)?;
        permit.allocated(bytes)?;
        Ok(Self {
            permit: Authority::Solve(permit),
            values,
        })
    }
    /// Initialize a planned element without permitting vector growth or a hidden allocation.
    pub(crate) fn push(&mut self, value: T) -> Result<(), OperationError> {
        let count = match &self.permit {
            Authority::Untracked => return Err(Error::UnticketedBuffer.into()),
            Authority::Reciprocal(permit) => permit
                .plan
                .specifications
                .get(permit.slot)
                .ok_or(Error::UnplannedMeaning)?
                .binding
                .entries(),
            Authority::Solve(permit) => permit.entries(),
        };
        if self.values.len() >= count {
            return Err(Error::IncompleteCoverage.into());
        }
        self.values.push(value);
        Ok(())
    }
    /// Preserve both the initialized owner and its permit when entering a private candidate.
    pub(crate) fn into_candidate(self) -> Result<Self, OperationError> {
        match &self.permit {
            Authority::Untracked => return Err(Error::UnticketedBuffer.into()),
            Authority::Reciprocal(permit) => {
                let entry = permit
                    .plan
                    .specifications
                    .get(permit.slot)
                    .ok_or(Error::UnplannedMeaning)?;
                if self.values.len() != entry.binding.entries() {
                    return Err(Error::IncompleteCoverage.into());
                }
                entry.state.set(State::Candidate);
            }
            Authority::Solve(permit) => permit.candidate(self.values.len())?,
        }
        Ok(self)
    }
    /// A live permit must belong to this exact plan and channel, independent of allocator addresses.
    pub(super) fn matches(&self, plan: &Rc<Plan>, slot: usize) -> bool {
        matches!(&self.permit, Authority::Reciprocal(permit) if Rc::ptr_eq(&permit.plan, plan) && permit.slot == slot)
    }
    /// Solve publication authenticates the actual payload owner's original finite stage.
    pub(crate) fn matches_solve(
        &self,
        plan: &Rc<crate::solve::scratch::Plan>,
        slot: usize,
    ) -> bool {
        matches!(&self.permit, Authority::Solve(permit) if permit.matches(plan, slot))
    }
    /// Borrow the plan retained by a live solve owner without manufacturing another permit.
    pub(crate) const fn solve_plan(&self) -> Option<&Rc<crate::solve::scratch::Plan>> {
        if let Authority::Solve(permit) = &self.permit {
            Some(permit.plan())
        } else {
            None
        }
    }
    /// Borrow fixed-length coordinates without granting access to the allocation owner.
    pub(crate) const fn as_slice(&self) -> &[T] {
        self.values.as_slice()
    }
}
impl<T> From<Vec<T>> for Buffer<T> {
    fn from(values: Vec<T>) -> Self {
        Self {
            permit: Authority::Untracked,
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
        match &self.permit {
            Authority::Reciprocal(permit) => {
                if let Some(entry) = permit.plan.specifications.get(permit.slot) {
                    entry.state.set(State::Released(self.values.len()));
                }
            }
            Authority::Solve(permit) => permit.release(self.values.len()),
            Authority::Untracked => {}
        }
    }
}
