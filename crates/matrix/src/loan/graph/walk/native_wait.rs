// SPDX-License-Identifier: AGPL-3.0-only
//! Native retries share the finite wait list and its captured membership watermark.

use super::{
    Core, Error, Handle, INVALID, INVALIDATING, NONE, SCANNING, SCOPING, STOPPED, WAITING, bitmap,
    control, drive, enter,
};
use crate::loan::charge;
use crate::{ExecutionAccount, LoanError};

impl Core {
    /// A native temporary refusal appends at most one original membership in stable order.
    pub(in crate::loan::graph) fn wait_native(
        &mut self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result =
            account
                .ledger
                .with_work_phase("matrix.graph", "native capacity wait", |budget| {
                    control::authenticate(backing, handle, budget)?;
                    if backing.control.flags & STOPPED != 0 {
                        return Err(Error::WorkStopped);
                    }
                    if backing.control.flags & (INVALIDATING | SCOPING) != 0 {
                        return Err(Error::ControlInProgress);
                    }
                    if backing.nodes[handle.slot].flags & WAITING != 0 {
                        return Ok(());
                    }
                    charge(budget, bitmap::height(backing.nodes.len()))?;
                    drive::append(backing, handle.slot, budget)?;
                    control::refresh(backing, handle.slot);
                    Ok(())
                });
        super::record_stop(backing, &result);
        result
    }

    /// Select at most one original wait member without consuming its attempt before sink preparation.
    pub(in crate::loan::graph) fn waiting_native(
        &mut self,
        account: &mut ExecutionAccount,
    ) -> Result<Option<Handle>, Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result =
            account
                .ledger
                .with_work_phase("matrix.graph", "native wait selection", |budget| {
                    charge(budget, 1)?; // H original scan, stop and traversal header.
                    if backing.control.flags & STOPPED != 0 {
                        return Err(Error::WorkStopped);
                    }
                    if backing.control.flags & (INVALIDATING | SCOPING) != 0 {
                        return Err(Error::ControlInProgress);
                    }
                    if backing.control.flags & SCANNING == 0 {
                        return Ok(None);
                    }
                    if backing.drive == 0 {
                        return Err(Error::TraversalExhausted);
                    }
                    let slot = backing.control.waiting[2];
                    if slot == NONE {
                        charge(budget, 1)?; // H completed bounded scan.
                        enter(backing)?;
                        backing.control.flags &= !SCANNING;
                        backing.control.watermark = 0;
                        return Ok(None);
                    }
                    charge(budget, 1)?; // N membership serial, epoch and next link.
                    let node = backing.nodes[slot];
                    if node.epochs[3] > backing.control.watermark {
                        charge(budget, 1)?; // H stop before newly appended membership.
                        enter(backing)?;
                        backing.control.flags &= !SCANNING;
                        backing.control.waiting[2] = NONE;
                        backing.control.watermark = 0;
                        return Ok(None);
                    }
                    let visited = node.epochs[2] == backing.control.epochs[3];
                    let invalid = node.flags & INVALID != 0;
                    if visited || invalid {
                        charge(budget, 1)?; // H saved cursor, without consuming a native attempt.
                        enter(backing)?;
                        backing.control.waiting[2] = node.links[3];
                        return Ok(None);
                    }
                    Ok(Some(Handle {
                        owner: backing.identity,
                        slot,
                        generation: node.generation,
                    }))
                });
        super::record_stop(backing, &result);
        result
    }
}
