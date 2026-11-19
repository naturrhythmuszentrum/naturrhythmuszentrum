// SPDX-License-Identifier: AGPL-3.0-only
//! Each extra specialized use funds one header, one bundle edge and its own weak debit.
use super::super::super::LcUsePayload;
use super::{LcPublishedResult, Use};
use crate::heap::resources::{ResourcePlan, ResourcePolicy};
use crate::loan::{
    charge,
    preparation::Preparation,
    rc_bytes,
    registry::{AllocationKind, CleanupReceipt, Debit, DebitOwner, LcDebitIdentity, Token},
};
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

impl<P: Scalar + 'static> LcPublishedResult<P> {
    /// Create a separately accounted owning use without copying the sealed law or numerical storage.
    /// # Errors
    /// Refuses foreign heaps/accounts, exhausted Work/bytes, allocation failure or stale use debits.
    pub fn retain(
        &self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Self, LoanError> {
        let payload = self.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
        account.authenticate(payload.account)?;
        if self.ticket().identity.scope != heap.scope {
            return Err(LoanError::ForeignOwner);
        }
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "LC use issue", |budget| {
                acquire(heap, self, budget)
            })
    }
    /// Consume this use with its already funded release, including on an exhausted account.
    /// # Errors
    /// Foreign-account refusal returns this same use and spends zero Work.
    /// # Panics
    /// Panics only if an internal invariant admits an incomplete published shell.
    pub fn release(self, account: &mut ExecutionAccount) -> Result<(), (Self, LoanError)> {
        if let Err(error) =
            account.authenticate(self.record.payload.as_ref().expect("published use").account)
        {
            return Err((self, error));
        }
        drop(self);
        Ok(())
    }
}
/// Fund a closed use lifetime and preserve every acquired prefix on refusal.
pub(super) fn acquire<P: Scalar + 'static>(
    heap: &mut Heap,
    source: &LcPublishedResult<P>,
    budget: &mut nrz_bindings::Budget,
) -> Result<LcPublishedResult<P>, LoanError> {
    charge(budget, 1)?;
    let s = source
        .bundle()
        .storage
        .as_ref()
        .ok_or(LoanError::WrongLoan)?;
    let checks = s
        .closure
        .rows
        .len()
        .checked_mul(28)
        .and_then(|n| n.checked_add(16))
        .ok_or(LoanError::CountOverflow)?;
    charge(budget, checks)?;
    super::validate::sealed(source.bundle())?;
    if s.identity.format != crate::storage::Format::of::<P>() {
        return Err(LoanError::WrongLoan);
    }
    super::readers::capture::<P>(s)?;
    authenticate(heap, source)?;
    charge(budget, heap.loans.debits.len())?;
    let [slot] = heap.loans.vacant_inventory::<1>(1)?;
    let capacity = slot
        .checked_add(1)
        .ok_or(LoanError::CountOverflow)?
        .max(heap.loans.debits.capacity());
    let (growth, old) = crate::loan::lifecycle::admission::growth(&heap.loans.debits, capacity)?;
    let retained = rc_bytes::<Use>()
        .checked_add(growth)
        .ok_or(LoanError::ByteOverflow)?;
    let scratch = old.checked_mul(2).ok_or(LoanError::ByteOverflow)?;
    charge(budget, heap.resource_work()?)?;
    let mut permit = heap.preflight_resources(
        ResourcePlan {
            retained,
            scratch,
            ..ResourcePlan::default()
        },
        ResourcePolicy::from_budget(budget),
    )?;
    permit.account = Some(s.identity.account);
    let revision = heap.loans.next_revision()?;
    charge(budget, 1)?;
    let key = crate::heap::identity::identity()?;
    let receipt = key;
    let issued = std::num::NonZeroU64::new(key).ok_or(LoanError::WrongLoan)?;
    let replacements = usize::from(growth != 0);
    charge(budget, 8 + 2 * replacements)?;
    permit.storage(heap.storage.epoch())?;
    let mut guard = Preparation::new(CleanupReceipt {
        identity: receipt,
        allocations: 1 + replacements,
        records: 1,
        edges: 2,
    });
    crate::loan::lifecycle::admission::grow_tracked(&mut heap.loans.debits, capacity, &mut guard)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let record = Rc::new(Use {
        payload: Some(LcUsePayload {
            identity: key,
            account: s.identity.account,
            bundle: Rc::clone(source.bundle()),
            token: Token { slot, key, receipt },
        }),
    });
    guard.acquired(false)?;
    // This fixed use lifetime has no variable destruction counts; its unique token
    // carries the receipt identity after the acquired-prefix guard completes.
    let cleanup = guard.finish()?;
    debug_assert_eq!(cleanup.identity, receipt);
    heap.loans.install(
        slot,
        Debit {
            kind: AllocationKind::LcUse {
                witness: Rc::downgrade(&record),
                receipt: Some(issued),
            },
            owner: DebitOwner::LcApplication(LcDebitIdentity::of(s.identity)),
            key,
            elements: 0,
            bytes: rc_bytes::<Use>(),
        },
    );
    heap.loans.revision = revision;
    Ok(LcPublishedResult {
        record,
        marker: PhantomData,
    })
}
/// Match the issued receipt or initial admission receipt and the exact weak witness.
fn authenticate<P: Scalar>(heap: &Heap, source: &LcPublishedResult<P>) -> Result<(), LoanError> {
    let p = source.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    let s = p.bundle.storage.as_ref().ok_or(LoanError::WrongLoan)?;
    let record = heap
        .loans
        .debits
        .get(p.token.slot)
        .and_then(Option::as_ref)
        .ok_or(LoanError::WrongLoan)?;
    let AllocationKind::LcUse { witness, receipt } = &record.kind else {
        return Err(LoanError::WrongLoan);
    };
    if p.token.receipt != receipt.map_or(s.identity.receipt, std::num::NonZeroU64::get)
        || receipt.is_some_and(|identity| identity.get() != record.key)
        || p.identity != p.token.key
        || p.account != s.identity.account
        || record.key != p.token.key
        || record.owner != DebitOwner::LcApplication(LcDebitIdentity::of(s.identity))
        || witness.as_ptr() != Rc::as_ptr(&source.record)
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}
