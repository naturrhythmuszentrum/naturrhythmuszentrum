// SPDX-License-Identifier: AGPL-3.0-only
//! Completed shell witnesses expire independently; storage's liveness remains the final payload edge.
use crate::loan::{
    charge,
    registry::{AllocationKind, DebitOwner},
};
use crate::{Heap, LoanError};
/// Neither witness is upgraded or interpreted as an ordinary pin-index entry.
pub(in crate::loan) fn debit(
    heap: &mut Heap,
    slot: usize,
    budget: &mut nrz_bindings::Budget,
    _changed: &mut bool,
) -> Result<(), LoanError> {
    let record = heap
        .loans
        .debits
        .get(slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    let DebitOwner::LcApplication(identity) = record.owner else {
        return Err(LoanError::WrongLoan);
    };
    if identity.scope != heap.scope
        || identity.account == 0
        || record.key == 0
        || record.elements != 0
    {
        return Err(LoanError::WrongLoan);
    }
    let expired = match &record.kind {
        AllocationKind::LcBundle(witness) => witness.strong_count() == 0,
        AllocationKind::LcUse { witness, receipt } => {
            if receipt.is_some_and(|receipt| receipt.get() != record.key) {
                return Err(LoanError::WrongLoan);
            }
            witness.strong_count() == 0
        }
        AllocationKind::Numerical
        | AllocationKind::Tracking
        | AllocationKind::Owner
        | AllocationKind::Pin
        | AllocationKind::Filled
        | AllocationKind::PreparedOwner { .. }
        | AllocationKind::PreparedUse(_)
        | AllocationKind::DyadicOwner { .. }
        | AllocationKind::DyadicUse(_)
        | AllocationKind::Constant(_)
        | AllocationKind::LcPending { .. }
        | AllocationKind::LcClosure(_)
        | AllocationKind::LcRows(_) => return Err(LoanError::WrongLoan),
    };
    if expired {
        let key = record.key;
        let revision = heap.loans.next_revision()?;
        charge(budget, 1)?;
        heap.loans.release(slot, key);
        heap.loans.revision = revision;
    }
    Ok(())
}
