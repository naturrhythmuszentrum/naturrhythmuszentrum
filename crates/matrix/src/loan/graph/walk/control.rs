// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered core control phases use the same funded index as application dispatch.

use super::{
    Backing, Core, Error, Handle, INVALID, LIVE, NONE, READY, TERMINAL, WAITING, advance, bitmap,
    enter,
};
use crate::loan::{ExecutionAccount, LoanError, charge};
use nrz_bindings::Budget;

/// A preserved fixture outcome permits notification and subsequent input-use release.
pub(super) const COMPLETE: u64 = 512;
/// Required evidence has not yet been acknowledged; this blocks retirement only.
pub(super) const EVIDENCE: u64 = 1024;
/// A dependency refused or lacked its demanded coverage; never a numerical value.
pub(super) const MISSING: u64 = 2048;
/// Dependency settlement is independent of the edge's lifetime retention.
const SETTLED: u64 = 2;
/// The fixed incidence slot has been released, not merely settled.
const RELEASED: u64 = 4;

/// The fixture's demand predicate reports only coverage/terminal state, never a theorem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Demand {
    /// Published coverage satisfies the original demand.
    Satisfied,
    /// The producer settled but cannot meet this required demand.
    Missing,
    /// Declared absence is consumable by this edge's explicit contract.
    AcceptedAbsence,
    /// A mandatory producer closed without its required contribution.
    RequiredAbsence,
    /// An inactive alternative has no outstanding numerical contribution.
    Inactive,
    /// The producer refused; original evidence remains with its owner.
    Refused,
}
/// One committed bounded control action, distinguishable from numerical dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Control {
    /// This edge settled once; lifetime ownership is still retained.
    Notified(usize),
    /// Only the graph-owned use was surrendered.
    Released(usize),
    /// Generation advanced, or a maximal-generation slot was permanently retired.
    Retired(usize),
}
impl Core {
    /// Exercise the bounded vacancy mechanism without exposing incremental graph admission.
    #[cfg(test)]
    pub(super) fn reuse_for_control(
        &mut self,
        account: &mut ExecutionAccount,
    ) -> Result<Option<Handle>, Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        account
            .ledger
            .with_work_phase("matrix.graph", "isolated pool control", |budget| {
                charge(budget, 1)?; // H entry and free-head inspection.
                let slot = backing.control.free[0];
                if slot == NONE {
                    return Ok(None);
                }
                if backing.drive == 0 {
                    return Err(Error::TraversalExhausted);
                }
                let height = bitmap::height(backing.nodes.len());
                let cleanup =
                    super::add(3, height.checked_mul(2).ok_or(LoanError::CountOverflow)?)?;
                let records = super::add(backing.cleanup.records, cleanup)?;
                charge(budget, super::add(2 + height, cleanup)?)?; // N slot, H free-head transition, M path.
                enter(backing)?;
                backing.cleanup.records = records;
                backing.control.free[0] = backing.nodes[slot].links[4];
                backing.nodes[slot].flags = LIVE;
                backing.nodes[slot].links[4] = NONE;
                refresh(backing, slot);
                Ok(Some(Handle {
                    owner: backing.identity,
                    slot,
                    generation: backing.nodes[slot].generation,
                }))
            })
    }
    /// Explicit whole-owner abandonment spends only finite admission cleanup entitlements.
    /// No native outcome is invented; the callback surrenders only the fixture's owned uses.
    pub(crate) fn abandon(
        mut self,
        account: &ExecutionAccount,
        mut release: impl FnMut(usize),
    ) -> Result<(), (Error, Self)> {
        if let Err(error) = account.authenticate(self.reservation.backing.account) {
            return Err((error.into(), self));
        }
        let Some(backing) = std::rc::Rc::get_mut(&mut self.reservation.backing) else {
            return Err((LoanError::WrongLoan.into(), self));
        };
        // Teardown inspects fixed pools under distinct P entitlements; it does not search
        // for eligible work or perform ordinary driving after a cumulative stop.
        for edge in &mut backing.edges {
            if edge.flags & RELEASED == 0 {
                release(edge.demand);
                edge.flags |= RELEASED;
            }
        }
        for slot in 0..backing.nodes.len() {
            if backing.nodes[slot].flags & LIVE != 0 {
                bitmap::set(&mut backing.ready, backing.nodes.len(), slot, false);
                backing.nodes[slot].flags = 0;
            }
        }
        Ok(())
    }
    /// Preserve a fixture outcome before enabling outgoing notifications. No LC result exists.
    pub(crate) fn complete(
        &mut self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result = account
            .ledger
            .with_work_phase("matrix.graph", "fixture outcome", |budget| {
                authenticate(backing, handle, budget)?;
                if backing.control.flags & super::STOPPED != 0 {
                    return Err(Error::WorkStopped);
                }
                if backing.control.flags & super::INVALIDATING != 0 {
                    return Err(Error::ControlInProgress);
                }
                if backing.nodes[handle.slot].flags & COMPLETE != 0 {
                    return Ok(());
                }
                let node = &backing.nodes[handle.slot];
                let started = node.flags & super::STARTED != 0;
                let permitted = node.flags & (INVALID | WAITING | TERMINAL) == 0;
                if !started || !permitted || node.counts[0] != 0 {
                    return Err(Error::InvalidPhase);
                }
                let epoch = backing.control.epochs[0]
                    .checked_add(1)
                    .ok_or(LoanError::CountOverflow)?;
                if backing.drive == 0 {
                    return Err(Error::TraversalExhausted);
                }
                charge(budget, 1 + bitmap::height(backing.nodes.len()))?; // H notification epoch, M full path.
                enter(backing)?;
                backing.control.epochs[0] = epoch;
                backing.nodes[handle.slot].epochs[0] = epoch;
                backing.nodes[handle.slot].flags |= COMPLETE | EVIDENCE;
                backing.nodes[handle.slot].links[5] = backing.nodes[handle.slot].links[0];
                refresh(backing, handle.slot);
                Ok(())
            });
        super::record_stop(backing, &result);
        result
    }
    /// Evidence acknowledgment alone grants no dispatch authority and releases no use.
    pub(crate) fn acknowledge(
        &mut self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.reservation.backing.account)?;
        let backing =
            std::rc::Rc::get_mut(&mut self.reservation.backing).ok_or(LoanError::WrongLoan)?;
        let result = account
            .ledger
            .with_work_phase("matrix.graph", "fixture evidence", |budget| {
                authenticate(backing, handle, budget)?;
                if backing.control.flags & super::STOPPED != 0 {
                    return Err(Error::WorkStopped);
                }
                if backing.control.flags & super::INVALIDATING != 0 {
                    return Err(Error::ControlInProgress);
                }
                if backing.drive == 0 {
                    return Err(Error::TraversalExhausted);
                }
                if backing.nodes[handle.slot].flags & COMPLETE == 0 {
                    return Err(Error::InvalidPhase);
                }
                charge(budget, bitmap::height(backing.nodes.len()))?; // M eligibility path.
                enter(backing)?;
                backing.nodes[handle.slot].flags &= !EVIDENCE;
                refresh(backing, handle.slot);
                Ok(())
            });
        super::record_stop(backing, &result);
        result
    }
}
/// H entry, A admitted owner and N exact occupation; no foreign slot is inspected.
fn authenticate(backing: &Backing, handle: Handle, budget: &mut Budget) -> Result<(), Error> {
    charge(budget, 2)?;
    if handle.owner != backing.identity {
        return Err(LoanError::ForeignOwner.into());
    }
    charge(budget, 1)?;
    let node = backing
        .nodes
        .get(handle.slot)
        .ok_or(LoanError::StaleOccupation)?;
    let live = node.flags & LIVE != 0;
    let same = node.generation == handle.generation;
    if !live || !same {
        return Err(LoanError::StaleOccupation.into());
    }
    Ok(())
}
/// A node's discriminant, not its bit alone, selects control versus application work.
pub(super) fn is_control(backing: &Backing, slot: usize) -> bool {
    backing.nodes[slot].flags & COMPLETE != 0
}
/// Retirement is gated by notification, own input uses, consumers and evidence.
fn eligible(backing: &Backing, slot: usize) -> bool {
    let node = &backing.nodes[slot];
    if node.flags & LIVE == 0 {
        return false;
    }
    if node.flags & COMPLETE != 0 {
        let notification = node.links[5] != NONE;
        let release = node.links[1] != NONE;
        let retire = node.counts[1] == 0 && node.flags & EVIDENCE == 0;
        return notification || release || retire;
    }
    let dispatchable = node.flags & (INVALID | WAITING | super::STARTED) == 0;
    dispatchable && node.counts[0] == 0
}
/// Caller funds the complete path, either M now or one finite cleanup P entitlement.
pub(super) fn refresh(backing: &mut Backing, slot: usize) {
    let value = eligible(backing, slot);
    bitmap::set(&mut backing.ready, backing.nodes.len(), slot, value);
    if value {
        backing.nodes[slot].flags |= READY;
    } else {
        backing.nodes[slot].flags &= !READY;
    }
}
/// Notification has priority; cleanup visits consume only previously charged P events.
pub(super) fn step(
    backing: &mut Backing,
    slot: usize,
    budget: &mut Budget,
    demand: &mut impl FnMut(usize, &mut Budget) -> Result<Demand, LoanError>,
    release: &mut impl FnMut(usize),
) -> Result<Control, Error> {
    if backing.nodes[slot].links[5] != NONE {
        notify(backing, slot, budget, demand)
    } else if backing.nodes[slot].links[1] != NONE {
        enter(backing)?;
        Ok(release_one(backing, slot, release))
    } else {
        let node = &backing.nodes[slot];
        let preserved = node.flags & COMPLETE != 0 && node.flags & EVIDENCE == 0;
        let detached = node.links[0] == NONE && node.counts[1] == 0;
        if !preserved || !detached {
            return Err(Error::InvalidPhase);
        }
        enter(backing)?;
        Ok(retire(backing, slot))
    }
}
/// Private native metadata checks precede the complete edge/count/path commit prepayment.
fn notify(
    backing: &mut Backing,
    slot: usize,
    budget: &mut Budget,
    demand: &mut impl FnMut(usize, &mut Budget) -> Result<Demand, LoanError>,
) -> Result<Control, Error> {
    let index = backing.nodes[slot].links[5];
    charge(budget, 3)?; // E edge, N producer generation/cursor, N consumer generation/count.
    let edge = backing.edges[index];
    for endpoint in edge.endpoints {
        if backing.nodes[endpoint.slot].generation != endpoint.generation {
            return Err(LoanError::StaleOccupation.into());
        }
    }
    let consumer = edge.endpoints[1].slot;
    let fresh = edge.flags & SETTLED == 0;
    let outcome = if fresh {
        Some(demand(edge.demand, budget)?)
    } else {
        None
    };
    let settles = fresh && backing.nodes[consumer].counts[0] == 1;
    let height = bitmap::height(backing.nodes.len());
    let node = &backing.nodes[slot];
    let remains = edge.links[1] != NONE
        || node.links[1] != NONE
        || (node.counts[1] == 0 && node.flags & EVIDENCE == 0);
    let changes = !remains;
    charge(
        budget,
        1 + if changes { height } else { 0 } + if settles { height } else { 0 },
    )?; // H cyclic cursor + M producer and changed consumer path.
    enter(backing)?;
    if let Some(outcome) = outcome {
        // Inline outcome discriminant survives settlement and later release; no new storage.
        backing.edges[index].flags |= SETTLED | (outcome as u64) << 3;
        backing.nodes[consumer].counts[0] -= 1;
        if matches!(
            outcome,
            Demand::Missing | Demand::RequiredAbsence | Demand::Refused
        ) {
            backing.nodes[consumer].flags |= MISSING;
        }
        if settles {
            if backing.nodes[consumer].flags & (MISSING | INVALID) == MISSING {
                backing.nodes[consumer].flags |= TERMINAL | COMPLETE | EVIDENCE;
                backing.nodes[consumer].links[5] = backing.nodes[consumer].links[0];
            }
            refresh(backing, consumer);
        }
    }
    backing.nodes[slot].links[5] = edge.links[1];
    if changes {
        refresh(backing, slot);
    }
    advance(backing, slot);
    Ok(Control::Notified(index))
}
/// P covers E, both N endpoints, up to four incidence neighbours, H and both full paths.
fn release_one(backing: &mut Backing, slot: usize, release: &mut impl FnMut(usize)) -> Control {
    let index = backing.nodes[slot].links[1];
    let edge = backing.edges[index];
    let producer = edge.endpoints[0].slot;
    for direction in 0..2 {
        let endpoint = edge.endpoints[direction].slot;
        let previous = edge.links[2 * direction];
        let next = edge.links[2 * direction + 1];
        if previous == NONE {
            backing.nodes[endpoint].links[direction] = next;
        } else {
            backing.edges[previous].links[2 * direction + 1] = next;
        }
        if next != NONE {
            backing.edges[next].links[2 * direction] = previous;
        }
    }
    if backing.nodes[producer].links[5] == index {
        backing.nodes[producer].links[5] = edge.links[1];
    }
    backing.nodes[producer].counts[1] -= 1;
    backing.edges[index].flags |= RELEASED;
    release(edge.demand); // Original native owned-pin drop is already funded, with no fallible suffix.
    refresh(backing, producer);
    refresh(backing, slot);
    advance(backing, slot);
    Control::Released(index)
}
/// P covers the node/free-list/generation, header cursor and its full index clear.
fn retire(backing: &mut Backing, slot: usize) -> Control {
    let node = &mut backing.nodes[slot];
    node.flags = 0;
    if let Some(generation) = node.generation.checked_add(1) {
        node.generation = generation;
        node.links[4] = backing.control.free[0];
        backing.control.free[0] = slot;
    }
    bitmap::set(&mut backing.ready, backing.nodes.len(), slot, false);
    advance(backing, slot);
    Control::Retired(slot)
}
