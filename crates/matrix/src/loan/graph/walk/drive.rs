// SPDX-License-Identifier: AGPL-3.0-only
//! One bounded core visit separates capacity evidence from any future numerical adapter.

#[cfg(test)]
use super::{
    ATTEMPTS, Core, Handle, INVALID, INVALIDATING, LIVE, READY, STARTED, STOPPED, TERMINAL,
    advance, bitmap, enter,
};
use super::{Error, NONE, WAITING};
#[cfg(test)]
use crate::loan::ExecutionAccount;
use crate::loan::graph::Backing;
use crate::loan::{LoanError, charge};
use nrz_bindings::Budget;

/// A core fixture's explicit capacity result; no mathematical result is manufactured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Capacity {
    /// The complete checked incremental peak fits beside current live owners.
    Fits,
    /// A releasable baseline blocks an otherwise fitting addition.
    Wait {
        /// Complete scoped requirement.
        required: u64,
        /// Currently available capacity in the same dimension.
        available: u64,
    },
    /// The addition alone exceeds the immutable dimension.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the core capacity classifier retains its independently tested intrinsic-refusal case"
        )
    )]
    Intrinsic {
        /// Addition alone, excluding the releasable baseline.
        required: u64,
        /// Immutable limit for that dimension.
        limit: u64,
    },
}
/// One entered transition consumes one cumulative traversal unit, never a refilled slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub(crate) enum Visit {
    /// No executable leaf remains in this core snapshot.
    Quiescent,
    /// This call has exhausted its smaller slice; original allowances remain unchanged.
    Paused,
    /// The indexed node entered one ordered control phase.
    Control(super::control::Control),
    /// Dispatch is closed while the saved invalidation is active.
    ControlPending,
    /// The fixture operation may now enter its declared phase; this is not publication.
    Started(Handle),
    /// A retained waiting node reports its actual resource evidence.
    Waiting(Handle, Capacity),
    /// No attempts remain; its pending use-release cursor is retained.
    AttemptsExhausted(Handle),
    /// Intrinsic refusal is terminal rather than a capacity retry.
    Refused(Handle, Capacity),
}
#[cfg(test)]
impl Core {
    /// A one-transition drive slice allows the caller to compose native phases only in F.
    #[cfg(test)]
    pub(crate) fn drive_one(
        &mut self,
        account: &mut ExecutionAccount,
        probe: impl FnMut(usize, &mut Budget) -> Result<Capacity, LoanError>,
        demand: impl FnMut(usize, &mut Budget) -> Result<super::control::Demand, LoanError>,
        release: impl FnMut(usize),
        receipt: impl FnMut(super::waiting::ScanReceipt),
    ) -> Result<Visit, Error> {
        self.drive_slice(1, account, probe, demand, release, receipt)
    }
    /// Fixture callbacks exercise the same walk without installing a delivery owner.
    #[cfg(test)]
    pub(crate) fn drive_slice(
        &mut self,
        slice: usize,
        account: &mut ExecutionAccount,
        probe: impl FnMut(usize, &mut Budget) -> Result<Capacity, LoanError>,
        demand: impl FnMut(usize, &mut Budget) -> Result<super::control::Demand, LoanError>,
        release: impl FnMut(usize),
        receipt: impl FnMut(super::waiting::ScanReceipt),
    ) -> Result<Visit, Error> {
        let mut attempt = super::attempt::Fixture { probe, receipt };
        self.drive_prepared(slice, account, &mut attempt, demand, release)
    }
    /// A smaller call slice never refills original attempts, Work or traversal allowance.
    #[cfg(test)]
    pub(crate) fn drive_prepared(
        &mut self,
        slice: usize,
        account: &mut ExecutionAccount,
        attempt: &mut impl super::attempt::Attempt,
        mut demand: impl FnMut(usize, &mut Budget) -> Result<super::control::Demand, LoanError>,
        mut release: impl FnMut(usize),
    ) -> Result<Visit, Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result = account
            .ledger
            .with_work_phase("matrix.graph", "ready walk", |budget| {
                charge(budget, 1)?; // H operation entry.
                if backing.control.flags & STOPPED != 0 {
                    return Err(Error::WorkStopped);
                }
                if backing.control.flags & (INVALIDATING | super::SCOPING) != 0 {
                    return Ok(Visit::ControlPending);
                }
                let mut last = Visit::Paused;
                for _ in 0..slice {
                    let Some(slot) = bitmap::next(
                        &backing.ready,
                        backing.nodes.len(),
                        backing.control.ready,
                        budget,
                    )?
                    else {
                        return Ok(Visit::Quiescent);
                    };
                    if backing.drive == 0 {
                        return Err(Error::TraversalExhausted);
                    }
                    charge(budget, 1)?; // N node phase and current eligibility.
                    last = if super::control::is_control(backing, slot) {
                        Visit::Control(super::control::step::<false>(
                            backing,
                            slot,
                            budget,
                            &mut demand,
                            &mut release,
                        )?)
                    } else {
                        visit(backing, slot, budget, attempt)?
                    };
                }
                Ok(last)
            });
        super::record_stop(backing, &result);
        result
    }
}
/// Fund each fixed mutation before consuming an attempt or changing ready/wait membership.
#[cfg(test)]
fn visit(
    backing: &mut Backing,
    slot: usize,
    budget: &mut Budget,
    attempt: &mut impl super::attempt::Attempt,
) -> Result<Visit, Error> {
    charge(budget, 1)?; // N leaf state, original attempt count and prepared update.
    let node = &backing.nodes[slot];
    let runnable = node.flags & READY != 0;
    let live = node.flags & LIVE != 0;
    let valid = node.flags & INVALID == 0;
    if !runnable || !live || !valid {
        return Err(LoanError::StaleOccupation.into());
    }
    let handle = Handle {
        owner: backing.identity,
        slot,
        generation: node.generation,
    };
    let exhausted = node.attempts == 0;
    // H saved cyclic cursor + complete changed ready path. A probe is entered only afterward.
    charge(budget, 1 + bitmap::height(backing.nodes.len()))?;
    attempt.prepare(handle, exhausted, budget)?;
    if exhausted {
        enter(backing)?;
        attempt.receipt(super::waiting::ScanReceipt {
            handle,
            capacity: None,
        });
        backing.nodes[slot].flags |=
            TERMINAL | ATTEMPTS | super::control::COMPLETE | super::control::EVIDENCE;
        backing.nodes[slot].links[5] = backing.nodes[slot].links[0];
        super::control::refresh(backing, slot);
        advance(backing, slot);
        return Ok(Visit::AttemptsExhausted(handle));
    }
    // Entered capacity probes retain both the consumed attempt and their actual native Work.
    enter(backing)?;
    backing.nodes[slot].attempts -= 1;
    let capacity = attempt.probe(slot, budget)?;
    attempt.receipt(super::waiting::ScanReceipt {
        handle,
        capacity: Some(capacity),
    });
    match capacity {
        Capacity::Fits => {
            clear(backing, slot);
            backing.nodes[slot].flags |= STARTED;
            advance(backing, slot);
            Ok(Visit::Started(handle))
        }
        Capacity::Intrinsic { .. } => {
            backing.nodes[slot].flags |=
                TERMINAL | super::control::COMPLETE | super::control::EVIDENCE;
            backing.nodes[slot].links[5] = backing.nodes[slot].links[0];
            super::control::refresh(backing, slot);
            advance(backing, slot);
            Ok(Visit::Refused(handle, capacity))
        }
        Capacity::Wait { .. } => {
            append(backing, slot, budget)?;
            clear(backing, slot);
            advance(backing, slot);
            Ok(Visit::Waiting(handle, capacity))
        }
    }
}
/// All ready-path visits were prepaid before the attempted transition.
#[cfg(test)]
fn clear(backing: &mut Backing, slot: usize) {
    bitmap::set(&mut backing.ready, backing.nodes.len(), slot, false);
    backing.nodes[slot].flags &= !READY;
}
/// The membership serial is checked before links change; append order is deterministic.
pub(super) fn append(backing: &mut Backing, slot: usize, budget: &mut Budget) -> Result<(), Error> {
    let serial = backing.control.epochs[4]
        .checked_add(1)
        .ok_or(LoanError::CountOverflow)?;
    let tail = backing.control.waiting[1];
    charge(budget, 1 + usize::from(tail != NONE))?; // H membership/header transition + N previous tail.
    backing.nodes[slot].flags |= WAITING;
    backing.nodes[slot].epochs[3] = serial;
    backing.nodes[slot].epochs[2] = backing.control.epochs[3];
    backing.nodes[slot].links[2] = tail;
    backing.nodes[slot].links[3] = NONE;
    if tail == NONE {
        backing.control.waiting[0] = slot;
    } else {
        backing.nodes[tail].links[3] = slot;
    }
    backing.control.waiting[1] = slot;
    backing.control.epochs[4] = serial;
    Ok(())
}
