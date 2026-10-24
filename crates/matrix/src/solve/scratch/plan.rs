// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable meanings and mutable permit states are shared by actual numerical owners.

use super::Stage;
use crate::{Dependency, Extent, OperationError, PrivateAllocationError as Error};
use nrz_bindings::BindingSymbols;
use nrz_scalar::Precision;
use std::{any::TypeId, cell::Cell, rc::Rc};

/// One planned allocation can be issued only once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum State {
    /// Complete declaration exists before the allocator is entered.
    Planned,
    /// The typed permit has been drawn.
    Issued,
    /// The original buffer remains live.
    Live,
    /// Its complete numerical owner moved into the candidate.
    Candidate,
    /// Its owner was dropped, recording the initialized length.
    Released(usize),
}
/// Closed stage, shape and observed lifecycle of one finite array request.
#[derive(Debug)]
pub(super) struct Specification {
    /// Closed mathematical role for this ordered rectangle.
    pub(super) stage: Stage,
    /// Initial/correction attempt, or absence for final candidate transfer.
    pub(super) iteration: Option<usize>,
    /// Complete planned logical rectangle.
    pub(super) extent: Extent,
    /// Finite single-issuance lifecycle under the exclusive lease.
    pub(super) state: Cell<State>,
    /// Observed owned capacity; backend storage stays opaque.
    pub(super) bytes: Cell<Option<usize>>,
}
/// Exact input bindings and the full deterministic schedule precede all RHS allocations.
#[derive(Debug)]
pub(crate) struct Plan {
    /// Original validated numerical execution.
    pub(super) trace: crate::execution::Trace,
    /// Unique candidate attempt identity independent of storage addresses.
    pub(super) token: u64,
    /// Shared original checked solve equation, axes and exact restoration.
    pub(super) contract: Rc<super::super::SolveContract>,
    /// Original checked differential equation retained before any sensitivity arrays.
    pub(super) derivative: Option<Rc<nrz_bindings::CheckedSensitivity>>,
    /// Concrete enclosure representation with two endpoints per logical coordinate.
    pub(super) enclosure: TypeId,
    /// Original source model, exact selection and statistical law for the finite contractions.
    pub(super) statistics: Option<Box<super::statistics::Binding>>,
    /// Selected source versions and their original value/companion declarations.
    pub(super) sources: Vec<(Dependency, BindingSymbols)>,
    /// Predeclared final result and companion symbols.
    pub(super) output: BindingSymbols,
    /// Full finite allocation schedule, including an optional untouched recovery suffix.
    pub(super) specifications: Vec<Specification>,
    /// Concrete Rust scalar representation fixed before allocation.
    pub(super) kind: TypeId,
    /// Declared numerical format.
    pub(super) precision: Precision,
    /// Owned bytes per scalar component.
    pub(super) width: usize,
    /// Largest simultaneous observable vector capacity.
    pub(super) peak: Cell<usize>,
    /// Length of the independently certified contiguous attempt prefix.
    pub(super) completed: Cell<usize>,
    /// Admitted owned plan and receipt bytes, excluding shared exact declarations.
    pub(super) metadata: usize,
}
/// Non-cloneable authority stays attached to the buffer that consumed it.
#[derive(Debug)]
pub(crate) struct Permit {
    /// Original immutable meaning and private lifecycle state.
    pub(super) plan: Rc<Plan>,
    /// Exact finite stage consumed by this non-cloneable permit.
    pub(super) slot: usize,
}
impl Permit {
    /// Preserve original schedule identity when publication inspects a numerical owner.
    pub(crate) const fn plan(&self) -> &Rc<Plan> {
        &self.plan
    }
    /// Slot identity is checked together with the shared original plan, never a numerical address.
    pub(crate) fn matches(&self, plan: &Rc<Plan>, slot: usize) -> bool {
        Rc::ptr_eq(&self.plan, plan) && self.slot == slot
    }

    /// The finite stage fixes both the initialized length and the maximum vector capacity request.
    pub(crate) fn entries(&self) -> usize {
        self.plan.specifications[self.slot].extent.entries()
    }
    /// Track observed capacity only after the pre-allocation ticket was drawn.
    pub(crate) fn allocated(&self, bytes: usize) -> Result<(), OperationError> {
        let entry = &self.plan.specifications[self.slot];
        if entry.state.get() != State::Issued {
            return Err(Error::DuplicateTicket.into());
        }
        let live = self
            .plan
            .specifications
            .iter()
            .try_fold(bytes, |sum, item| {
                if matches!(item.state.get(), State::Live | State::Candidate) {
                    sum.checked_add(item.bytes.get().unwrap_or(0))
                        .ok_or(OperationError::ResourceLimit)
                } else {
                    Ok(sum)
                }
            })?;
        self.plan.trace.allocated(bytes)?;
        entry.bytes.set(Some(bytes));
        entry.state.set(State::Live);
        self.plan.peak.set(self.plan.peak.get().max(live));
        Ok(())
    }
    /// Only fully initialized output channels can change into candidate ownership.
    pub(crate) fn candidate(&self, len: usize) -> Result<(), OperationError> {
        let entry = &self.plan.specifications[self.slot];
        if !entry.stage.is_candidate_at(entry.iteration)
            || len != self.entries()
            || entry.state.get() != State::Live
        {
            return Err(Error::IncompleteCoverage.into());
        }
        entry.state.set(State::Candidate);
        Ok(())
    }
    /// Record destruction without moving or cloning the authority away from its owner.
    pub(crate) fn release(&self, len: usize) {
        let entry = &self.plan.specifications[self.slot];
        if matches!(entry.state.get(), State::Live | State::Candidate) {
            self.plan.trace.released(entry.bytes.get().unwrap_or(0));
        }
        entry.state.set(State::Released(len));
    }
}
/// Actual aligned reference-counted allocation header, excluding shared exact declarations.
#[repr(C)]
pub(super) struct RcLayout {
    /// Strong reference counter.
    pub(super) strong: usize,
    /// Weak reference counter.
    pub(super) weak: usize,
    /// Aligned shared plan payload.
    pub(super) value: Plan,
}
