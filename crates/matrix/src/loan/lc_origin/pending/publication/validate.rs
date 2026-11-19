// SPDX-License-Identifier: AGPL-3.0-only
//! Publication checks current sources once; readers authenticate immutable seals without cell scans.
use super::super::super::{LcCertainty, LcSeal};
use super::super::{Bundle, LcApplicationPending, Storage};
use crate::loan::{
    charge,
    payload::bit,
    registry::{AllocationKind, DebitOwner, LcDebitIdentity, State, Token},
};
use crate::mask::storage::Native;
use crate::{Coverage, Heap, HostChannel, LoanError, NumericalErrorScope};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Pointer identities are locators inside a retained seal, never external proof authority.
pub(super) fn address<T>(pointer: *const T) -> Result<u64, LoanError> {
    u64::try_from(pointer as usize).map_err(|_| LoanError::CountOverflow)
}
/// Fixed guards, every positional source, partition and attachment precede any mutation.
pub(super) fn pending<P: Scalar + 'static, const N: usize>(
    owner: &LcApplicationPending<'_, P, N>,
    heap: &Heap,
    budget: &mut nrz_bindings::Budget,
) -> Result<(LcSeal, u64), LoanError> {
    if owner.identity.scope != heap.scope {
        return Err(LoanError::ForeignOwner);
    }
    charge(budget, 51)?;
    let native = owner.native.as_ref().ok_or(LoanError::WrongLoan)?;
    let s = &native.storage;
    super::super::check::pending(heap, owner.identity, &s.liveness)?;
    if Rc::strong_count(&native.bundle) != 1
        || Rc::weak_count(&native.bundle) != 0
        || Rc::strong_count(&native.initial_use) != 1
        || Rc::weak_count(&native.initial_use) != 0
        || native.bundle.storage.is_some()
        || native.bundle.seal.is_some()
        || native.initial_use.payload.is_some()
    {
        return Err(LoanError::WrongLoan);
    }
    let cells = s.layout.axis.capacity();
    let words = cells.div_ceil(64);
    if s.identity != owner.identity
        || s.schedule.format != crate::storage::Format::of::<P>()
        || s.schedule.revisions != [1; 4]
        || s.closure.rows.len() != N
        || s.demand != owner.demand
        || s.demand_mask.layout != s.demand.layout()
        || s.demand_layout.axis != s.layout.axis
        || s.completed.layout != s.identity.layout
        || s.layout.components.extent() != crate::Extent::new(cells, 1)?
        || s.demand_mask.words.len() != words
        || s.completed.words.len() != words
        || s.outcomes.len() != cells
    {
        return Err(LoanError::LayoutMismatch);
    }
    if s.cleanup.identity != s.identity.receipt {
        return Err(LoanError::WrongLoan);
    }
    s.cleanup
        .edges
        .checked_add(5)
        .ok_or(LoanError::CountOverflow)?;
    tokens(heap, s)?;
    let revision = heap.loans.terminal_revision()?;
    let bulk = N
        .checked_mul(91)
        .and_then(|n| cells.checked_mul(8)?.checked_add(n))
        .and_then(|n| n.checked_add(words.checked_mul(2)?))
        .and_then(|n| n.checked_add(9 + 26))
        .ok_or(LoanError::CountOverflow)?;
    charge(budget, bulk)?;
    for (index, row) in s.closure.rows.iter().enumerate() {
        let original = owner
            .inventory
            .input(index)
            .ok_or(LoanError::TargetMismatch)?;
        if row.position != index
            || row.schema != original.specialization_key()
            || row.axes != s.closure.rows[0].axes
            || row.axes.combined.axis() != s.layout.axis
        {
            return Err(LoanError::TargetMismatch);
        }
        super::super::check::source(heap, row)?;
    }
    attachments(s)?;
    partition(s)?;
    Ok((seal(s)?, revision))
}
/// All nineteen token locators are checked before the two shell witnesses redirect.
fn tokens(heap: &Heap, s: &Storage) -> Result<(), LoanError> {
    let d = &s.debits;
    for token in d.numerical.iter().chain(&d.tracking).chain([
        &d.storage,
        &d.liveness,
        &d.layout,
        &d.outcomes,
        &d.completed_words,
        &d.completed_header,
        &d.bundle,
        &d.initial_use,
    ]) {
        let token = token.as_ref().ok_or(LoanError::WrongLoan)?;
        let record = debit(heap, token)?;
        if record.owner != DebitOwner::LcApplication(LcDebitIdentity::of(s.identity))
            || token.receipt != s.identity.receipt
        {
            return Err(LoanError::WrongLoan);
        }
        let AllocationKind::LcPending { witness, .. } = &record.kind else {
            return Err(LoanError::WrongLoan);
        };
        if witness.as_ptr() != Rc::as_ptr(&s.liveness) {
            return Err(LoanError::WrongLoan);
        }
    }
    for (token, header) in [(&d.closure, true), (&d.rows, false)] {
        let token = token.as_ref().ok_or(LoanError::WrongLoan)?;
        let record = debit(heap, token)?;
        let expected = super::super::super::LcJoinIdentity {
            scope: s.identity.scope,
            account: s.identity.account,
            join: s.closure.identity,
        };
        if record.owner != DebitOwner::LcJoin(expected) || token.receipt != expected.join {
            return Err(LoanError::WrongLoan);
        }
        let ((AllocationKind::LcClosure(w), true) | (AllocationKind::LcRows(w), false)) =
            (&record.kind, header)
        else {
            return Err(LoanError::WrongLoan);
        };
        if w.as_ptr() != Rc::as_ptr(&s.closure) {
            return Err(LoanError::WrongLoan);
        }
    }
    Ok(())
}
/// Authenticate the exact allocation locator before reading its role.
fn debit<'a>(heap: &'a Heap, token: &Token) -> Result<&'a crate::loan::registry::Debit, LoanError> {
    let record = heap
        .loans
        .debits
        .get(token.slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    if record.key != token.key {
        return Err(LoanError::WrongLoan);
    }
    Ok(record)
}
/// Validate all three scalar and initialization storage geometries.
fn attachments(s: &Storage) -> Result<(), LoanError> {
    let cells = s.layout.axis.capacity();
    for channel in &s.channels {
        if !channel.numerical.matches(s.identity.format, cells)
            || channel.components.len() != cells.div_ceil(64)
            || channel.complete.len() != cells.div_ceil(64)
        {
            return Err(LoanError::LayoutMismatch);
        }
    }
    Ok(())
}
/// Each word is synthesized during its only outcome pass; tail bits must be zero.
fn partition(s: &Storage) -> Result<(), LoanError> {
    for (word, chunk) in s.outcomes.chunks(64).enumerate() {
        let mut filled = 0_u64;
        let demand_word = s.demand_mask.words[word];
        for (offset, record) in chunk.iter().enumerate() {
            let cell = word * 64 + offset;
            let selected = demand_word & (1 << offset) != 0;
            super::outcomes::decode(*record, cell)?;
            if (record.status == 0) == selected {
                return Err(LoanError::CoverageMismatch);
            }
            let ready = record.status == 2;
            for channel in &s.channels {
                if bit(&channel.components, cell) != ready || bit(&channel.complete, cell) != ready
                {
                    return Err(LoanError::CoverageMismatch);
                }
            }
            if ready {
                filled |= 1 << offset;
            }
        }
        if s.completed.words[word] != filled
            || (chunk.len() < 64 && demand_word >> chunk.len() != 0)
        {
            return Err(LoanError::CoverageMismatch);
        }
    }
    Ok(())
}
/// Bind immutable storage identities and the fixed assurance.
fn seal(s: &Storage) -> Result<LcSeal, LoanError> {
    Ok(LcSeal {
        identity: s.identity,
        join: s.closure.identity,
        correspondence: address(Rc::as_ptr(&s.correspondence))?,
        closure: address(Rc::as_ptr(&s.closure))?,
        schedule: s.schedule.clone(),
        demand: s.demand,
        outcomes: address(s.outcomes.as_ptr())?,
        completed: s.identity.mask,
        coverage: Coverage::RepresentedInputs,
        error_scope: NumericalErrorScope::LocalOperation,
        certainty: LcCertainty::Undecided,
    })
}
/// Fixed seal verification has no logical-cell or mask-word traversal.
pub(super) fn sealed(bundle: &Bundle) -> Result<&Storage, LoanError> {
    let s = bundle.storage.as_ref().ok_or(LoanError::WrongLoan)?;
    if bundle.seal != Some(seal(s)?) {
        return Err(LoanError::WrongLoan);
    }
    Ok(s)
}
/// Current reading authenticates only the live output occupation, never current sources.
pub(in crate::loan) fn current(heap: &Heap, bundle: &Rc<Bundle>) -> Result<(), LoanError> {
    let s = bundle.storage.as_ref().ok_or(LoanError::WrongLoan)?;
    if s.identity.scope != heap.scope {
        return Err(LoanError::ForeignOwner);
    }
    let State::LcPublished(actual) = &heap.loans.group(s.identity)?.state else {
        return Err(LoanError::StaleOccupation);
    };
    if !Rc::ptr_eq(actual, bundle) {
        return Err(LoanError::WrongLoan);
    }
    for (channel, key) in HostChannel::ALL.into_iter().zip(s.identity.channels) {
        let crate::heap::descriptors::DescriptorRecord::Managed(record) = heap
            .descriptors
            .slots
            .get(key.ok_or(LoanError::WrongLoan)?)?
        else {
            return Err(LoanError::StaleOccupation);
        };
        if record.identity != s.identity || record.channel != channel || !record.filled {
            return Err(LoanError::WrongLoan);
        }
    }
    if !matches!(heap.masks.layouts.get(s.identity.layout.key)?, Native::Published { owner, managed: true } if Rc::ptr_eq(owner, &s.layout))
        || !matches!(heap.masks.masks.get(s.identity.mask.key)?, Native::Published { owner, managed: true } if Rc::ptr_eq(owner, &s.completed))
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}
