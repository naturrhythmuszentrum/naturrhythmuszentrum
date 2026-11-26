// SPDX-License-Identifier: AGPL-3.0-only
//! Waiting scans capture a serial watermark and repair their cursor on removal.

#[cfg(test)]
use super::drive::Capacity;
#[cfg(test)]
use super::{ATTEMPTS, INVALID, READY, TERMINAL, bitmap};
use super::{Core, Error, NONE, SCANNING, WAITING, enter};
use crate::loan::graph::Backing;
use crate::loan::{ExecutionAccount, LoanError, charge};
#[cfg(test)]
use nrz_bindings::Budget;

/// Every entered capacity probe returns its original typed evidence to the bounded caller sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub(crate) struct ScanReceipt {
    /// Exact occupation attempted, never just a reusable slot number.
    pub(crate) handle: super::Handle,
    /// None means attempt exhaustion, with no capacity probe entered.
    pub(crate) capacity: Option<Capacity>,
}

impl Core {
    /// Explicit drive or relevant release starts one new epoch, never replacing an active scan.
    pub(crate) fn begin_capacity_epoch(
        &mut self,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result = account
            .ledger
            .with_work_phase("matrix.graph", "capacity epoch", |budget| {
                charge(budget, 1)?; // H operation entry.
                if backing.control.flags & super::STOPPED != 0 {
                    return Err(Error::WorkStopped);
                }
                if backing.control.flags & (super::INVALIDATING | super::SCOPING) != 0 {
                    return Err(Error::ControlInProgress);
                }
                if backing.control.flags & SCANNING != 0 {
                    return Err(Error::ControlInProgress);
                }
                let epoch = backing.control.epochs[3]
                    .checked_add(1)
                    .ok_or(LoanError::CountOverflow)?;
                if backing.drive == 0 {
                    return Err(Error::TraversalExhausted);
                }
                charge(budget, 1)?; // H epoch/cursor/watermark transition.
                enter(backing)?;
                backing.control.epochs[3] = epoch;
                backing.control.watermark = backing.control.epochs[4];
                backing.control.waiting[2] = backing.control.waiting[0];
                backing.control.flags |= SCANNING;
                Ok(())
            });
        super::record_stop(backing, &result);
        result
    }
    /// Cancellation retains membership and spent attempts, clearing only the saved scan.
    pub(crate) fn cancel_capacity_scan(
        &mut self,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result = account.ledger.with_work_phase(
            "matrix.graph",
            "capacity scan cancellation",
            |budget| {
                charge(budget, 2)?; // H operation entry + H saved cursor/watermark transition.
                if backing.control.flags & super::STOPPED != 0 {
                    return Err(Error::WorkStopped);
                }
                enter(backing)?;
                finish(backing);
                Ok(())
            },
        );
        super::record_stop(backing, &result);
        result
    }
    /// Fixture scans share the production attempt boundary without a delivery owner.
    #[cfg(test)]
    pub(crate) fn scan_capacity(
        &mut self,
        slice: usize,
        account: &mut ExecutionAccount,
        probe: impl FnMut(usize, &mut Budget) -> Result<Capacity, LoanError>,
        receipt: impl FnMut(ScanReceipt),
    ) -> Result<bool, Error> {
        let mut attempt = super::attempt::Fixture { probe, receipt };
        self.scan_prepared(slice, account, &mut attempt)
    }
    /// Newly appended or reinserted membership cannot extend the captured scan.
    #[cfg(test)]
    pub(crate) fn scan_prepared(
        &mut self,
        slice: usize,
        account: &mut ExecutionAccount,
        attempt: &mut impl super::attempt::Attempt,
    ) -> Result<bool, Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result = account
            .ledger
            .with_work_phase("matrix.graph", "capacity scan", |budget| {
                charge(budget, 1)?; // H operation entry.
                if backing.control.flags & super::STOPPED != 0 {
                    return Err(Error::WorkStopped);
                }
                if backing.control.flags & (super::INVALIDATING | super::SCOPING) != 0 {
                    return Err(Error::ControlInProgress);
                }
                for _ in 0..slice {
                    if backing.control.flags & SCANNING == 0 {
                        return Ok(true);
                    }
                    if backing.drive == 0 {
                        return Err(Error::TraversalExhausted);
                    }
                    scan_one(backing, budget, attempt)?;
                }
                Ok(backing.control.flags & SCANNING == 0)
            });
        super::record_stop(backing, &result);
        result
    }
}
/// One bounded scan member is inspected once; a retry consumes its original attempt.
#[cfg(test)]
fn scan_one(
    backing: &mut Backing,
    budget: &mut Budget,
    attempt: &mut impl super::attempt::Attempt,
) -> Result<(), Error> {
    let slot = backing.control.waiting[2];
    if slot == NONE {
        charge(budget, 1)?; // H completed scan transition.
        enter(backing)?;
        finish(backing);
        return Ok(());
    }
    charge(budget, 1)?; // N membership, serial, epoch, attempt and saved next link.
    let node = backing.nodes[slot];
    if node.epochs[3] > backing.control.watermark {
        charge(budget, 1)?; // H bounded stop before a newly inserted member.
        enter(backing)?;
        finish(backing);
        return Ok(());
    }
    let already_attempted = node.epochs[2] == backing.control.epochs[3];
    let invalid = node.flags & INVALID != 0;
    if already_attempted || invalid {
        charge(budget, 1)?; // H scan cursor, with no attempt or readiness change.
        enter(backing)?;
        backing.control.waiting[2] = node.links[3];
        return Ok(());
    }
    let exhausted = node.attempts == 0;
    // Fund the fixed cursor save before entering a probe; no fresh attempt is fabricated.
    charge(budget, 1)?;
    attempt.prepare(
        super::Handle {
            owner: backing.identity,
            slot,
            generation: node.generation,
        },
        exhausted,
        budget,
    )?;
    enter(backing)?;
    let capacity = if exhausted {
        None
    } else {
        backing.nodes[slot].attempts -= 1;
        backing.nodes[slot].epochs[2] = backing.control.epochs[3];
        Some(attempt.probe(slot, budget)?)
    };
    // Return evidence before any subsequent graph debit can refuse; no receipt is lost
    // on a later commit failure. The caller supplies its bounded evidence destination.
    attempt.receipt(ScanReceipt {
        handle: super::Handle {
            owner: backing.identity,
            slot,
            generation: node.generation,
        },
        capacity,
    });
    let keep_waiting = matches!(capacity, Some(Capacity::Wait { .. }));
    if !keep_waiting {
        let fits = matches!(capacity, Some(Capacity::Fits));
        let neighbours = usize::from(node.links[2] != NONE) + usize::from(node.links[3] != NONE);
        charge(budget, neighbours + bitmap::height(backing.nodes.len()))?;
        remove(backing, slot);
        if fits {
            backing.nodes[slot].flags |= READY;
            bitmap::set(&mut backing.ready, backing.nodes.len(), slot, true);
        } else {
            backing.nodes[slot].flags |=
                TERMINAL | super::control::COMPLETE | super::control::EVIDENCE;
            backing.nodes[slot].links[5] = backing.nodes[slot].links[0];
            super::control::refresh(backing, slot);
            if exhausted {
                backing.nodes[slot].flags |= ATTEMPTS;
            }
        }
    }
    backing.control.waiting[2] = node.links[3];
    Ok(())
}
/// Commit only after the caller funds every affected neighbour and the saved header cursor.
pub(super) fn remove(backing: &mut Backing, slot: usize) {
    let previous = backing.nodes[slot].links[2];
    let next = backing.nodes[slot].links[3];
    if previous == NONE {
        backing.control.waiting[0] = next;
    } else {
        backing.nodes[previous].links[3] = next;
    }
    if next == NONE {
        backing.control.waiting[1] = previous;
    } else {
        backing.nodes[next].links[2] = previous;
    }
    if backing.control.waiting[2] == slot {
        backing.control.waiting[2] = next;
    }
    backing.nodes[slot].links[2] = NONE;
    backing.nodes[slot].links[3] = NONE;
    backing.nodes[slot].flags &= !WAITING;
}
/// Clearing a scan never clears any waiting-node obligation.
const fn finish(backing: &mut Backing) {
    backing.control.waiting[2] = NONE;
    backing.control.watermark = 0;
    backing.control.flags &= !SCANNING;
}
