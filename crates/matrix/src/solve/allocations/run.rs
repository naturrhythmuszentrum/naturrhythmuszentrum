// SPDX-License-Identifier: AGPL-3.0-only
//! A closed factor plan precedes backend entry and owns each permit's exact interpretation.

use super::{Backend, Buffer, FactorAllocation, FactorAllocationStage as Stage, FactorInventory};
use crate::{
    ExecutionId, FactorDependency, InputSnapshot, OperationError, PrivateAllocationError as Error,
    PrivateAllocationTicket, RegionContract,
};
use nrz_bindings::BindingSymbols;
use nrz_scalar::Scalar;
use std::{any::TypeId, cell::Cell, rc::Rc};

/// One non-reusable stage's actual lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum State {
    /// Complete allocating interpretation exists, with no ticket issued.
    Planned,
    /// A ticket was issued before an allocation call.
    Issued,
    /// The numerical owner holds the issued permit.
    Live,
    /// The complete initialized owner was released.
    Released(usize),
}
/// Shared exact operator binding and finite allocation schedule.
#[derive(Debug)]
pub(super) struct Plan {
    /// Original immutable checked coordinate contracts, shared without rebuilding declarations.
    pub(super) inputs: Rc<[RegionContract]>,
    /// Original source and companion symbols, including their original graph owners.
    pub(super) source: BindingSymbols,
    /// Exact selected operator and algorithm revision.
    pub(super) dependency: FactorDependency,
    /// Origin execution, independent of the storage address.
    execution: ExecutionId,
    /// Unique factor attempt, including repeated factor requests before publication.
    token: u64,
    /// Positive square order checked before the first numerical allocation.
    pub(super) order: usize,
    /// Complete square entry count.
    pub(super) entries: usize,
    /// Exact logical scalar bytes for one square.
    pub(super) logical: usize,
    /// Concrete scalar representation required of wrapper-owned buffers.
    kind: TypeId,
    /// Opaque backend heap/workspace allowance, distinct from observed capacity.
    backend: usize,
    /// Finite non-reusable stage states.
    pub(super) states: [Cell<State>; 6],
    /// Actual vector capacity; backend-owned capacity remains absent.
    pub(super) bytes: [Cell<Option<usize>>; 6],
}
/// A factor construction keeps one plan before any allocation request.
#[derive(Debug)]
pub(crate) struct Run(pub(super) Rc<Plan>);
/// Non-cloneable allocation authority retained beside the actual owner.
#[derive(Debug)]
pub(super) struct Permit {
    /// Shared allocating interpretation and lifecycle state.
    pub(super) plan: Rc<Plan>,
    /// Exact finite stage position.
    pub(super) slot: usize,
    /// Opaque ticket drawn before the allocator is entered.
    pub(super) ticket: PrivateAllocationTicket,
}
impl Run {
    /// Admit aligned newly owned metadata under the existing caller staged/output byte policies.
    pub(crate) fn check_budget(
        dependency: &FactorDependency,
        budget: &nrz_bindings::Budget,
    ) -> Result<(), OperationError> {
        use nrz_bindings::readings::algebra::limits::BudgetKind;
        let bytes = size_of::<RcLayout>()
            .checked_add(size_of::<FactorInventory>())
            .and_then(|bytes| bytes.checked_add(dependency.layout().owned_bytes()))
            .ok_or(OperationError::ResourceLimit)?;
        let bytes = u64::try_from(bytes).map_err(|_| OperationError::ResourceLimit)?;
        let location =
            nrz_bindings::readings::algebra::Location::new("factor allocation inventory");
        budget.check(BudgetKind::StagedPayloadBytes, bytes, location)?;
        budget.check(BudgetKind::OutputBytes, bytes, location)?;
        Ok(())
    }
    /// Reuse checked exact source declarations and admit the complete factor schedule.
    pub(crate) fn new<P: Scalar + 'static>(
        execution: ExecutionId,
        input: &InputSnapshot<P>,
        contract: &super::super::SolveContract,
        dependency: &FactorDependency,
    ) -> Result<Self, OperationError> {
        let extent = contract
            .inputs
            .first()
            .ok_or(Error::IncompleteCoverage)?
            .shape()
            .extent();
        if extent.rows() == 0
            || extent.rows() != extent.columns()
            || input.layout().extent() != extent
        {
            return Err(Error::UnplannedMeaning.into());
        }
        let logical = extent
            .entries()
            .checked_mul(size_of::<P>())
            .ok_or(OperationError::ResourceLimit)?;
        let backend = contract
            .work
            .checked_mul(size_of::<P>())
            .ok_or(OperationError::ResourceLimit)?;
        if logical
            .checked_mul(4)
            .is_none_or(|required| required > backend)
        {
            return Err(OperationError::ResourceLimit);
        }
        Ok(Self(Rc::new(Plan {
            inputs: Rc::clone(&contract.inputs),
            source: input.symbols.clone(),
            dependency: dependency.clone(),
            execution,
            token: crate::heap::identity::identity()?,
            order: extent.rows(),
            entries: extent.entries(),
            logical,
            kind: TypeId::of::<P>(),
            backend,
            states: std::array::from_fn(|_| Cell::new(State::Planned)),
            bytes: std::array::from_fn(|_| Cell::new(None)),
        })))
    }
    /// Draw a backend request ticket before constructing the opaque owner.
    pub(crate) fn backend<T: 'static>(
        &self,
        stage: Stage,
        allocate: impl FnOnce() -> Result<T, OperationError>,
    ) -> Result<Backend<T>, OperationError> {
        if !stage.is_backend() || self.0.backend_kind(stage) != TypeId::of::<T>() {
            return Err(Error::Representation.into());
        }
        let permit = self.grant(stage)?;
        let value = allocate()?;
        permit.plan.states[permit.slot].set(State::Live);
        Ok(Backend { value, permit })
    }
    /// Matrix-owned scalar vectors require the planned concrete type and exact finite extent.
    pub(crate) fn buffer<P: Scalar + 'static>(
        &self,
        stage: Stage,
    ) -> Result<Buffer<P>, OperationError> {
        if stage.is_backend() || self.0.kind != TypeId::of::<P>() {
            return Err(Error::Representation.into());
        }
        Buffer::new(self.grant(stage)?)
    }
    /// A released stage cannot obtain a second ticket under the same factor attempt.
    fn grant(&self, stage: Stage) -> Result<Permit, OperationError> {
        let slot = Stage::ALL
            .iter()
            .position(|&value| value == stage)
            .ok_or(Error::UnplannedMeaning)?;
        if self.0.states[slot].get() != State::Planned {
            return Err(Error::DuplicateTicket.into());
        }
        let ticket = PrivateAllocationTicket::new(self.0.execution, self.0.token, slot);
        self.0.states[slot].set(State::Issued);
        Ok(Permit {
            plan: Rc::clone(&self.0),
            slot,
            ticket,
        })
    }
    /// Expose the factor only after all transient requests completed and the factor owner remains live.
    pub(crate) fn complete(
        &self,
        inline: usize,
        ticket: PrivateAllocationTicket,
    ) -> Result<FactorInventory, OperationError> {
        if ticket != PrivateAllocationTicket::new(self.0.execution, self.0.token, 1) {
            return Err(Error::ForeignInventory.into());
        }
        for (slot, stage) in Stage::ALL.into_iter().enumerate() {
            let expected = if stage == Stage::Factors {
                State::Live
            } else {
                State::Released(self.0.entries)
            };
            if self.0.states[slot].get() != expected {
                return Err(Error::IncompleteCoverage.into());
            }
        }
        let metadata = size_of::<RcLayout>()
            .checked_add(size_of::<FactorInventory>())
            .and_then(|bytes| bytes.checked_add(self.0.dependency.layout().owned_bytes()))
            .ok_or(OperationError::ResourceLimit)?;
        Ok(FactorInventory {
            allocations: std::array::from_fn(|slot| FactorAllocation {
                plan: Rc::clone(&self.0),
                stage: Stage::ALL[slot],
                ticket: PrivateAllocationTicket::new(self.0.execution, self.0.token, slot),
                bytes: self.0.bytes[slot].get(),
            }),
            backend: self.0.backend,
            inline,
            metadata,
        })
    }
}
impl Plan {
    /// The opaque backend request still has one checked scalar and storage representation.
    fn backend_kind(&self, stage: Stage) -> TypeId {
        if self.kind == TypeId::of::<f32>() {
            if stage == Stage::Factors {
                TypeId::of::<faer::linalg::solvers::PartialPivLu<f32>>()
            } else {
                TypeId::of::<faer::Mat<f32>>()
            }
        } else if stage == Stage::Factors {
            TypeId::of::<faer::linalg::solvers::PartialPivLu<f64>>()
        } else {
            TypeId::of::<faer::Mat<f64>>()
        }
    }
}

/// Aligned reference-counted allocation header and its actual plan payload.
#[repr(C)]
struct RcLayout {
    /// Strong reference counter.
    strong: usize,
    /// Weak reference counter.
    weak: usize,
    /// Aligned plan metadata.
    value: Plan,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
