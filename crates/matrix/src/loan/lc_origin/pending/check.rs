// SPDX-License-Identifier: AGPL-3.0-only
//! Authentication reads immutable metadata only; captured values are never loaded here.
use super::super::{LcJoinIdentity, LcLiveness, LcSourceRow};
use crate::loan::registry::{AllocationKind, DebitInventory, DebitOwner, Identity, State, Token};
use crate::{Heap, HostChannel, LcOmegaPrepared, LoanError, MaskVersion};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Foreign-account refusal precedes all Work and allocation.
pub(super) fn account<P: Scalar, const N: usize>(
    prepared: &LcOmegaPrepared<'_, P, N>,
    account: u64,
) -> Result<(), LoanError> {
    let row = prepared
        .receipt
        .closure
        .rows
        .first()
        .ok_or(LoanError::UnsupportedContract)?;
    if account != row.source.account {
        return Err(LoanError::ForeignAccount);
    }
    Ok(())
}

/// Two existing tokens retain their original join identity, role, receipt and weak witness.
fn token(
    heap: &Heap,
    token: &Token,
    prepared: &super::super::LcClosure,
    account: u64,
    header: bool,
) -> Result<(), LoanError> {
    let debit = heap
        .loans
        .debits
        .get(token.slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    let identity = LcJoinIdentity {
        scope: heap.scope,
        account,
        join: prepared.identity,
    };
    let correct = token.receipt == identity.join
        && debit.owner == DebitOwner::LcJoin(identity)
        && debit.key == token.key
        && debit.elements == 0;
    if !correct {
        return Err(LoanError::WrongLoan);
    }
    let ((AllocationKind::LcClosure(witness), true) | (AllocationKind::LcRows(witness), false)) =
        (&debit.kind, header)
    else {
        return Err(LoanError::WrongLoan);
    };
    if witness.as_ptr() != std::ptr::from_ref(prepared) {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}

/// Nine fixed request and demand checks precede table planning.
pub(super) fn boundary<P: Scalar + 'static, const N: usize>(
    prepared: &LcOmegaPrepared<'_, P, N>,
    heap: &Heap,
    demand: MaskVersion,
) -> Result<(), LoanError> {
    let receipt = &prepared.receipt;
    let rows = &receipt.closure.rows;
    if rows.len() != N
        || receipt.join != receipt.closure.identity
        || receipt.schedule.positions != prepared.declaration.positions
        || receipt.schedule.specialization != prepared.declaration.key
        || receipt.schedule.format != crate::storage::Format::of::<P>()
        || receipt.schedule.revisions != [1; 4]
    {
        return Err(LoanError::TargetMismatch);
    }
    let first = rows.first().ok_or(LoanError::UnsupportedContract)?;
    let mask = heap.mask(demand, demand.layout())?;
    if mask.axis() != first.axes.combined.axis()
        || mask.gather().is_some()
        || mask.components().extent()
            != crate::Extent::new(first.axes.combined.axis().capacity(), 1)?
    {
        return Err(LoanError::LayoutMismatch);
    }
    Ok(())
}

/// Called only after the complete admission visit inventory was prepaid.
pub(super) fn all<P: Scalar + 'static, const N: usize>(
    prepared: &LcOmegaPrepared<'_, P, N>,
    heap: &Heap,
    account: u64,
) -> Result<(), LoanError> {
    let receipt = &prepared.receipt;
    let rows = &receipt.closure.rows;
    token(heap, &receipt.header, &receipt.closure, account, true)?;
    token(heap, &receipt.rows, &receipt.closure, account, false)?;
    let first = rows.first().ok_or(LoanError::UnsupportedContract)?;
    for (index, row) in rows.iter().enumerate() {
        if row.position != index
            || row.schema
                != prepared
                    .declaration
                    .inventory
                    .input(index)
                    .ok_or(LoanError::TargetMismatch)?
                    .specialization_key()
            || row.axes != first.axes
        {
            return Err(LoanError::TargetMismatch);
        }
        source(heap, row)?;
    }
    Ok(())
}

