// SPDX-License-Identifier: AGPL-3.0-only
//! Scope closure uses funded roots, membership and one serialized bounded traversal.

use super::super::{
    Backing,
    records::Frame,
    walk::{Core, Error, Handle},
};
use crate::loan::charge;
use crate::{ExecutionAccount, LoanError};
use nrz_bindings::Budget;

/// One scope traversal owns the shared stack until its complete closure is published.
const BUILDING_SCOPE: u64 = super::super::walk::SCOPING;
/// A missing incidence or traversal link, never an ordinary slot.
const NONE: usize = usize::MAX;

/// Scope identity cannot be reconstructed from a root or numerical ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scope {
    /// Exact admitted graph owner.
    owner: u64,
    /// Reserved descriptor position.
    index: usize,
    /// Nonwrapping notification generation.
    generation: u64,
}

/// Boundary stamps are part of the adapter's fixed per-node storage.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Boundary {
    /// Only the current serialized traversal reads these generation stamps.
    pub(super) generation: u64,
}

/// An entered step either expands the closure or finishes its publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Progress {
    /// Fixed atomic traversal work remains, with all cursors preserved.
    Pending,
    /// The fixed admitted closure is available for scoped notification.
    Published,
}

/// Authenticate and prepay the complete initiating operation before any mark or gate changes.
pub(super) fn begin(
    core: &mut Core,
    boundaries: &mut [Boundary],
    roots: &[Handle],
    owned: &[Handle],
    account: &mut ExecutionAccount,
) -> Result<Scope, Error> {
    account.authenticate(core.reservation.backing.account)?;
    let backing =
        std::rc::Rc::get_mut(&mut core.reservation.backing).ok_or(LoanError::WrongLoan)?;
    let result = account
        .ledger
        .with_work_phase("matrix.graph", "scope admission", |budget| {
            charge(budget, 1)?; // H original header, counters and control gate.
            if backing.control.flags & (1 | BUILDING_SCOPE) != 0 {
                return Err(Error::ControlInProgress);
            }
            if backing.control.flags & 2 != 0 {
                return Err(Error::WorkStopped);
            }
            let generation = backing.control.epochs[2]
                .checked_add(1)
                .ok_or(LoanError::CountOverflow)?;
            let visitation = backing.control.epochs[1]
                .checked_add(1)
                .ok_or(LoanError::CountOverflow)?;
            if boundaries.len() != backing.nodes.len() {
                return Err(LoanError::UnsupportedContract.into());
            }
            validate(backing, roots, owned, budget)?;
            let index = vacancy(backing, budget)?;
            if backing.drive == 0 {
                return Err(Error::TraversalExhausted);
            }
            let words = backing.nodes.len().div_ceil(64);
            let offset = index
                .checked_mul(words)
                .and_then(|n| n.checked_mul(2))
                .ok_or(LoanError::ByteOverflow)?;
            // H publication, M four scope words, each root-word update and each boundary stamp.
            let work = 5_usize
                .checked_add(roots.len())
                .and_then(|n| n.checked_add(owned.len()))
                .ok_or(LoanError::CountOverflow)?;
            charge(budget, work)?;
            backing.drive -= 1;
            backing.control.epochs[2] = generation;
            backing.control.epochs[1] = visitation;
            backing.control.flags |= BUILDING_SCOPE;
            backing.control.pending = [index, 0];
            backing.scopes[index] = super::super::records::Scope {
                generation,
                cursors: [0, 0],
                membership: offset,
            };
            for handle in roots {
                backing.membership[offset + handle.slot / 64] |= 1 << (handle.slot % 64);
            }
            for handle in owned {
                boundaries[handle.slot].generation = generation;
            }
            Ok(Scope {
                owner: backing.identity,
                index,
                generation,
            })
        });
    super::super::walk::record_stop(backing, &result);
    result
}

