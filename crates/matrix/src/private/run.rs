// SPDX-License-Identifier: AGPL-3.0-only
//! One immutable finite plan authorizes every typed allocation before any float is allocated.

use super::{
    PrivateAllocation, PrivateAllocationError as Error, PrivateAllocationTicket,
    ReciprocalInventory,
};
use crate::{ExecutionId, OperationError};
use nrz_bindings::readings::{PositiveReciprocalMap, ReciprocalAllocation, ReciprocalStage};
use nrz_scalar::{DecodedLiteral, Precision, Scalar};
use std::{any::TypeId, cell::Cell, rc::Rc};

/// Private allocation state; the typed permit remains attached to the actual buffer owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum State {
    /// Prepared exact binding without an allocation ticket.
    Planned,
    /// A ticket was drawn before entering the allocator.
    Issued,
    /// A typed buffer still owns its allocation.
    Live,
    /// Ownership and its typed permit moved together into a candidate.
    Candidate,
    /// A buffer was released with the recorded initialized entry count.
    Released(usize),
}

/// Exact binding and fixed representation, shared by the owner and its allocation permits.
#[derive(Debug)]
pub(super) struct Specification {
    /// Complete bindings-owned entry/component interpretation.
    pub(super) binding: ReciprocalAllocation,
    /// Sealed concrete Rust representation of one logical element.
    kind: TypeId,
    /// Owned element size, including alignment and auxiliary fields.
    width: usize,
    /// Complete floating component count, checked before allocation.
    floats: usize,
    /// Current ticket lifecycle under the exclusive execution lease.
    pub(super) state: Cell<State>,
    /// Actual numerical capacity recorded after fallible allocation.
    pub(super) bytes: Cell<usize>,
}

/// Metadata is allocated once; buffer owners share its immutable exact declarations.
#[derive(Debug)]
pub(super) struct Plan {
    /// Unique originating execution.
    pub(super) trace: crate::execution::Trace,
    /// Unique attempt token; failed retries cannot reuse allocation tickets.
    candidate: u64,
    /// One complete closed schedule in declaration order.
    pub(super) specifications: [Specification; 8],
    /// Execution scalar format.
    precision: Precision,
    /// Largest simultaneous numerical capacity, excluding plan and receipt metadata.
    peak: Cell<usize>,
}

/// One candidate attempt owns the lifecycle of the transferred arrays.
#[derive(Debug)]
pub(crate) struct Run(pub(super) Rc<Plan>);

/// A non-cloneable typed permit is the only route to allocating a covered buffer.
#[derive(Debug)]
pub(super) struct Permit<T> {
    /// Shared complete plan, prepared before this ticket was drawn.
    pub(super) plan: Rc<Plan>,
    /// Finite stage identity; the shared plan retains the original execution and candidate token.
    pub(super) slot: usize,
    /// Exact initialized entry universe sealed when this typed permit is drawn.
    pub(super) entries: usize,
    /// Representation checked when the ticket was drawn.
    marker: std::marker::PhantomData<T>,
}

