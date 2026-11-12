// SPDX-License-Identifier: AGPL-3.0-only
//! Every acquired prefix and detached state releases its immediate edge before its final witness.

use super::super::{super::ready, leave};
use super::{TestResult, admit, candidate, prefix, request};
use crate::loan::fault::{self, Injection};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationEvaluation, ApplicationPublication,
    ApplicationRequest, Elementwise, LoanError, ProducerOutcome,
};
use std::rc::Rc;

/// Six replacements, nine buffers, four shells, three extras and eight preparation sites total thirty.
#[test]
fn chain_every_preparation_failure_and_unwind_preserves_inventory() -> TestResult {
    for reciprocal in [false, true] {
        for unwind in [false, true] {
            for site in 0..30 {
                preparation(reciprocal, unwind, site)?;
            }
        }
    }
    Ok(())
}

/// The ordinary common plan funds the complete prefix before any acquired output storage.
fn preparation(reciprocal: bool, unwind: bool, site: usize) -> TestResult {
    let (mut heap, mut account, tile, _) = prefix(3, reciprocal)?;
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
    let source = Rc::downgrade(&request.input.record);
    let before = account.usage().work;
    let injection = if unwind {
        Injection::unwinding_after(site)
    } else {
        Injection::after(site)
    };
    if unwind {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _result = admit(&mut heap, request, function, &mut account);
            }))
            .is_err()
        );
    } else {
        let request = match admit(&mut heap, request, function, &mut account) {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(
                    LoanError::AllocationRefused | LoanError::Heap(crate::HeapError::Capacity),
                ),
                request,
            ) => request,
            other => return Err(format!("site {site}: {other:?}").into()),
        };
        assert_eq!(Rc::as_ptr(&request.input.record), source.as_ptr());
        drop(request);
    }
    assert_eq!(
        account.usage().work - before,
        if reciprocal { 446 } else { 376 }
    );
    assert_eq!(Injection::attempts(), site + 1);
    assert_eq!(source.strong_count(), 0);
    assert_eq!(heap.loans.pending, 0);
    drop(injection);
    Ok(())
}

/// Ready, not-started, rejected, candidate and repair carry the same exact source pin.
pub(super) fn phase(
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    mut owner: crate::OwnedApplication<f64>,
    phase: usize,
) -> TestResult<crate::OwnedApplication<f64>> {
    match phase {
        0 => {}
        1 => {
            leave(account, 1)?;
            owner = match owner.evaluate(account) {
                ApplicationEvaluation::ApplicationNotStarted(_, owner) => owner,
                other => return Err(format!("{other:?}").into()),
            };
        }
        2 => {
            owner = match heap.publish_application(owner, account) {
                ApplicationPublication::ApplicationRejected(_, owner) => owner,
                other => return Err(format!("{other:?}").into()),
            };
        }
        3 | 4 => {
            owner = candidate(owner, account)?;
            if phase == 4 {
                let injection = Injection::after(0);
                owner = match heap.publish_application(owner, account) {
                    ApplicationPublication::ApplicationRepair(
                        ApplicationError::Loan(LoanError::AllocationRefused),
                        owner,
                    ) => owner,
                    other => return Err(format!("{other:?}").into()),
                };
                drop(injection);
            }
        }
        _ => return Err("phase".into()),
    }
    Ok(owner)
}

/// Every state cancels without remaining Work, or abandons exactly once on ordinary drop or unwind.
#[test]
fn chain_all_detached_states_cancel_drop_and_unwind() -> TestResult {
    for reciprocal in [false, true] {
        for state in 0..5 {
            for ending in 0..3 {
                detached(reciprocal, state, ending)?;
            }
        }
    }
    Ok(())
}

/// An independent extra pin proves that releasing the downstream edge cannot consume another use.
fn detached(reciprocal: bool, state: usize, ending: usize) -> TestResult {
    let (mut heap, mut account, tile, _) = prefix(1, reciprocal)?;
    let extra = heap.retain_pin(tile.pin(), &mut account)?;
    let function = if reciprocal {
        Elementwise::Reciprocal
    } else {
        Elementwise::SquareRoot
    };
    let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
    let owner = ready(admit(&mut heap, request, function, &mut account))?;
    let source = Rc::downgrade(&owner.core.input.as_ref().ok_or("source")?.record);
    let ticket = owner.ticket();
    let witness = Rc::downgrade(&owner.core.host.witness);
    let owner = phase(&mut heap, &mut account, owner, state)?;
    fault::watch_source_release(ticket.identity.receipt, source.clone());
    if ending == 0 {
        leave(&mut account, 0)?;
        let before = account.usage().work;
        assert!(matches!(
            heap.cancel_application(owner, &mut account),
            ApplicationCancellation::ApplicationCancelled(_)
        ));
        assert_eq!(account.usage().work, before);
    } else if ending == 1 {
        drop(owner);
    } else {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _owner = owner;
                panic!("chain detached owner unwind");
            }))
            .is_err()
        );
    }
    assert_eq!(source.strong_count(), 0);
    assert_eq!(witness.strong_count(), 0);
    assert_eq!(fault::source_at_witness(), Some(0));
    assert_eq!(Rc::strong_count(&extra.record), 1);
    observe_terminal(&mut heap, &mut account, ticket, state, ending)?;
    drop(extra);
    Ok(())
}

/// Reconciliation has its own ledger; delivery remains exclusively on the original producer account.
fn observe_terminal(
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    ticket: crate::LoanTicket,
    state: usize,
    ending: usize,
) -> TestResult {
    // A separate scan account observes prepaid destruction; it never funds the exhausted producer.
    let mut observer =
        crate::ExecutionAccount::new(nrz_bindings::Budget::new(nrz_bindings::Limits::default())?)?;
    assert!(matches!(
        heap.reconcile_detached(&mut observer, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    let group = heap.loans.group(ticket.identity)?;
    if ending == 0 {
        assert!(group.delivered);
        assert!(
            matches!(group.outcome, ProducerOutcome::Absent(receipt) if receipt.reason() == crate::AbsenceReason::Cancelled)
        );
    } else {
        assert!(!group.delivered);
        assert!(
            matches!(group.outcome, ProducerOutcome::Absent(receipt) if receipt.reason() == crate::AbsenceReason::Abandoned)
        );
        if state != 1 {
            assert!(
                matches!(heap.take_producer_outcome(ticket, account)?, ProducerOutcome::Absent(receipt) if receipt.reason() == crate::AbsenceReason::Abandoned)
            );
            assert_eq!(
                heap.take_producer_outcome(ticket, account)?,
                ProducerOutcome::AlreadyDelivered
            );
        }
    }
    Ok(())
}
