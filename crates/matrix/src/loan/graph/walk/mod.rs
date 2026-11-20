// SPDX-License-Identifier: AGPL-3.0-only
//! Private bounded graph mechanics; numerical adapters supply no authority through this core.

mod admission;
mod bitmap;
mod control;
mod drive;
mod invalidation;
#[cfg(test)]
mod tests;
mod waiting;

use super::{Backing, ExecutionAccount, LoanError, Reservation};
use crate::loan::charge;

/// Null incidence/free/wait link; zero remains an ordinary slot.
const NONE: usize = usize::MAX;
/// Occupied core fixture node, without LC execution authority.
const LIVE: u64 = 1;
/// DFS gray colour, stored inline with node state.
const GRAY: u64 = 2;
/// DFS black colour, stored inline with node state.
const BLACK: u64 = 4;
/// The node currently has a ready-index entry.
const READY: u64 = 8;
/// Dispatch was permanently revoked for this occupation.
const INVALID: u64 = 16;
/// Active invalidation retains its own stack/generation between calls.
const INVALIDATING: u64 = 1;
/// A cumulative Work failure stops ordinary transitions until explicit cancellation.
const STOPPED: u64 = 2;
/// A bounded membership-watermark scan is active.
const SCANNING: u64 = 4;
/// Waiting membership is independent of ready-index presence.
const WAITING: u64 = 32;
/// A fixture dispatch has entered; it is not numerical publication.
const STARTED: u64 = 64;
/// A terminal core outcome has been preserved before release.
const TERMINAL: u64 = 128;
/// Attempt exhaustion is a terminal fixture outcome with prepaid uses still pending.
const ATTEMPTS: u64 = 256;

/// Admission creates a distinct handle type; O's inspection handles cannot authorize mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Handle {
    /// Admitted owner identity.
    owner: u64,
    /// Occupied node slot.
    slot: usize,
    /// Exact occupation generation.
    generation: u64,
}

/// A proposed concrete dependency; its kind never excludes it from cycle checking.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Connection {
    /// Proposed producer slot.
    pub(crate) producer: usize,
    /// Proposed consumer slot.
    pub(crate) consumer: usize,
    /// Original demand association, interpreted only by its admitted owner.
    pub(crate) demand: usize,
    /// Semantic and numerical dependencies share the same acyclicity test.
    pub(crate) semantic: bool,
}
/// Core admission does not admit an LC or numerical node contract.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    /// Bounded core fixture with no mathematical result authority.
    Fixture,
    /// No adapter can silently interpret an unknown contract.
    Unsupported,
}
/// Typed core failures keep resource stops distinct from topology and ownership refusals.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// Existing typed original-account refusal.
    #[error("core native refusal: {0}")]
    Native(LoanError),
    /// A semantic or numerical cycle exists in the private proposal.
    #[error("cyclic graph proposal")]
    Cycle,
    /// One invalidation already owns the saved traversal.
    #[error("graph control already in progress")]
    ControlInProgress,
    /// The original cumulative traversal allowance is exhausted.
    #[error("original traversal allowance exhausted")]
    TraversalExhausted,
    /// Original cumulative Work has stopped ordinary graph transitions.
    #[error("original Work allowance stopped the graph")]
    WorkStopped,
    /// The occupation has not reached the phase required by this operation.
    #[error("graph operation is invalid in this phase")]
    InvalidPhase,
}
impl From<LoanError> for Error {
    fn from(value: LoanError) -> Self {
        Self::Native(value)
    }
}
/// Reservation consumption either publishes a complete core or returns the original owner.
#[derive(Debug)]
pub(crate) enum Admission {
    /// Atomic topology/readiness publication succeeded.
    Admitted(Core),
    /// Inspection readers still protect the unchanged reservation.
    OwnershipWait(Reservation),
    /// No runnable graph was published; the caller still owns private storage for cancellation.
    Refused(Error, Reservation),
}
/// No public constructor or numerical access is provided by the walk increment.
#[derive(Debug)]
pub(crate) struct Core {
    /// O's allocation set stays resident; admission requires its exclusive ownership.
    reservation: Reservation,
}
impl Core {
    /// Obtain graph authority only after complete admission, never by converting an O handle.
    pub(crate) fn handle(
        &self,
        slot: usize,
        account: &mut ExecutionAccount,
    ) -> Result<Handle, Error> {
        account.authenticate(self.reservation.backing.account)?;
        account
            .ledger
            .with_work_phase("matrix.graph", "graph handle", |budget| {
                charge(budget, 2)?; // H operation entry + N occupied-slot inspection.
                let node = self
                    .reservation
                    .backing
                    .nodes
                    .get(slot)
                    .ok_or(LoanError::StaleOccupation)?;
                if node.flags & LIVE == 0 {
                    return Err(LoanError::StaleOccupation.into());
                }
                Ok(Handle {
                    owner: self.reservation.backing.identity,
                    slot,
                    generation: node.generation,
                })
            })
    }
}
/// Checked event summation never saturates a claimed prepayment.
fn add(left: usize, right: usize) -> Result<usize, LoanError> {
    super::inventory::add(left, right)
}
/// A saved cyclic cursor is a separate H transition after a committed visit.
const fn advance(backing: &mut Backing, slot: usize) {
    backing.control.ready = (slot + 1) % backing.nodes.len();
}
/// Ordinary driving never borrows another call's traversal allowance.
const fn enter(backing: &mut Backing) -> Result<(), Error> {
    if backing.drive == 0 {
        return Err(Error::TraversalExhausted);
    }
    backing.drive -= 1;
    Ok(())
}

/// A failed original Work debit freezes every ordinary entry point, including saved controls.
const fn record_stop<T>(backing: &mut Backing, result: &Result<T, Error>) {
    let stopped = matches!(result, Err(Error::Native(LoanError::Budget(fault) | LoanError::Mask(crate::MaskError::Budget(fault)))) if matches!(fault.kind, nrz_bindings::readings::algebra::FaultKind::Exhausted { budget: nrz_bindings::readings::algebra::limits::BudgetKind::Work, .. }));
    if stopped {
        backing.control.flags |= STOPPED;
    }
}