impl Run {
    /// Prepare every exact meaning and concrete element size before the numerical schedule.
    pub(crate) fn new<P: Scalar + 'static>(
        trace: &crate::execution::Trace,
        candidate: u64,
        definition: &PositiveReciprocalMap,
    ) -> Result<Self, OperationError> {
        use ReciprocalStage as Stage;
        let specifications = [
            specification::<P>(definition, Stage::References)?,
            specification::<P>(definition, Stage::Coefficients)?,
            specification::<P>(definition, Stage::Offsets)?,
            specification::<P>(definition, Stage::Images)?,
            specification::<P>(definition, Stage::Contracted)?,
            specification::<P>(definition, Stage::Centers)?,
            specification::<P>(definition, Stage::Totals)?,
            specification::<P>(definition, Stage::Errors)?,
        ];
        Ok(Self(Rc::new(Plan {
            trace: trace.clone(),
            candidate,
            specifications,
            precision: P::PRECISION,
            peak: Cell::new(0),
        })))
    }

    /// Draw the stage's ticket, then allocate only the predeclared concrete representation.
    pub(crate) fn buffer<T: 'static>(
        &self,
        stage: ReciprocalStage,
    ) -> Result<super::Buffer<T>, OperationError> {
        let binding = self
            .0
            .specifications
            .iter()
            .find(|entry| entry.binding.stage() == stage)
            .ok_or(Error::UnplannedMeaning)?
            .binding
            .clone();
        super::Buffer::allocate(self.grant(&binding)?)
    }

    /// Exact identity and representation checks finish before the allocator is entered.
    fn grant<T: 'static>(
        &self,
        binding: &ReciprocalAllocation,
    ) -> Result<Permit<T>, OperationError> {
        let slot = self
            .0
            .specifications
            .iter()
            .position(|entry| entry.binding.same_binding(binding))
            .ok_or(Error::UnplannedMeaning)?;
        let entry = self
            .0
            .specifications
            .get(slot)
            .ok_or(Error::UnplannedMeaning)?;
        if entry.kind != TypeId::of::<T>() || entry.width != size_of::<T>() {
            return Err(Error::Representation.into());
        }
        if entry.state.get() != State::Planned {
            return Err(Error::DuplicateTicket.into());
        }
        entry.state.set(State::Issued);
        Ok(Permit {
            plan: Rc::clone(&self.0),
            slot,
            entries: entry.binding.entries(),
            marker: std::marker::PhantomData,
        })
    }

    /// Publication independently matches exact identity, every completed stage and live payload owner.
    pub(crate) fn validate<P: Scalar>(
        &self,
        execution: ExecutionId,
        candidate: u64,
        definition: &PositiveReciprocalMap,
        payload: &crate::publication::Payload<P>,
    ) -> Result<(), OperationError> {
        let expected = definition.allocation(ReciprocalStage::References);
        if self.0.trace.execution() != execution
            || self.0.candidate != candidate
            || self.0.precision != P::PRECISION
            || !self.0.specifications[0].binding.same_binding(&expected)
        {
            return Err(Error::ForeignInventory.into());
        }
        for (slot, entry) in self.0.specifications.iter().enumerate() {
            let values = match entry.binding.stage() {
                ReciprocalStage::Centers => Some(&payload.centers),
                ReciprocalStage::Totals => payload.bounds.as_ref(),
                ReciprocalStage::Errors => payload.evaluation.as_ref(),
                _ => {
                    if entry.state.get() != State::Released(entry.binding.entries()) {
                        return Err(Error::IncompleteCoverage.into());
                    }
                    continue;
                }
            }
            .ok_or(Error::IncompleteCoverage)?;
            if values.len() != entry.binding.entries()
                || entry.state.get() != State::Candidate
                || !values.matches(&self.0, slot)
            {
                return Err(Error::UnticketedBuffer.into());
            }
        }
        Ok(())
    }

    /// Snapshot receipts after live validation; candidate owners drop before the no-fail commit.
    pub(crate) fn completed(&self, candidate_bytes: usize) -> Box<ReciprocalInventory> {
        Box::new(ReciprocalInventory {
            allocations: std::array::from_fn(|slot| {
                let entry = &self.0.specifications[slot];
                PrivateAllocation {
                    binding: entry.binding.clone(),
                    ticket: PrivateAllocationTicket {
                        execution: self.0.trace.execution(),
                        candidate: self.0.candidate,
                        slot,
                    },
                    floats: entry.floats,
                    bytes: entry.bytes.get(),
                }
            }),
            precision: self.0.precision,
            peak: self.0.peak.get(),
            metadata: size_of::<ReciprocalInventory>(),
            planning: size_of::<RcLayout>(),
            candidate: candidate_bytes,
        })
    }
}

