// SPDX-License-Identifier: AGPL-3.0-only
//! Defensive final-record corruption tests the publication boundary, never an intermediate oracle.
use super::LcApplicationEvaluated;
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_scalar::Scalar;
/// Corrupt one final record or attachment and retain the same refused owner on both checks.
pub(in crate::loan) fn partition<P: Scalar + 'static>(
    mut candidate: LcApplicationEvaluated<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    case: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = &mut candidate
        .owner
        .native
        .as_mut()
        .expect("evaluated owner")
        .storage;
    match case {
        0 => storage.outcomes[0].status = 1,
        1 => storage.outcomes[0].status = 6,
        2 => {
            storage.outcomes[0] = super::super::super::LcOutcome {
                status: 3,
                target: 3,
                reason: 1,
            }
        }
        3 => {
            storage.outcomes[0] = super::super::super::LcOutcome {
                status: 5,
                target: 4,
                reason: 0x0102,
            }
        }
        4 => {
            storage.outcomes[0] = super::super::super::LcOutcome {
                status: 5,
                target: 5,
                reason: 0x0103,
            }
        }
        5 => {
            storage.outcomes[0] = super::super::super::LcOutcome {
                status: 5,
                target: 3,
                reason: 0x0206,
            }
        }
        6 => {
            storage.outcomes[1] = super::super::super::LcOutcome {
                status: 2,
                target: 0,
                reason: 0,
            }
        }
        7 => storage.channels[0].components[0] &= !1,
        8 => {
            std::rc::Rc::get_mut(&mut storage.completed)
                .expect("private filled mask")
                .words[0] &= !1;
        }
        9 => {
            storage.channels[2].complete.pop();
        }
        10 => {
            // Replace only the logical numerical shape; masks and terminal records remain valid.
            storage.channels[2].numerical = crate::loan::payload::Numerical::zeroed::<P>(0)?;
        }
        _ => return Err("unknown corruption case".into()),
    }
    let ticket = candidate.ticket();
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let injection = crate::loan::fault::Injection::after(0);
    for _ in 0..2 {
        let before = account.usage().work;
        let failure = candidate
            .publish(heap, account)
            .expect_err("malformed terminal partition");
        if case >= 9 {
            assert!(
                matches!(failure.error, LoanError::LayoutMismatch),
                "{:?}",
                failure.error
            );
        } else {
            assert!(
                matches!(failure.error, LoanError::CoverageMismatch),
                "{:?}",
                failure.error
            );
        }
        assert_eq!(account.usage().work - before, 639);
        assert_eq!(failure.owner.ticket(), ticket);
        candidate = failure.owner;
    }
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::scalars(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    candidate.cancel(heap, account)?;
    Ok(())
}

/// Apply a real caller-retained reservation to the same original ledger, without replacing limits.
pub(in crate::loan) fn staged<P: Scalar + 'static>(
    result: &super::LcPublishedResult<P>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    spare: bool,
    short: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes: u64 = if spare { 64 } else { 64 + 264 + 2 * 94 * 264 };
    let caller = account.limits().staged_payload_bytes - bytes + short;
    let before = heap.resource_bytes()?;
    let injection = crate::loan::fault::Injection::after(usize::MAX);
    let retained = account
        .ledger
        .with_retained_payload(caller, crate::loan::AT, |budget| {
            super::uses::acquire(heap, result, budget)
        });
    if short == 0 {
        let retained = retained?;
        assert_eq!(
            crate::loan::fault::Injection::attempts(),
            if spare { 1 } else { 2 }
        );
        assert_eq!(
            heap.resource_bytes()? - before,
            if spare { 64 } else { 328 }
        );
        retained.release(account).map_err(|(_, error)| error)?;
    } else {
        assert!(retained.is_err());
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(heap.resource_bytes()?, before);
    }
    drop(injection);
    Ok(())
}

/// Exactly two shell debits redirect; all fifteen storage witnesses retain the original liveness.
pub(in crate::loan) fn published<P: Scalar>(result: &super::LcPublishedResult<P>, heap: &Heap) {
    use crate::loan::registry::{AllocationKind, DebitOwner, LcDebitIdentity};
    let bundle = result.bundle();
    let storage = bundle.storage.as_ref().expect("published storage");
    let identity = DebitOwner::LcApplication(LcDebitIdentity::of(storage.identity));
    let mut roles = [0; 3];
    for record in heap
        .loans
        .debits
        .iter()
        .flatten()
        .filter(|record| record.owner == identity)
    {
        match &record.kind {
            AllocationKind::LcPending { witness, .. } => {
                assert_eq!(witness.as_ptr(), std::rc::Rc::as_ptr(&storage.liveness));
                roles[0] += 1;
            }
            AllocationKind::LcBundle(witness) => {
                assert_eq!(witness.as_ptr(), std::rc::Rc::as_ptr(bundle));
                roles[1] += 1;
            }
            AllocationKind::LcUse {
                witness,
                receipt: None,
            } => {
                assert_eq!(witness.as_ptr(), std::rc::Rc::as_ptr(&result.record));
                roles[2] += 1;
            }
            _ => panic!("unexpected initial published debit"),
        }
    }
    assert_eq!(roles, [15, 1, 1]);
    assert_eq!(std::rc::Rc::weak_count(&storage.liveness), 15);
    assert_eq!(std::rc::Rc::strong_count(&storage.liveness), 1);
    assert_eq!(storage.cleanup.edges, 30);
    assert_eq!(std::rc::Rc::strong_count(bundle), 2);
    assert_eq!(std::rc::Rc::weak_count(bundle), 1);
    let payload = result.record.payload.as_ref().expect("initial use");
    assert_eq!(payload.token.receipt, storage.identity.receipt);
}

/// A mismatched issued receipt cannot be retained or reconciled away after its owner expires.
pub(in crate::loan) fn receipt<P: Scalar + 'static>(
    result: &super::LcPublishedResult<P>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::loan::registry::AllocationKind;
    let extra = result.retain(heap, account)?;
    let payload = extra.record.payload.as_ref().expect("issued use");
    let slot = payload.token.slot;
    assert_eq!(payload.identity, payload.token.key);
    assert_eq!(payload.token.receipt, payload.token.key);
    let key = payload.token.key;
    let AllocationKind::LcUse { receipt, .. } =
        &mut heap.loans.debits[slot].as_mut().expect("use debit").kind
    else {
        return Err("use debit kind".into());
    };
    *receipt = std::num::NonZeroU64::new(key.checked_add(1).ok_or("key overflow")?);
    assert!(matches!(
        extra.retain(heap, account),
        Err(LoanError::WrongLoan)
    ));
    extra.release(account).map_err(|(_, error)| error)?;
    assert!(matches!(
        heap.reconcile_detached(account, 100_000, None),
        crate::ScanOutcome::Refused(LoanError::WrongLoan)
    ));
    assert_eq!(
        heap.loans.debits[slot]
            .as_ref()
            .expect("retained conservative debit")
            .bytes,
        64
    );
    Ok(())
}
