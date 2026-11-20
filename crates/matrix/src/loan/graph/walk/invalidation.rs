// SPDX-License-Identifier: AGPL-3.0-only
//! Revocation preserves outcomes, attempts and uses; one saved traversal owns the gate.

use super::{Core, Error, Handle, INVALID, INVALIDATING, LIVE, NONE, READY, bitmap, enter};
use crate::loan::graph::{Backing, records::Frame};
use crate::loan::{ExecutionAccount, LoanError, charge};
use nrz_bindings::Budget;

impl Core {
    /// Refuse a second start without replacing the existing stack, marks or generation.
    pub(crate) fn invalidate(
        &mut self,
        root: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result =
            account
                .ledger
                .with_work_phase("matrix.graph", "invalidation start", |budget| {
                    charge(budget, 1)?; // H operation entry, including a busy refusal.
                    if backing.control.flags & super::STOPPED != 0 {
                        return Err(Error::WorkStopped);
                    }
                    if backing.control.flags & INVALIDATING != 0 {
                        return Err(Error::ControlInProgress);
                    }
                    charge(budget, 1)?; // A standalone admitted-owner comparison.
                    if root.owner != backing.identity {
                        return Err(LoanError::ForeignOwner.into());
                    }
                    charge(budget, 1)?; // N root generation/state inspection and prepared revocation.
                    let node = backing
                        .nodes
                        .get(root.slot)
                        .ok_or(LoanError::StaleOccupation)?;
                    let live = node.flags & LIVE != 0;
                    let same = node.generation == root.generation;
                    if !live || !same {
                        return Err(LoanError::StaleOccupation.into());
                    }
                    let generation = backing.control.epochs[1]
                        .checked_add(1)
                        .ok_or(LoanError::CountOverflow)?;
                    if backing.drive == 0 {
                        return Err(Error::TraversalExhausted);
                    }
                    let clear =
                        node.flags & READY != 0 && !super::control::is_control(backing, root.slot);
                    // H epoch/gate save + M visitation word + X push + every changed ready-path word.
                    charge(
                        budget,
                        3 + if clear {
                            bitmap::height(backing.nodes.len())
                        } else {
                            0
                        },
                    )?;
                    enter(backing)?;
                    backing.control.epochs[1] = generation;
                    backing.control.flags |= INVALIDATING;
                    backing.control.pending = [root.slot, 1];
                    backing.marks[root.slot] = generation;
                    backing.nodes[root.slot].flags |= INVALID;
                    backing.stack[0] = Frame {
                        node: root.slot,
                        edge: backing.nodes[root.slot].links[0],
                    };
                    clear_ready(backing, root.slot, clear);
                    Ok(())
                });
        super::record_stop(backing, &result);
        result
    }
    /// A bounded slice resumes the already allocated visitation generation; it never replaces it.
    pub(crate) fn resume_invalidation(
        &mut self,
        slice: usize,
        account: &mut ExecutionAccount,
    ) -> Result<bool, Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result =
            account
                .ledger
                .with_work_phase("matrix.graph", "invalidation resume", |budget| {
                    charge(budget, 1)?; // H operation entry.
                    if backing.control.flags & super::STOPPED != 0 {
                        return Err(Error::WorkStopped);
                    }
                    for _ in 0..slice {
                        if backing.control.flags & INVALIDATING == 0 {
                            return Ok(true);
                        }
                        if backing.drive == 0 {
                            return Err(Error::TraversalExhausted);
                        }
                        step(backing, budget)?;
                    }
                    Ok(backing.control.flags & INVALIDATING == 0)
                });
        super::record_stop(backing, &result);
        result
    }
}
/// The caller prepaid all summary words; no affected dispatch can escape the owner gate.
fn clear_ready(backing: &mut Backing, slot: usize, clear: bool) {
    if clear {
        bitmap::set(&mut backing.ready, backing.nodes.len(), slot, false);
        backing.nodes[slot].flags &= !READY;
    }
}
/// One edge or one exhausted frame is one indivisible, resumable transition.
fn step(backing: &mut Backing, budget: &mut Budget) -> Result<(), Error> {
    let depth = backing.control.pending[1];
    let frame = backing.stack[depth - 1];
    if frame.edge == NONE {
        charge(budget, 3)?; // N exhausted frame + X pop + H saved traversal cursor/gate.
        enter(backing)?;
        backing.control.pending[1] -= 1;
        if depth == 1 {
            backing.control.flags &= !INVALIDATING;
            backing.control.pending = [NONE; 2];
        }
        return Ok(());
    }
    charge(budget, 4)?; // E edge, N source cursor, N target generation/state, M visit word.
    let edge = backing.edges[frame.edge];
    let target = edge.endpoints[1];
    if backing.nodes[target.slot].generation != target.generation {
        return Err(LoanError::StaleOccupation.into());
    }
    let fresh = backing.marks[target.slot] != backing.control.epochs[1];
    let clear = fresh
        && backing.nodes[target.slot].flags & READY != 0
        && !super::control::is_control(backing, target.slot);
    // H traversal save; a fresh child also pushes X and clears its complete ready path.
    let work = 1
        + usize::from(fresh)
        + if clear {
            bitmap::height(backing.nodes.len())
        } else {
            0
        };
    charge(budget, work)?;
    enter(backing)?;
    backing.stack[depth - 1].edge = edge.links[1];
    if fresh {
        backing.marks[target.slot] = backing.control.epochs[1];
        backing.nodes[target.slot].flags |= INVALID;
        clear_ready(backing, target.slot, clear);
        backing.stack[depth] = Frame {
            node: target.slot,
            edge: backing.nodes[target.slot].links[0],
        };
        backing.control.pending[1] += 1;
    }
    Ok(())
}