/// Reuse the original host/mapped checks, with their complete fixed tariff prepaid by admission.
fn source(heap: &Heap, row: &LcSourceRow) -> Result<(), LoanError> {
    let State::Published(original_owner) = &heap.loans.group(row.source)?.state else {
        return Err(LoanError::StaleDependency);
    };
    let original = original_owner
        .payload
        .as_ref()
        .ok_or(LoanError::WrongLoan)?;
    let pin = row.pin.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    let mapped = pin.bundle.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    let host = original.origin.evidence.is_none()
        && original.endpoints.is_none()
        && original.retained_law.is_none();
    let closed = mapped.endpoints.is_none()
        && mapped.retained_law.is_none()
        && original.channels.iter().flatten().count() == 3
        && mapped.channels.iter().flatten().count() == 3;
    if !host || !closed {
        return Err(LoanError::UnsupportedContract);
    }
    heap.current_lc_pair_prepaid(original, &row.pin)?;
    let source = mapped.origin.lc_source()?;
    if original.identity != row.source
        || mapped.identity != row.mapped
        || source.identity != row.source
        || original.identity.layout != row.layout
        || original.declaration.axis != row.axes.instances
        || mapped.declaration.axis != row.axes.combined.axis()
        || source.projection != row.projection
    {
        return Err(LoanError::TargetMismatch);
    }
    for (actual, expected) in [
        (&original.declaration.symbols, &row.symbols[0]),
        (&source.declaration.symbols, &row.symbols[0]),
        (&mapped.declaration.symbols, &row.symbols[1]),
    ] {
        if actual.value() != expected.value()
            || actual.bound() != expected.bound()
            || actual.error() != expected.error()
        {
            return Err(LoanError::TargetMismatch);
        }
    }
    for channel in HostChannel::ALL {
        let index = channel.index();
        let expected = row.companions[index];
        if mapped.qualified[index] != Some(expected)
            || expected.identity != row.mapped
            || expected.channel != channel
            || original.qualified[index] != source.attachments[index]
            || source.attachments[index].is_none()
        {
            return Err(LoanError::TargetMismatch);
        }
    }
    Ok(())
}

/// Closed pending occupations never enter ordinary host handback or pin-index dispatch.
pub(super) fn occupation(heap: &Heap, identity: Identity) -> Result<(), LoanError> {
    if identity.scope != heap.scope {
        return Err(LoanError::ForeignOwner);
    }
    let group = heap.loans.group(identity)?;
    if !matches!(group.state, State::LcOut(_))
        || !matches!(group.debits, DebitInventory::LcApplication(_))
    {
        return Err(LoanError::WrongLoan);
    }
    for (channel, key) in HostChannel::ALL.into_iter().zip(identity.channels) {
        let key = key.ok_or(LoanError::WrongLoan)?;
        let crate::heap::descriptors::DescriptorRecord::Managed(record) =
            heap.descriptors.slots.get(key)?
        else {
            return Err(LoanError::StaleOccupation);
        };
        if record.identity != identity || record.channel != channel || record.filled {
            return Err(LoanError::WrongLoan);
        }
    }
    if !matches!(heap.masks.layouts.get(identity.layout.key)?, crate::mask::storage::Native::Reserved(root) if *root == identity.root)
        || !matches!(heap.masks.masks.get(identity.mask.key)?, crate::mask::storage::Native::Reserved(root) if *root == identity.root)
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}

/// Cancellation checks its original weak witness without acquiring an additional edge.
pub(super) fn pending(
    heap: &Heap,
    identity: Identity,
    liveness: &Rc<LcLiveness>,
) -> Result<(), LoanError> {
    occupation(heap, identity)?;
    let State::LcOut(witness) = &heap.loans.group(identity)?.state else {
        return Err(LoanError::WrongLoan);
    };
    if witness.as_ptr() != Rc::as_ptr(liveness)
        || liveness.identity != identity
        || liveness.receipt != identity.receipt
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}