/// Roots must belong to the caller's explicit fixed ownership boundary.
fn validate(
    backing: &Backing,
    roots: &[Handle],
    owned: &[Handle],
    budget: &mut Budget,
) -> Result<(), Error> {
    for handle in roots.iter().chain(owned) {
        charge(budget, 1)?; // A exact original owner before indexing.
        if handle.owner != backing.identity {
            return Err(LoanError::ForeignOwner.into());
        }
        charge(budget, 1)?; // N original occupation.
        let node = backing
            .nodes
            .get(handle.slot)
            .filter(|_| handle.slot < backing.proposed)
            .ok_or(LoanError::StaleOccupation)?;
        if node.generation != handle.generation || node.flags & 1 == 0 {
            return Err(LoanError::StaleOccupation.into());
        }
    }
    for root in roots {
        let mut found = false;
        for member in owned {
            charge(budget, 1)?; // A recorded boundary membership comparison.
            if root == member {
                found = true;
                break;
            }
        }
        if !found {
            return Err(LoanError::UnsupportedContract.into());
        }
    }
    Ok(())
}

/// A scope is retained until its owner drops; no generation or historical membership is reused.
fn vacancy(backing: &Backing, budget: &mut Budget) -> Result<usize, Error> {
    for (index, scope) in backing.scopes.iter().enumerate() {
        charge(budget, 1)?; // M bounded scope descriptor inspection.
        if scope.generation == 0 {
            return Ok(index);
        }
    }
    Err(LoanError::UnsupportedContract.into())
}

/// A scope step never invokes a numerical operation or cancels an outstanding host loan.
pub(super) fn step(
    core: &mut Core,
    boundaries: &[Boundary],
    scope: Scope,
    account: &mut ExecutionAccount,
) -> Result<Progress, Error> {
    account.authenticate(core.reservation.backing.account)?;
    let backing =
        std::rc::Rc::get_mut(&mut core.reservation.backing).ok_or(LoanError::WrongLoan)?;
    let result = account
        .ledger
        .with_work_phase("matrix.graph", "scope closure", |budget| {
            authenticate(backing, scope, budget)?;
            if backing.control.flags & 2 != 0 {
                return Err(Error::WorkStopped);
            }
            if backing.scopes[scope.index].cursors[0] == NONE {
                return Ok(Progress::Published);
            }
            if backing.control.pending[0] != scope.index {
                return Err(Error::ControlInProgress);
            }
            if backing.drive == 0 {
                return Err(Error::TraversalExhausted);
            }
            charge(budget, 2)?; // H traversal entry and M saved depth.
            let depth = backing.scopes[scope.index].cursors[0];
            if depth == 0 {
                root(backing, scope.index, budget)
            } else {
                edge(backing, boundaries, scope.index, depth, budget)
            }
        });
    super::super::walk::record_stop(backing, &result);
    result
}

/// No foreign or stale scope reads another owner's descriptor.
fn authenticate(backing: &Backing, scope: Scope, budget: &mut Budget) -> Result<(), Error> {
    charge(budget, 2)?; // H entry, A owner.
    if backing.identity != scope.owner {
        return Err(LoanError::ForeignOwner.into());
    }
    charge(budget, 1)?; // M retained descriptor generation.
    if backing
        .scopes
        .get(scope.index)
        .is_none_or(|record| record.generation != scope.generation)
    {
        return Err(LoanError::StaleOccupation.into());
    }
    Ok(())
}

/// Stable root order and shared visitation stamps avoid re-enqueuing a dependency.
fn root(backing: &mut Backing, index: usize, budget: &mut Budget) -> Result<Progress, Error> {
    charge(budget, 2)?; // M root cursor and membership offset.
    let scope = backing.scopes[index];
    let cursor = scope.cursors[1];
    if cursor == backing.proposed {
        charge(budget, 2)?; // M completed depth and notification cursor; the header transition is prepaid.
        backing.drive -= 1;
        backing.scopes[index].cursors[0] = NONE;
        backing.scopes[index].cursors[1] = 0;
        backing.control.flags &= !BUILDING_SCOPE;
        backing.control.pending = [NONE; 2];
        return Ok(Progress::Published);
    }
    charge(budget, 2)?; // M root bitmap and visitation word.
    let present = backing.membership[scope.membership + cursor / 64] & (1 << (cursor % 64)) != 0;
    let fresh = backing.marks[cursor] != backing.control.epochs[1];
    if present && fresh {
        // N source incidence; X push; M mark and closure bit; M descriptor cursor.
        charge(budget, 6)?;
        backing.drive -= 1;
        push(backing, index, 0, cursor);
    } else {
        charge(budget, 1)?; // M descriptor cursor.
        backing.drive -= 1;
    }
    backing.scopes[index].cursors[1] += 1;
    Ok(Progress::Pending)
}

