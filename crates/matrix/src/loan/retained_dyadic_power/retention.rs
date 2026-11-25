// SPDX-License-Identifier: AGPL-3.0-only
//! Every additional dyadic use prepays its header and edge without copying the authority.

use super::{LawIdentity, PreparedDyadicPowerUseRecord, RetainedDyadicPower, admission::Plan};
use crate::{
    Heap,
    loan::{
        ExecutionAccount, LoanError, charge, rc_bytes,
        registry::{AllocationKind, CleanupReceipt, Debit, DebitOwner, Token},
    },
};
use std::rc::Rc;

/// Authenticate original heap/account, immutable seal and both live debit witnesses.
pub(super) fn authenticate(heap: &Heap, law: &RetainedDyadicPower) -> Result<(), LoanError> {
    let owner = law.owner()?;
    if owner.identity.scope != heap.scope {
        return Err(LoanError::ForeignOwner);
    }
    let frozen = law.frozen()?;
    if frozen.seal != law.record.binding.seal
        || frozen.seal.revisions[0] != 1
        || frozen.seal.revisions[2] != 0
        || frozen.seal.exponent != frozen.parts.exponent()
    {
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
    let exact_owner = owner_debit.owner == DebitOwner::Dyadic(owner.identity)
        && owner_debit.key == owner.token.key
        && owner.token.key == owner.identity.owner;
    let exact_use =
        use_debit.owner == DebitOwner::Dyadic(owner.identity) && use_debit.key == record.token.key;
    let receipts = owner.token.receipt == owner.cleanup.identity
        && record.token.receipt == record.cleanup.identity;
    if !exact_owner || !exact_use || !receipts {
        return Err(LoanError::WrongLoan);
    }
    let AllocationKind::DyadicOwner { witness, allowance } = &owner_debit.kind else {
        return Err(LoanError::WrongLoan);
    };
    if *allowance
        != owner
            .evidence
            .checked_add(owner.keys)
            .ok_or(LoanError::ByteOverflow)?
    {
        return Err(LoanError::WrongLoan);
    }
    let owner_edge = record.owner.as_ref().ok_or(LoanError::WrongLoan)?;
    if !witness.ptr_eq(&Rc::downgrade(owner_edge)) {
        return Err(LoanError::WrongLoan);
    }
    let AllocationKind::DyadicUse(witness) = &use_debit.kind else {
        return Err(LoanError::WrongLoan);
    };
    if !witness.ptr_eq(&Rc::downgrade(record)) {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}

impl Heap {
    /// Acquire one independently funded use on the original account.
    /// # Errors
    /// Refuses foreign identity, resource, revision or allocation exhaustion.
    pub fn retain_retained_dyadic_power(
        &mut self,
        law: &RetainedDyadicPower,
        account: &mut ExecutionAccount,
    ) -> Result<RetainedDyadicPower, LoanError> {
        let span = tracing::trace_span!(
            "nrz.retain",
            family = "dyadic-power",
            result = tracing::field::Empty,
            work_used = tracing::field::Empty,
            table_capacity = tracing::field::Empty
        );
        let _entered = span.enter();
        let before = account.usage().work;
        let result = law
            .owner()
            .and_then(|owner| account.authenticate(owner.identity.account))
            .and_then(|()| {
                account.ledger.with_work_phase(
                    crate::loan::FAMILY,
                    "retain dyadic power law",
                    |budget| acquire(self, law, account.identity, budget),
                )
            });
        span.record("work_used", account.usage().work.saturating_sub(before));
        span.record("table_capacity", self.loans.debits.capacity());
        span.record(
            "result",
            if result.is_ok() {
                "accepted"
            } else {
                "refused"
            },
        );
        result
    }

    /// Consume exactly one prepaid use, returning that same use on authentication refusal.
    /// # Errors
    /// A rejected release charges its attempted authentication and preserves the use.
    pub fn release_retained_dyadic_power(
        &mut self,
        law: RetainedDyadicPower,
        account: &mut ExecutionAccount,
    ) -> Result<(), (LoanError, RetainedDyadicPower)> {
        let span = tracing::trace_span!(
            "nrz.release",
            family = "dyadic-power",
            result = tracing::field::Empty,
            work_used = tracing::field::Empty
        );
        let _entered = span.enter();
        let before = account.usage().work;
        let result = law.owner().and_then(|owner| {
            account.authenticate(owner.identity.account)?;
            authenticate(self, &law)
        });
        if let Err(error) = result {
            let charged = account.ledger.with_work_phase(
                crate::loan::FAMILY,
                "rejected dyadic power release",
                |budget| charge(budget, 1),
            );
            span.record("work_used", account.usage().work.saturating_sub(before));
            span.record("result", "refused");
            return Err((charged.err().unwrap_or(error), law));
        }
        let result = account.ledger.with_work_phase(
            crate::loan::FAMILY,
            "prepaid dyadic power release",
            |_| {
                drop(law);
                Ok::<(), std::convert::Infallible>(())
            },
        );
        span.record("work_used", account.usage().work.saturating_sub(before));
        span.record("result", "released");
        match result {
            Ok(()) => Ok(()),
            Err(never) => match never {},
        }
    }
}

/// Further-use admission adds only its use header, table growth and disjoint temporaries.
fn acquire(
    heap: &mut Heap,
    law: &RetainedDyadicPower,
    account: u64,
    budget: &mut nrz_bindings::Budget,
) -> Result<RetainedDyadicPower, LoanError> {
    let owner = law.owner()?;
    if owner.identity.account != account {
        return Err(LoanError::ForeignAccount);
    }
    charge(budget, 4)?;
    authenticate(heap, law)?;
    let key = super::identity()?;
    let receipt = super::identity()?;
    let plan = Plan::new(heap, account, None, receipt, budget)?;
    let (mut guard, slots, revision) = plan.allocate(heap, account)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mut record = Rc::new(PreparedDyadicPowerUseRecord {
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
    Ok(RetainedDyadicPower { record })
}

/// A use debit owns only a non-upgradeable expiration witness.
pub(super) fn install_use(
    heap: &mut Heap,
    record: &Rc<PreparedDyadicPowerUseRecord>,
    identity: LawIdentity,
) {
    heap.loans.install(
        record.token.slot,
        Debit {
            kind: AllocationKind::DyadicUse(Rc::downgrade(record)),
            owner: DebitOwner::Dyadic(identity),
            key: record.token.key,
            elements: 0,
            bytes: rc_bytes::<PreparedDyadicPowerUseRecord>(),
        },
    );
}
