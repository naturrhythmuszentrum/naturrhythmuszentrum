// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable finite requests retain their original numerical authority through owner destruction.

use super::{Channel, Role};
use crate::{Extent, OperationError, PrivateAllocationError as Error};
use nrz_scalar::Precision;
use std::{cell::Cell, rc::Rc};

/// A finite allocation cannot be issued twice or silently detached from its owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum State {
    /// Complete meaning exists before the allocator is entered.
    Planned,
    /// Typed allocation authority was drawn exactly once.
    Issued,
    /// The covered numerical owner remains live.
    Live,
    /// The final covered owner belongs to the unpublished candidate.
    Candidate,
    /// Owner destruction retained its initialized length.
    Released(usize),
}
/// One original stage and its observed numerical lifetime.
#[derive(Debug)]
pub(super) struct Specification {
    /// Exact source, local law or publication role.
    pub(super) role: Role,
    /// Original operand, output or ordered cross side.
    pub(super) index: usize,
    /// Candidate, enclosure, error or opaque backend representation.
    pub(super) channel: Channel,
    /// Complete logical rectangle fixed before allocation.
    pub(super) extent: Extent,
    /// Required scalar representation, including distinct cross-input formats.
    pub(super) precision: Precision,
    /// A checked conditional branch can leave this request unissued.
    pub(super) optional: bool,
    /// Non-cloneable permit lifecycle.
    pub(super) state: Cell<State>,
    /// Actual vector capacity; opaque backend storage remains unmeasured.
    pub(super) bytes: Cell<Option<usize>>,
}
/// Original model, exact selected targets and finite schedule shared by every actual owner.
#[derive(Debug)]
pub(crate) struct Plan {
    /// Original exact operand definitions, shared with captured numerical inputs.
    pub(super) bindings: Vec<crate::statistics::capture::InputBinding>,
    /// Original checked operation; absence denotes the closed ordered cross-covariance law.
    pub(super) operation: Option<crate::CheckedOperation>,
    /// Exact source order of the common model.
    pub(super) columns: Vec<usize>,
    /// Finite requests determined from original metadata and the closed algorithm.
    pub(super) specifications: Vec<Specification>,
    /// Common execution counter shared with other covered allocating scopes.
    pub(super) trace: crate::execution::Trace,
    /// Unique inventory identity, independent of numerical addresses.
    pub(super) token: u64,
    /// Exact successful candidate association, established before publication.
    pub(super) candidate: Cell<Option<u64>>,
    /// Actual currently live vector capacity in this scoped schedule.
    pub(super) live: Cell<usize>,
    /// Common scoped lifetime peak, never a sum of matrix peaks.
    pub(super) peak: Cell<usize>,
    /// Admitted aligned metadata, including selection copies and historical projections.
    pub(super) metadata: usize,
    /// Named algorithm admission bound for opaque backend storage and workspaces.
    pub(super) backend: Cell<usize>,
    /// Final admitted affine or linearized law after the original numerical classification checks.
    pub(super) assurance: Cell<crate::MomentAssurance>,
}
/// Non-cloneable authority stays attached to the buffer or backend owner it admitted.
#[derive(Debug)]
pub(crate) struct Permit {
    /// Original allocation definition and exact model binding.
    pub(super) plan: Rc<Plan>,
    /// One finite request in original schedule order.
    pub(super) slot: usize,
}
impl Permit {
    /// Complete initialized logical length required of this owner.
    pub(crate) fn entries(&self) -> usize {
        self.plan.specifications[self.slot].extent.entries()
    }
    /// Actual vector capacity joins both the scoped and execution-wide lifetime counters.
    pub(crate) fn allocated(&self, bytes: usize) -> Result<(), OperationError> {
        let entry = &self.plan.specifications[self.slot];
        if entry.state.get() != State::Issued {
            return Err(Error::DuplicateTicket.into());
        }
        let live = self
            .plan
            .live
            .get()
            .checked_add(bytes)
            .ok_or(OperationError::ResourceLimit)?;
        self.plan.trace.allocated(bytes)?;
        self.plan.live.set(live);
        self.plan.peak.set(self.plan.peak.get().max(live));
        entry.bytes.set(Some(bytes));
        entry.state.set(State::Live);
        Ok(())
    }
    /// A complete final numerical owner may enter a candidate without copying its permit.
    pub(crate) fn candidate(&self, len: usize) -> Result<(), OperationError> {
        let entry = &self.plan.specifications[self.slot];
        if len != self.entries() || entry.state.get() != State::Live {
            return Err(Error::IncompleteCoverage.into());
        }
        entry.state.set(State::Candidate);
        Ok(())
    }
    /// Release is reported only for an actual successful owned allocation.
    pub(crate) fn release(&self, len: usize) {
        let entry = &self.plan.specifications[self.slot];
        if matches!(entry.state.get(), State::Live | State::Candidate)
            && let Some(bytes) = entry.bytes.get()
        {
            self.plan.trace.released(bytes);
            self.plan.live.set(self.plan.live.get() - bytes);
        }
        entry.state.set(State::Released(len));
    }
    /// Numerical equality cannot replace the original allocating plan and request.
    pub(crate) fn matches(&self, plan: &Rc<Plan>, slot: usize) -> bool {
        Rc::ptr_eq(&self.plan, plan) && self.slot == slot
    }
    /// Publication may borrow the original plan, but cannot clone allocation authority.
    pub(crate) const fn plan(&self) -> &Rc<Plan> {
        &self.plan
    }
}
/// Aligned reference-count allocation used for exact metadata admission.
#[repr(C)]
pub(super) struct RcLayout<T> {
    /// Strong reference counter.
    pub(super) strong: usize,
    /// Weak reference counter.
    pub(super) weak: usize,
    /// Original retained owner.
    pub(super) value: T,
}
