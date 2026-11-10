// SPDX-License-Identifier: AGPL-3.0-only
//! Each additional use shares one authority and prepays its own header and edge destruction.

use super::{LawIdentity, PreparedExpm1UseRecord, RetainedExpm1, admission::Plan};
use crate::{
    Heap,
    loan::{
        ExecutionAccount, LoanError, charge, rc_bytes,
        registry::{AllocationKind, CleanupReceipt, Debit, DebitOwner, Token},
    },
};
use std::rc::Rc;

/// Check exact sealed identities, receipts and both weak witnesses without upgrading either.
pub(super) fn authenticate(heap: &Heap, law: &RetainedExpm1) -> Result<(), LoanError> {
    let owner = law.owner()?;
    if owner.identity.scope != heap.scope {
        return Err(LoanError::ForeignOwner);
    }
    let frozen = law.frozen()?;
    if frozen.seal.revisions != [1, 1, 1, frozen.parts.authority.request().environment] {
        return Err(LoanError::WrongLoan);
    }
    let record = &law.record;
    if super::Binding::of(law)? != record.binding {
        return Err(LoanError::WrongLoan);
    }
    let owner_debit = heap
        .loans
        .debits
        .get(owner.token.slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    let use_debit = heap
        .loans
        .debits
        .get(record.token.slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    let exact_owner = owner_debit.owner == DebitOwner::Prepared(owner.identity)
        && owner_debit.key == owner.token.key
        && owner.token.key == owner.identity.owner;
    let exact_use = use_debit.owner == DebitOwner::Prepared(owner.identity)
        && use_debit.key == record.token.key;
    let receipts = owner.token.receipt == owner.cleanup.identity
        && record.token.receipt == record.cleanup.identity;
    if !exact_owner || !exact_use || !receipts {
        return Err(LoanError::WrongLoan);
    }
    let AllocationKind::PreparedOwner { witness, .. } = &owner_debit.kind else {
        return Err(LoanError::WrongLoan);
    };
    let owner_edge = record.owner.as_ref().ok_or(LoanError::WrongLoan)?;
    if !witness.ptr_eq(&Rc::downgrade(owner_edge)) {
        return Err(LoanError::WrongLoan);
    }
    let AllocationKind::PreparedUse(witness) = &use_debit.kind else {
        return Err(LoanError::WrongLoan);
    };
    if !witness.ptr_eq(&Rc::downgrade(record)) {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}
impl Heap {
    /// Admit a separately funded use on the original account without copying authority.
    /// # Errors
    /// Refuses foreign identities, byte/Work ceilings, revision or allocation exhaustion.
    pub fn retain_retained_expm1(
        &mut self,
        law: &RetainedExpm1,
        account: &mut ExecutionAccount,
    ) -> Result<RetainedExpm1, LoanError> {
        account.authenticate(law.owner()?.identity.account)?;
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "retain prepared law", |budget| {
                acquire(self, law, account.identity, budget)
            })
    }

    /// Consume exactly one prepaid use without requiring fresh Work for release.
    /// # Errors
    /// Charges rejected authentication to the caller and returns the unchanged use,
    /// including when that charge refuses for exhausted Work.
    pub fn release_retained_expm1(
        &mut self,
        law: RetainedExpm1,
        account: &mut ExecutionAccount,
    ) -> Result<(), (LoanError, RetainedExpm1)> {
        let result = law.owner().and_then(|owner| {
            account.authenticate(owner.identity.account)?;
            authenticate(self, &law)
        });
        if let Err(error) = result {
            let charged = account.ledger.with_work_phase(
                crate::loan::FAMILY,
                "rejected law release",
                |budget| charge(budget, 1),
            );
            return Err((charged.err().unwrap_or(error), law));
        }
        let result =
            account
                .ledger
                .with_work_phase(crate::loan::FAMILY, "prepaid law release", |_| {
                    drop(law);
                    Ok::<(), std::convert::Infallible>(())
                });
        match result {
            Ok(()) => Ok(()),
            Err(never) => match never {},
        }
    }
}
/// The same admission routine runs within any caller reservation on the original ledger.
pub(super) fn acquire(
    heap: &mut Heap,
    law: &RetainedExpm1,
    account: u64,
    budget: &mut nrz_bindings::Budget,
) -> Result<RetainedExpm1, LoanError> {
    let owner = law.owner()?;
    if owner.identity.account != account {
        return Err(LoanError::ForeignAccount);
    }
    charge(budget, 4)?; // Account authentication, owner seal and two debit visits.
    authenticate(heap, law)?;
    let key = super::identity()?;
    let receipt = super::identity()?;
    let plan = Plan::new(heap, account, None, receipt, budget)?;
    let (mut guard, slots, revision) = plan.allocate(heap, account)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mut record = Rc::new(PreparedExpm1UseRecord {
        owner: None,
        binding: super::Binding {
            identity: owner.identity,
            use_identity: key,
            seal: law.frozen()?.seal,
        },
        token: Token {
            slot: slots[0],
            key,
            receipt,
        },
        cleanup: CleanupReceipt::empty(receipt),
    });
    guard.acquired(false)?;
    let cleanup = guard.finish()?;
    let shell = Rc::get_mut(&mut record).ok_or(LoanError::WrongLoan)?;
    shell.cleanup = cleanup;
    shell.owner = Some(Rc::clone(
        law.record.owner.as_ref().ok_or(LoanError::WrongLoan)?,
    ));
    install_use(heap, &record, owner.identity);
    heap.loans.revision = revision;
    Ok(RetainedExpm1 { record })
}
/// One prepared use row owns only an expiration witness, never a strong law root.
pub(super) fn install_use(
    heap: &mut Heap,
    record: &Rc<PreparedExpm1UseRecord>,
    identity: LawIdentity,
) {
    heap.loans.install(
        record.token.slot,
        Debit {
            kind: AllocationKind::PreparedUse(Rc::downgrade(record)),
            owner: DebitOwner::Prepared(identity),
            key: record.token.key,
            elements: 0,
            bytes: rc_bytes::<PreparedExpm1UseRecord>(),
        },
    );
}