/// Reuse the checked exact declaration and preflight all representation sizes.
fn specification<P: Scalar + 'static>(
    definition: &PositiveReciprocalMap,
    stage: ReciprocalStage,
) -> Result<Specification, OperationError> {
    let (kind, width) = representation::<P>(stage);
    let binding = definition.allocation(stage);
    let floats = binding
        .entries()
        .checked_mul(stage.components().len())
        .ok_or(OperationError::ResourceLimit)?;
    binding
        .entries()
        .checked_mul(width)
        .ok_or(OperationError::ResourceLimit)?;
    Ok(Specification {
        binding,
        kind,
        width,
        floats,
        state: Cell::new(State::Planned),
        bytes: Cell::new(0),
    })
}

/// Match the reference-counted allocation header and payload alignment for byte accounting.
#[repr(C)]
struct RcLayout {
    /// Strong reference counter in the shared allocation header.
    strong: usize,
    /// Weak reference counter in the shared allocation header.
    weak: usize,
    /// Aligned immutable plan and private lifecycle cells.
    value: Plan,
}

/// Each closed stage has one fixed Rust representation; equal byte lengths cannot substitute it.
const fn representation<P: Scalar + 'static>(stage: ReciprocalStage) -> (TypeId, usize) {
    match stage {
        ReciprocalStage::References | ReciprocalStage::Coefficients | ReciprocalStage::Offsets => (
            TypeId::of::<DecodedLiteral<P>>(),
            size_of::<DecodedLiteral<P>>(),
        ),
        ReciprocalStage::Images => (
            TypeId::of::<crate::kernels::reciprocal::Image<P>>(),
            size_of::<crate::kernels::reciprocal::Image<P>>(),
        ),
        ReciprocalStage::Contracted => (
            TypeId::of::<crate::kernels::arithmetic::Entry<P>>(),
            size_of::<crate::kernels::arithmetic::Entry<P>>(),
        ),
        ReciprocalStage::Centers | ReciprocalStage::Totals | ReciprocalStage::Errors => {
            (TypeId::of::<P>(), size_of::<P>())
        }
    }
}

impl Plan {
    /// Register actual capacity after allocation while preserving the ticket drawn beforehand.
    pub(super) fn allocated(&self, slot: usize, bytes: usize) -> Result<(), OperationError> {
        let entry = self
            .specifications
            .get(slot)
            .ok_or(Error::UnplannedMeaning)?;
        if entry.state.get() != State::Issued {
            return Err(Error::DuplicateTicket.into());
        }
        let mut live = bytes;
        for other in &self.specifications {
            if matches!(other.state.get(), State::Live | State::Candidate) {
                live = live
                    .checked_add(other.bytes.get())
                    .ok_or(OperationError::ResourceLimit)?;
            }
        }
        self.trace.allocated(bytes)?;
        entry.bytes.set(bytes);
        entry.state.set(State::Live);
        self.peak.set(self.peak.get().max(live));
        Ok(())
    }
}

impl Run {
    /// An untracked candidate cannot publish under the reciprocal recipe, even with valid numbers.
    pub(crate) fn publication<P: Scalar, Q: Scalar>(
        bound: &crate::Bound<'_, P, Q>,
        candidate: &crate::Candidate<Q>,
    ) -> Result<Option<Box<ReciprocalInventory>>, OperationError> {
        match (bound.operation.reciprocal_map(), &candidate.private) {
            (Some(definition), Some(run)) => {
                let payload = candidate.outputs.first().ok_or(Error::IncompleteCoverage)?;
                run.validate(bound.execution, candidate.token, definition, payload)?;
                let bytes = candidate
                    .outputs
                    .capacity()
                    .checked_mul(size_of::<crate::publication::Payload<Q>>())
                    .ok_or(OperationError::ResourceLimit)?;
                Ok(Some(run.completed(bytes)))
            }
            (Some(_), None) => Err(Error::IncompleteCoverage.into()),
            (_, Some(_)) => Err(Error::ForeignInventory.into()),
            (_, None) => Ok(None),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
