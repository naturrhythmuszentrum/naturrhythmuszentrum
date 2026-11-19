// SPDX-License-Identifier: AGPL-3.0-only
//! The liveness header stays charged until all other retaining witnesses are gone.
use crate::loan::{
    charge,
    registry::{
        AbsenceReason, AllocationKind, CancellationReceipt, DebitOwner, Identity, LoanTicket,
        ProducerOutcome, State,
    },
};
use crate::{Heap, LoanError};
use nrz_bindings::Budget;

/// Only LC dispatch retires these reservations; the ordinary pin index is inapplicable.
pub(super) fn terminal(
    heap: &mut Heap,
    identity: Identity,
    revision: u64,
    reason: AbsenceReason,
) -> Result<CancellationReceipt, LoanError> {
    let group = heap
        .loans
        .groups
        .get_mut(identity.root.slot)
        .and_then(Option::as_mut)
        .ok_or(LoanError::StaleOccupation)?;
    let receipt = CancellationReceipt {
        ticket: LoanTicket { identity },
        reason,
    };
    group.state = State::Retiring;
    group.outcome = ProducerOutcome::Absent(receipt);
    group.delivered = reason == AbsenceReason::Cancelled;
    drop(heap.masks.layouts.remove(identity.layout.key));
    drop(heap.masks.masks.remove(identity.mask.key));
    heap.loans.pending -= 1;
    heap.loans.revision = revision;
    Ok(receipt)
}

/// Expiration is terminal absence, never a numerical result or inferred missing input.
pub(in crate::loan) fn abandoned(
    heap: &mut Heap,
    slot: usize,
    budget: &mut Budget,
) -> Result<(), LoanError> {
    let Some(group) = heap.loans.groups.get(slot).and_then(Option::as_ref) else {
        return Ok(());
    };
    let State::LcOut(witness) = &group.state else {
        return Ok(());
    };
    if witness.strong_count() != 0 {
        return Ok(());
    }
    let identity = group.identity;
    let _span = tracing::trace_span!(
        "nrz.retire",
        family = "lc-origin",
        application = identity.request,
        reason = "abandoned"
    )
    .entered();
    charge(budget, 6)?;
    super::check::occupation(heap, identity)?;
    let revision = heap.loans.terminal_revision()?;
    terminal(heap, identity, revision, AbsenceReason::Abandoned)?;
    Ok(())
}

/// A complete debit-slot walk sees even reversed inventories and leaves deferred headers charged.
fn other_witnesses(
    heap: &Heap,
    identity: crate::loan::registry::LcDebitIdentity,
    header_slot: usize,
    budget: &mut Budget,
) -> Result<bool, LoanError> {
    charge(budget, 1)?;
    if heap
        .loans
        .groups
        .get(identity.root.slot)
        .and_then(Option::as_ref)
        .is_some_and(|group| {
            crate::loan::registry::LcDebitIdentity::of(group.identity) == identity
                && matches!(group.state, State::LcOut(_))
        })
    {
        return Ok(true);
    }
    for (slot, entry) in heap.loans.debits.iter().enumerate() {
        charge(budget, 1)?;
        let other = slot != header_slot
            && entry.as_ref().is_some_and(|debit| {
                debit.owner == DebitOwner::LcApplication(identity)
                    && matches!(debit.kind, AllocationKind::LcPending { .. })
            });
        if other {
            return Ok(true);
        }
    }
    Ok(false)
}

/// No weak upgrade or payload access is needed to release a dead allocation's conservative debit.
pub(in crate::loan) fn debit(
    heap: &mut Heap,
    slot: usize,
    budget: &mut Budget,
    changed: &mut bool,
) -> Result<(), LoanError> {
    let Some(record) = heap.loans.debits.get(slot).and_then(Option::as_ref) else {
        return Ok(());
    };
    let AllocationKind::LcPending { witness, header } = &record.kind else {
        return Err(LoanError::WrongLoan);
    };
    let DebitOwner::LcApplication(identity) = record.owner else {
        return Err(LoanError::WrongLoan);
    };
    if identity.scope != heap.scope || identity.account == 0 || record.key == 0 {
        return Err(LoanError::WrongLoan);
    }
    if witness.strong_count() != 0 {
        return Ok(());
    }
    if *header && other_witnesses(heap, identity, slot, budget)? {
        *changed = true;
        return Ok(());
    }
    let key = record.key;
    let revision = heap.loans.next_revision()?;
    charge(budget, 1)?;
    heap.loans.release(slot, key);
    heap.loans.revision = revision;
    Ok(())
}
