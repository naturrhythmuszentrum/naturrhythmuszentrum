// SPDX-License-Identifier: AGPL-3.0-only
//! Origin and successful consumer shares belong to a lease before publication.

use super::{ExecutionInventory, SharedFactorAllocation, Trace};
use crate::{ExecutionId, OperationError, PrivateAllocationError, PrivateAllocationTicket};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Type erasure preserves actual ownership without exposing any backend representation.
pub(crate) trait ErasedFactor: std::fmt::Debug {
    /// Authority attached to this actual backend owner.
    fn ticket(&self) -> PrivateAllocationTicket;
    /// New aligned reference-count header of the shared numerical owner.
    fn header_bytes(&self) -> usize;
    /// Shared complete historical construction inventory, with no numerical owner reference.
    fn inventory(&self) -> &Rc<crate::FactorInventory>;
}
/// A share carries its exact original ticket and its optional successful candidate association.
#[derive(Debug)]
pub(super) struct Share {
    /// Independent strong numerical ownership, never copied into immutable history.
    resource: Rc<dyn ErasedFactor>,
    /// Current successful candidate, if this execution consumed this factor.
    candidate: Option<u64>,
}
/// The unpublished origin owns this lease before a factor can escape.
#[derive(Debug)]
pub(crate) struct Lease {
    /// Common allocation lifetime counter, with no numerical resource references.
    pub(crate) trace: Trace,
    /// Independent origin and completed consumer shares, deduplicated by exact ticket.
    shares: RefCell<Vec<Share>>,
    /// Original staged and retained metadata ceilings.
    limits: [u64; 2],
    /// Newly admitted backend work beyond the preserved original factor reservation.
    extra: Cell<usize>,
}
impl Lease {
    /// Admit registry metadata before creating the execution's shared counter.
    pub(crate) fn new(
        execution: ExecutionId,
        budget: &nrz_bindings::Budget,
    ) -> Result<Self, OperationError> {
        let bytes = Trace::metadata_bytes() + super::Registry::entry_bytes();
        let limits = [
            budget.limits().staged_payload_bytes,
            budget.limits().output_bytes,
        ];
        check(bytes, limits)?;
        Ok(Self {
            trace: Trace::new(execution),
            shares: RefCell::new(Vec::new()),
            limits,
            extra: Cell::new(0),
        })
    }
    /// Reserve the complete next origin association before constructing any factor payload.
    pub(crate) fn admit_factor(&self, header: usize) -> Result<(), OperationError> {
        self.admit(header)
    }
    /// Admit a new consumer association before its numerical solve begins.
    pub(crate) fn admit_consumer(&self, resource: &dyn ErasedFactor) -> Result<(), OperationError> {
        let shares = self
            .shares
            .try_borrow()
            .map_err(|_| OperationError::ResourceLimit)?;
        if shares
            .iter()
            .any(|share| share.resource.ticket() == resource.ticket())
        {
            return Ok(());
        }
        drop(shares);
        self.admit(0)
    }
    /// Both metadata ceilings cover the complete planned final registry and receipt.
    fn admit(&self, header: usize) -> Result<(), OperationError> {
        let shares = self
            .shares
            .try_borrow()
            .map_err(|_| OperationError::ResourceLimit)?;
        let count = shares
            .len()
            .checked_add(1)
            .ok_or(OperationError::ResourceLimit)?;
        let bytes = self
            .metadata(&shares, count.max(shares.capacity()), count)?
            .checked_add(header)
            .ok_or(OperationError::ResourceLimit)?;
        check(bytes, self.limits)
    }
    /// Retain a factor before it escapes its origin or a successful consumer candidate.
    pub(crate) fn retain(
        &self,
        resource: Rc<dyn ErasedFactor>,
        backend: usize,
        candidate: Option<u64>,
    ) -> Result<(), OperationError> {
        let ticket = resource.ticket();
        let mut shares = self
            .shares
            .try_borrow_mut()
            .map_err(|_| OperationError::ResourceLimit)?;
        if let Some(share) = shares
            .iter_mut()
            .find(|share| share.resource.ticket() == ticket)
        {
            if !Rc::ptr_eq(&share.resource, &resource)
                || share.resource.inventory().backend_allowance_bytes() != backend
            {
                return Err(PrivateAllocationError::ForeignInventory.into());
            }
            if candidate.is_some() {
                if share.candidate.is_some() && share.candidate != candidate {
                    return Err(PrivateAllocationError::DuplicateTicket.into());
                }
                share.candidate = candidate;
            }
            return Ok(());
        }
        let count = shares
            .len()
            .checked_add(1)
            .ok_or(OperationError::ResourceLimit)?;
        let extra = if ticket.execution() == self.trace.execution() {
            resource.header_bytes()
        } else {
            0
        };
        let bytes = self
            .metadata(&shares, count, count)?
            .checked_add(extra)
            .ok_or(OperationError::ResourceLimit)?;
        check(bytes, self.limits)?;
        shares
            .try_reserve_exact(1)
            .map_err(|_| OperationError::ResourceLimit)?;
        if resource.inventory().allocations()[1].ticket() != ticket
            || resource.inventory().backend_allowance_bytes() != backend
        {
            return Err(PrivateAllocationError::ForeignInventory.into());
        }
        shares.push(Share {
            resource,
            candidate,
        });
        Ok(())
    }
    /// Keep the original factor allowance and only the newly admitted additional work.
    pub(crate) fn completed_backend(
        &self,
        original: usize,
        current: usize,
        statistics: usize,
    ) -> Result<(), OperationError> {
        let extra = current
            .saturating_sub(original)
            .checked_add(statistics)
            .ok_or(OperationError::ResourceLimit)?;
        self.extra.set(extra);
        Ok(())
    }
    /// Authenticate the original resource and exact successful candidate association.
    pub(crate) fn validate<P: nrz_scalar::Scalar>(
        &self,
        candidate: &crate::Candidate<P>,
    ) -> Result<(), OperationError> {
        let shares = self
            .shares
            .try_borrow()
            .map_err(|_| OperationError::ResourceLimit)?;
        for evidence in &candidate.evidence {
            if let crate::RecipeEvidence::Solve(claim) = evidence {
                let usage = claim
                    .factor_use()
                    .ok_or(PrivateAllocationError::IncompleteCoverage)?;
                let share = shares
                    .iter()
                    .find(|share| share.resource.ticket() == usage.ticket())
                    .ok_or(PrivateAllocationError::ForeignInventory)?;
                if usage.execution() != self.trace.execution()
                    || share.candidate != Some(candidate.token)
                    || share.resource.ticket() != usage.ticket()
                    || usage.origin() != usage.ticket().execution()
                {
                    return Err(PrivateAllocationError::ForeignInventory.into());
                }
            }
        }
        Ok(())
    }
    /// Snapshot exact identities and common observations without retaining any numerical resource.
    pub(crate) fn snapshot(&self) -> Result<ExecutionInventory, OperationError> {
        let shares = self
            .shares
            .try_borrow()
            .map_err(|_| OperationError::ResourceLimit)?;
        let mut factors = crate::binding::buffer(shares.len())?;
        factors.extend(shares.iter().map(|share| SharedFactorAllocation {
            inventory: Rc::clone(share.resource.inventory()),
        }));
        let metadata = self.metadata(&shares, shares.capacity(), factors.capacity())?;
        check(metadata, self.limits)?;
        let backend = factors.iter().try_fold(self.extra.get(), |sum, factor| {
            sum.checked_add(factor.backend_allowance_bytes())
                .ok_or(OperationError::ResourceLimit)
        })?;
        let [allocations, bytes, peak] = self.trace.observations();
        Ok(ExecutionInventory {
            execution: self.trace.execution(),
            factors,
            allocations,
            bytes,
            peak,
            metadata,
            backend,
        })
    }
    /// Transfer only resource shares; publication drops temporary limits and live-counter handles.
    pub(super) fn into_shares(self) -> Vec<Share> {
        self.shares.into_inner()
    }
    /// Count each actual association allocation and each newly originating owner header once.
    fn metadata(
        &self,
        shares: &[Share],
        capacity: usize,
        receipts: usize,
    ) -> Result<usize, OperationError> {
        let mut bytes = Trace::metadata_bytes() + super::Registry::entry_bytes();
        for share in shares {
            if share.resource.ticket().execution() == self.trace.execution() {
                bytes = bytes
                    .checked_add(share.resource.header_bytes())
                    .ok_or(OperationError::ResourceLimit)?;
            }
        }
        bytes
            .checked_add(
                capacity
                    .checked_mul(size_of::<Share>())
                    .ok_or(OperationError::ResourceLimit)?,
            )
            .and_then(|bytes| {
                bytes.checked_add(receipts.checked_mul(size_of::<SharedFactorAllocation>())?)
            })
            .ok_or(OperationError::ResourceLimit)
    }
}
/// The original caller ceilings govern each complete retained metadata set.
fn check(bytes: usize, limits: [u64; 2]) -> Result<(), OperationError> {
    let bytes = u64::try_from(bytes).map_err(|_| OperationError::ResourceLimit)?;
    if limits.into_iter().any(|limit| bytes > limit) {
        return Err(OperationError::ResourceLimit);
    }
    Ok(())
}