/// Each incoming edge either crosses the fixed boundary or admits one marked dependency.
fn edge(
    backing: &mut Backing,
    boundaries: &[Boundary],
    index: usize,
    depth: usize,
    budget: &mut Budget,
) -> Result<Progress, Error> {
    charge(budget, 1)?; // N current traversal node and its saved frame cursor.
    let frame = backing.stack[depth - 1];
    if frame.edge == NONE {
        charge(budget, 2)?; // X pop and M descriptor depth.
        backing.drive -= 1;
        backing.scopes[index].cursors[0] -= 1;
        return Ok(Progress::Pending);
    }
    charge(budget, 6)?; // E incidence, N producer; M boundary, visitation, scope generation and offset.
    let edge = backing.edges[frame.edge];
    let producer = edge.endpoints[0];
    if backing.nodes[producer.slot].generation != producer.generation {
        return Err(LoanError::StaleOccupation.into());
    }
    let owned = boundaries[producer.slot].generation == backing.scopes[index].generation;
    let fresh = backing.marks[producer.slot] != backing.control.epochs[1];
    charge(budget, if owned && fresh { 5 } else { 0 })?; // Optional complete push; the source cursor visit is prepaid.
    backing.drive -= 1;
    backing.stack[depth - 1].edge = edge.links[3];
    if owned && fresh {
        push(backing, index, depth, producer.slot);
    }
    Ok(Progress::Pending)
}

/// Every field was funded before entry; the admitted acyclic closure fits the reserved stack.
fn push(backing: &mut Backing, index: usize, depth: usize, node: usize) {
    let offset = backing.scopes[index].membership + backing.nodes.len().div_ceil(64);
    backing.marks[node] = backing.control.epochs[1];
    backing.membership[offset + node / 64] |= 1 << (node % 64);
    backing.stack[depth] = Frame {
        node,
        edge: backing.nodes[node].links[1],
    };
    backing.scopes[index].cursors[0] = depth + 1;
}

/// Inspect exactly one saved notification position without extending its original closure.
pub(super) fn next(
    core: &Core,
    scope: Scope,
    account: &mut ExecutionAccount,
) -> Result<Option<(usize, bool)>, Error> {
    account.authenticate(core.reservation.backing.account)?;
    let backing = &core.reservation.backing;
    authenticate(backing, scope, &mut account.ledger)?;
    charge(&mut account.ledger, 3)?; // M completed depth, saved cursor and closure membership.
    let descriptor = backing.scopes[scope.index];
    if descriptor.cursors[0] != NONE {
        return Err(Error::ControlInProgress);
    }
    let slot = descriptor.cursors[1];
    if slot == backing.proposed {
        return Ok(None);
    }
    let offset = descriptor.membership + backing.nodes.len().div_ceil(64);
    Ok(Some((
        slot,
        backing.membership[offset + slot / 64] & (1 << (slot % 64)) != 0,
    )))
}

/// Prepay the saved cursor and traversal transition before any expectation is closed.
pub(super) fn prepay_next(core: &Core, account: &mut ExecutionAccount) -> Result<(), Error> {
    if core.reservation.backing.control.flags & 2 != 0 {
        return Err(Error::WorkStopped);
    }
    if core.reservation.backing.control.flags & (1 | BUILDING_SCOPE) != 0 {
        return Err(Error::ControlInProgress);
    }
    if core.reservation.backing.drive == 0 {
        return Err(Error::TraversalExhausted);
    }
    charge(&mut account.ledger, 2)?; // H finite traversal and M saved scope cursor.
    Ok(())
}

/// The exclusive adapter operation holds the original authenticated cursor until this commit.
pub(super) fn commit_next(core: &mut Core, scope: Scope) -> Result<(), Error> {
    let backing =
        std::rc::Rc::get_mut(&mut core.reservation.backing).ok_or(LoanError::WrongLoan)?;
    backing.drive -= 1;
    backing.scopes[scope.index].cursors[1] += 1;
    Ok(())
}

#[cfg(test)]
mod tests;
