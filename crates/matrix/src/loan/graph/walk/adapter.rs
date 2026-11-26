// SPDX-License-Identifier: AGPL-3.0-only
//! Native phase composition shares the original ready index and control ordering.

use super::{Core, Error, Handle, INVALIDATING, SCOPING, STARTED, STOPPED, bitmap, control};
use crate::loan::charge;
use crate::{ExecutionAccount, LoanError};
use nrz_bindings::Budget;

/// Selection grants no native execution authority until the phase's attempt is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::loan::graph) enum Selection {
    /// No currently eligible transition exists.
    Quiescent,
    /// Shared traversal control must finish first.
    ControlPending,
    /// One notification, release or retirement step committed.
    Control(control::Control),
    /// The concrete adapter must inspect the native phase before admitting its call.
    Native(Handle),
}
impl Core {
    /// Adapter entry points preserve original Work failure even before a native attempt begins.
    pub(in crate::loan::graph) fn record_external<T>(&mut self, result: &Result<T, Error>) {
        if let Some(backing) = std::rc::Rc::get_mut(&mut self.reservation.backing) {
            if matches!(result, Err(Error::Adapter(error)) if error.work_stopped()) {
                backing.control.flags |= STOPPED;
            }
            super::record_stop(backing, result);
        }
    }

    /// Preserve a native original-ledger Work stop using the caller's prepaid header update.
    pub(in crate::loan::graph) fn stop_native(&mut self) -> Result<(), Error> {
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        backing.control.flags |= STOPPED;
        Ok(())
    }

    /// Select in the same cyclic order, giving existing notification/release/retirement priority.
    pub(in crate::loan::graph) fn select(
        &mut self,
        account: &mut ExecutionAccount,
        mut demand: impl FnMut(usize, &mut Budget) -> Result<control::Demand, LoanError>,
        mut release: impl FnMut(usize),
    ) -> Result<Selection, Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result =
            account
                .ledger
                .with_work_phase("matrix.graph", "native phase selection", |budget| {
                    charge(budget, 1)?;
                    if backing.control.flags & STOPPED != 0 {
                        return Err(Error::WorkStopped);
                    }
                    if backing.control.flags & (INVALIDATING | SCOPING) != 0 {
                        return Ok(Selection::ControlPending);
                    }
                    let Some(slot) = bitmap::next(
                        &backing.ready,
                        backing.nodes.len(),
                        backing.control.ready,
                        budget,
                    )?
                    else {
                        return Ok(Selection::Quiescent);
                    };
                    if backing.drive == 0 {
                        return Err(Error::TraversalExhausted);
                    }
                    charge(budget, 1)?; // N selected phase.
                    if control::is_control(backing, slot) {
                        Ok(Selection::Control(control::step::<true>(
                            backing,
                            slot,
                            budget,
                            &mut demand,
                            &mut release,
                        )?))
                    } else {
                        Ok(Selection::Native(Handle {
                            owner: backing.identity,
                            slot,
                            generation: backing.nodes[slot].generation,
                        }))
                    }
                });
        super::record_stop(backing, &result);
        result
    }

    /// Native input expectations wait outside the ready set without consuming a fictitious probe.
    pub(in crate::loan::graph) fn park(
        &mut self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        self.phase_ready(handle, false, account)
    }

    /// A saved successor phase becomes eligible only after its exact native owner was preserved.
    pub(in crate::loan::graph) fn resume(
        &mut self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        self.phase_ready(handle, true, account)
    }

    /// Prepay the complete index change before touching phase or summary words.
    fn phase_ready(
        &mut self,
        handle: Handle,
        ready: bool,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result =
            account
                .ledger
                .with_work_phase("matrix.graph", "native phase preservation", |budget| {
                    control::authenticate(backing, handle, budget)?;
                    if backing.control.flags & STOPPED != 0 {
                        return Err(Error::WorkStopped);
                    }
                    if backing.control.flags & (INVALIDATING | SCOPING) != 0 {
                        return Err(Error::ControlInProgress);
                    }
                    if backing.nodes[handle.slot].flags & control::COMPLETE != 0 {
                        return Err(Error::InvalidPhase);
                    }
                    let node = backing.nodes[handle.slot];
                    let waiting = node.flags & super::WAITING != 0;
                    let neighbours = if waiting {
                        usize::from(node.links[2] != super::NONE)
                            + usize::from(node.links[3] != super::NONE)
                            + 1
                    } else {
                        0
                    };
                    charge(budget, bitmap::height(backing.nodes.len()) + neighbours)?;
                    if waiting {
                        super::waiting::remove(backing, handle.slot);
                    }
                    if ready {
                        backing.nodes[handle.slot].flags &= !STARTED;
                    } else {
                        backing.nodes[handle.slot].flags |= STARTED;
                    }
                    control::refresh(backing, handle.slot);
                    Ok(())
                });
        super::record_stop(backing, &result);
        result
    }

    /// Fan-out retention consumes the additional consumer's original attempt, never its producer's.
    /// The caller must prepare the corresponding sink destination before invoking this method.
    pub(in crate::loan::graph) fn enter_external(
        &mut self,
        selected: Handle,
        attempted: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<bool, Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result =
            account
                .ledger
                .with_work_phase("matrix.graph", "external native attempt", |budget| {
                    control::authenticate(backing, selected, budget)?;
                    control::authenticate(backing, attempted, budget)?;
                    if backing.control.flags & STOPPED != 0 {
                        return Err(Error::WorkStopped);
                    }
                    if backing.control.flags & (INVALIDATING | SCOPING) != 0 {
                        return Err(Error::ControlInProgress);
                    }
                    if backing.drive == 0 {
                        return Err(Error::TraversalExhausted);
                    }
                    charge(budget, 1)?; // H saved cyclic cursor and traversal entry.
                    let waiting = backing.nodes[attempted.slot].flags & super::WAITING != 0;
                    if waiting {
                        if backing.control.flags & super::SCANNING == 0
                            || backing.control.waiting[2] != attempted.slot
                        {
                            return Err(Error::InvalidPhase);
                        }
                        charge(budget, 2)?; // H saved scan cursor and N visited capacity epoch.
                    }
                    let exhausted = backing.nodes[attempted.slot].attempts == 0;
                    if waiting {
                        backing.nodes[attempted.slot].epochs[2] = backing.control.epochs[3];
                        backing.control.waiting[2] = backing.nodes[attempted.slot].links[3];
                    }
                    backing.drive -= 1;
                    if !exhausted {
                        backing.nodes[attempted.slot].attempts -= 1;
                    }
                    super::advance(backing, selected.slot);
                    Ok(!exhausted)
                });
        super::record_stop(backing, &result);
        result
    }
}
