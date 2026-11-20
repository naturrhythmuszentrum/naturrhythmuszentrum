// SPDX-License-Identifier: AGPL-3.0-only
//! Admitted in-process evidence, separate from numerical and adapter authority.

mod admission;
mod codec;
mod delivery;
#[cfg(test)]
pub(in crate::loan) mod tests;
mod witness;

use crate::loan::{ExecutionAccount, LoanError, charge};
use codec::{Encoding, Position};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Closed sink stops retain the native capability at their consuming boundary.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// Original account or storage refusal.
    #[error("sink native refusal: {0}")]
    Native(#[from] LoanError),
    /// No unused destination or witness slot is available.
    #[error("sink destination unavailable")]
    Full,
    /// Retained evidence has no owning bound.
    #[error("sink evidence bound is unproved")]
    Unbounded,
    /// The actual encoded evidence exceeds its admitted destination.
    #[error("sink evidence exceeds its admitted length")]
    Overlength,
    /// An immutable pull still borrows the evidence backing.
    #[error("sink evidence is borrowed")]
    Borrowed,
}
/// Completion is explicit; abandonment can never masquerade as delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub(crate) enum Status {
    /// Required destinations remain outstanding.
    Incomplete,
    /// Every admitted required destination was acknowledged.
    Complete,
    /// The graph owner was dropped with obligations outstanding.
    Abandoned,
    /// A proven bound was violated; no partial record was installed.
    Overlength,
}
/// Authority names one original owner, position and nonwrapping occupation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Handle {
    /// Original sink identity.
    owner: u64,
    /// Admitted destination.
    slot: usize,
    /// Exact current occupation.
    generation: u64,
}
/// Sole mutable graph-side delivery owner; never a numerical capability.
#[derive(Debug)]
pub(crate) struct Sink {
    /// Witnesses share only status and evidence, without a graph backedge.
    backing: Rc<Backing>,
}
/// Fixed destination metadata is distinct from its evidence-word backing.
#[derive(Debug)]
#[repr(C)]
struct Slot {
    /// Fixed admitted offset into the evidence words.
    offset: usize,
    /// Proven maximum encoded byte length.
    bound: usize,
    /// Closed admitted codec.
    encoding: Encoding,
    /// Exact original position, retained without a native owner edge.
    position: Position,
    /// Never wraps; exhausted slots remain retired.
    generation: Cell<u64>,
    /// Vacant, outstanding, acknowledged or retired.
    state: Cell<u64>,
    /// Actual encoded bytes, excluding final zero padding.
    length: Cell<usize>,
    /// Separately admitted outside evidence references.
    witnesses: Cell<usize>,
}
/// Shared storage includes all headers, fixed status and witness-slot capacity.
#[derive(Debug)]
#[repr(C)]
struct Backing {
    /// Original ledger identity.
    account: u64,
    /// Globally unique nonwrapping sink owner.
    identity: u64,
    /// Independently readable status survives graph abandonment.
    status: Cell<Status>,
    /// Admitted obligations not yet acknowledged, including unfilled destinations.
    outstanding: Cell<usize>,
    /// Complete record capacity, including unused caller slots.
    slots: Vec<Slot>,
    /// Full caller-supplied evidence capacity, never grown during delivery.
    words: RefCell<Vec<u64>>,
    /// Fixed witness occupancy; dropping an edge never needs a mutable payload borrow.
    witnesses: Vec<Cell<u64>>,
    /// Final field releases the account debit after payload storage is destroyed.
    debit: Debit,
}
/// One shared account registration owns a checked byte sum, not payload or graph edges.
#[derive(Debug)]
#[repr(C)]
struct Debit {
    /// Same registration retained by the original account and surviving evidence owner.
    registration: Rc<Cell<usize>>,
    /// Exact allocation inventory released only by final storage destruction.
    bytes: usize,
}
impl Drop for Debit {
    fn drop(&mut self) {
        // P funded this final record update and retaining-edge release before acquisition.
        self.registration.set(self.registration.get() - self.bytes);
    }
}
impl Drop for Sink {
    fn drop(&mut self) {
        // P funded this fixed status update; no mutable evidence borrow or ledger is needed.
        self.backing.status.set(Status::Abandoned);
    }
}
/// Same-account registration capacity remains visible even after the last evidence owner.
pub(super) fn live_bytes(account: &ExecutionAccount) -> Result<usize, LoanError> {
    account.delivery.as_ref().map_or(Ok(0), |registration| {
        super::inventory::add(crate::loan::rc_bytes::<Cell<usize>>(), registration.get())
    })
}
