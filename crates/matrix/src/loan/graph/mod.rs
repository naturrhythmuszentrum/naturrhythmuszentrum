// SPDX-License-Identifier: AGPL-3.0-only
//! Non-executable scoped storage precedes graph validation and publication.
//! Uncovered authority/backend and delivery rows never become a complete total.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the adapter increment wires the crate-private graph core"
    )
)]

mod inventory;
mod plan;
mod records;
mod reserve;
pub(in crate::loan) mod sink;
#[cfg(test)]
mod tests;
mod walk;

use super::{ExecutionAccount, LoanError, charge, registry::CleanupReceipt};
use crate::storage::Format;
pub(crate) use inventory::Inventory;
pub(crate) use plan::Arguments;
use records::{Control, Edge, Frame, Node, Scope};
use std::rc::Rc;

/// Sole construction owner; never an executable or published graph.
#[derive(Debug)]
pub(crate) struct Reservation {
    /// Shared only by authenticated inspection readers, not cloneable owner handles.
    backing: Rc<Backing>,
}
/// An outside inspector retains exactly the same covered allocation set.
#[derive(Debug)]
pub(crate) struct Reader {
    /// Last inspector keeps reservation storage resident after the constructor drops.
    backing: Rc<Backing>,
}
/// Borrowed enumeration of actual reservation storage, not caller-supplied byte claims.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Borrowed<'a>(&'a Backing);
/// Inspectable reservation slot; no constructor outside this module supplies authority.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Handle {
    /// Original reservation identity.
    owner: u64,
    /// Reserved node position.
    slot: usize,
    /// Nonwrapping occupation generation.
    generation: u64,
}
/// Read-only facts about a reserved node, not a mutable graph record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Slot {
    /// Initial original attempt envelope, not consumed by inspection.
    pub(crate) attempts: u64,
    /// Reserved occupation stamp.
    pub(crate) generation: u64,
}
/// Private buffers have no runnable publication; W alone will consume this representation.
#[derive(Debug)]
#[repr(C)]
struct Backing {
    /// Original ledger identity.
    account: u64,
    /// Reservation identity from the heap identity allocator.
    identity: u64,
    /// One original cumulative drive count, distinct from Work.
    drive: u64,
    /// Occupied proposal prefix; remaining node slots are reserved vacancies.
    proposed: usize,
    /// Format of the future native adapter, never inferred from byte counts.
    format: Format,
    /// Ready/wait/scope cursor and epoch storage, with no transition implementation.
    control: Control,
    /// Destruction was funded before any backing allocation.
    cleanup: CleanupReceipt,
    /// Full allocated node extent, including vacancies.
    nodes: Vec<Node>,
    /// Exactly the proposed edge extent; no spare edge growth.
    edges: Vec<Edge>,
    /// Every base and summary word, initially clear.
    ready: Vec<u64>,
    /// Bounded nonrecursive traversal frames, initially unused.
    stack: Vec<Frame>,
    /// Visitation generation words for all node positions.
    marks: Vec<u64>,
    /// Bounded notification-scope descriptors.
    scopes: Vec<Scope>,
    /// Packed original roots and closure membership for every reserved scope.
    membership: Vec<u64>,
}
impl Reservation {
    /// Enumerate a borrowed owner without exposing or reconstructing its storage.
    pub(crate) fn borrowed(&self) -> Borrowed<'_> {
        Borrowed(&self.backing)
    }
    /// Retain an outside inspection edge; the edge cleanup is paid before cloning.
    pub(crate) fn reader(&self, account: &mut ExecutionAccount) -> Result<Reader, LoanError> {
        account.authenticate(self.backing.account)?;
        account
            .ledger
            .with_work_phase("matrix.graph", "reservation reader", |budget| {
                // H entry + P retained-edge cleanup; the account check remains uncharged.
                charge(budget, 2)?;
                Ok(Reader {
                    backing: Rc::clone(&self.backing),
                })
            })
    }
    /// Inspect only a proposed reservation slot, without activation or numerical authority.
    pub(crate) fn handle(
        &self,
        slot: usize,
        account: &mut ExecutionAccount,
    ) -> Result<Handle, LoanError> {
        account.authenticate(self.backing.account)?;
        account
            .ledger
            .with_work_phase("matrix.graph", "reservation handle", |budget| {
                // H entry + N fixed slot inspection.
                charge(budget, 2)?;
                let node = self
                    .backing
                    .nodes
                    .get(slot)
                    .filter(|_| slot < self.backing.proposed)
                    .ok_or(LoanError::WrongLoan)?;
                Ok(Handle {
                    owner: self.backing.identity,
                    slot,
                    generation: node.generation,
                })
            })
    }
    /// Authentication failure leaves the owner and every original slot untouched.
    pub(crate) fn inspect(
        &self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<Slot, LoanError> {
        account.authenticate(self.backing.account)?;
        account
            .ledger
            .with_work_phase("matrix.graph", "reservation slot", |budget| {
                // H entry + A reservation identity, before touching a foreign slot.
                charge(budget, 2)?;
                if handle.owner != self.backing.identity {
                    return Err(LoanError::ForeignOwner);
                }
                // N: inspect the authenticated slot and its occupation generation.
                charge(budget, 1)?;
                let node = self
                    .backing
                    .nodes
                    .get(handle.slot)
                    .filter(|_| handle.slot < self.backing.proposed)
                    .ok_or(LoanError::StaleOccupation)?;
                if node.generation != handle.generation {
                    return Err(LoanError::StaleOccupation);
                }
                Ok(Slot {
                    attempts: node.attempts,
                    generation: node.generation,
                })
            })
    }
}
impl Reader {
    /// Include storage retained only by an outside reader in the scoped inventory.
    pub(crate) fn borrowed(&self) -> Borrowed<'_> {
        Borrowed(&self.backing)
    }
}
