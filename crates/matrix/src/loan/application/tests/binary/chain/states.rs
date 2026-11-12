// SPDX-License-Identifier: AGPL-3.0-only
//! Publication, detached heap destruction and last-pin destruction preserve prepaid release order.

use super::super::super::ready;
use super::super::leave;
use super::{TestResult, admit, candidate, prefix, published, request};
use crate::loan::fault::{self, Injection};
use crate::{ApplicationError, ApplicationPublication, Elementwise, LoanError};
use std::rc::Rc;

/// The late shell is the only publication allocation; its unwind never repeats scalar evaluation.
#[test]
fn chain_late_shell_unwind_releases_input_before_witness() -> TestResult {
    for reciprocal in [false, true] {
        let (mut heap, mut account, tile, _) = prefix(1, reciprocal)?;
        let function = if reciprocal {
            Elementwise::Reciprocal
        } else {
            Elementwise::SquareRoot
        };
        let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
        let owner = candidate(
            ready(admit(&mut heap, request, function, &mut account))?,
            &mut account,
        )?;
        let ticket = owner.ticket();
        let source = Rc::downgrade(&owner.core.input.as_ref().ok_or("input")?.record);
        fault::watch_source_release(ticket.identity.receipt, source.clone());
        fault::reset_scalars();
        let before = account.usage().work;
        let injection = Injection::unwinding_after(0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _result = heap.publish_application(owner, &mut account);
            }))
            .is_err()
        );
        assert_eq!(
            account.usage().work - before,
            if reciprocal { 233 } else { 193 }
        );
        assert_eq!(fault::scalars(), 0);
        assert_eq!(source.strong_count(), 0);
        assert_eq!(fault::source_at_witness(), Some(0));
        drop(injection);
        assert!(matches!(
            heap.reconcile_detached(&mut account, 100_000, None),
            crate::ScanOutcome::Complete
        ));
        assert!(
            matches!(heap.take_producer_outcome(ticket, &mut account)?, crate::ProducerOutcome::Absent(receipt) if receipt.reason() == crate::AbsenceReason::Abandoned)
        );
        assert_eq!(
            heap.take_producer_outcome(ticket, &mut account)?,
            crate::ProducerOutcome::AlreadyDelivered
        );
    }
    Ok(())
}

/// Ready and repair owners remain sufficient after heap destruction, with no fresh destructor Work.
#[test]
fn chain_heap_drop_then_detached_drop_or_unwind() -> TestResult {
    for repair in [false, true] {
        for unwind in [false, true] {
            let (mut heap, mut account, tile, _) = prefix(1, true)?;
            let request = request(
                &mut heap,
                tile.into_pin(),
                Elementwise::Reciprocal,
                2,
                &mut account,
            )?;
            let mut owner = ready(admit(
                &mut heap,
                request,
                Elementwise::Reciprocal,
                &mut account,
            ))?;
            if repair {
                owner = candidate(owner, &mut account)?;
                let injection = Injection::after(0);
                owner = match heap.publish_application(owner, &mut account) {
                    ApplicationPublication::ApplicationRepair(
                        ApplicationError::Loan(LoanError::AllocationRefused),
                        owner,
                    ) => owner,
                    other => return Err(format!("{other:?}").into()),
                };
                drop(injection);
            }
            let source = Rc::downgrade(&owner.core.input.as_ref().ok_or("input")?.record);
            fault::watch_source_release(owner.ticket().identity.receipt, source.clone());
            drop(heap);
            leave(&mut account, 0)?;
            let before = account.usage().work;
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _owner = owner;
                        panic!("chain owner after heap unwind");
                    }))
                    .is_err()
                );
            } else {
                drop(owner);
            }
            assert_eq!(account.usage().work, before);
            assert_eq!(source.strong_count(), 0);
            assert_eq!(fault::source_at_witness(), Some(0));
        }
    }
    Ok(())
}

/// A registry-only chain result admits a new current pin and a last historical pin outlives the heap.
#[test]
fn chain_registry_only_result_and_last_pin_release() -> TestResult {
    for unwind in [false, true] {
        let (mut heap, mut account, tile, _) = prefix(1, true)?;
        let request = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::Reciprocal,
            2,
            &mut account,
        )?;
        let owner = candidate(
            ready(admit(
                &mut heap,
                request,
                Elementwise::Reciprocal,
                &mut account,
            ))?,
            &mut account,
        )?;
        let (tile, receipt) = published(heap.publish_application(owner, &mut account))?;
        drop(tile);
        let pin = heap.acquire_pin::<f64>(receipt.ticket(), &mut account)?;
        drop(heap);
        assert!(
            pin.historical_reader(&mut account)?
                .read_component(crate::HostChannel::Center, 0, 0, &mut account)?
                .is_finite()
        );
        let source = Rc::downgrade(&pin.record);
        let before = account.usage().work;
        if unwind {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _pin = pin;
                    panic!("chain last pin unwind");
                }))
                .is_err()
            );
        } else {
            drop(pin);
        }
        assert_eq!(source.strong_count(), 0);
        assert_eq!(account.usage().work, before);
    }
    Ok(())
}

/// Ordinary readers keep 26/4 for binary origins and 25/3 for chained unary origins.
#[test]
fn chain_current_historical_and_selected_reader_work() -> TestResult {
    for unary in [false, true] {
        let (heap, mut account, tile, _) = prefix(1, unary)?;
        let before = account.usage().work;
        let reader = tile.reader(&heap, &mut account)?;
        assert_eq!(account.usage().work - before, if unary { 25 } else { 26 });
        let before = account.usage().work;
        assert!(reader.contains(0, &mut account)?);
        let _outcome = reader.application_outcome(0, &mut account)?;
        let _center = reader.read_component(crate::HostChannel::Center, 0, 0, &mut account)?;
        assert_eq!(account.usage().work - before, 3);
        drop(heap);
        let before = account.usage().work;
        let _reader = tile.pin().historical_reader(&mut account)?;
        assert_eq!(account.usage().work - before, if unary { 3 } else { 4 });
    }
    Ok(())
}

/// Every detached state blocks immediate-source reclaim, even after all older ancestors retire.
#[test]
fn chain_immediate_source_is_pinned_in_every_detached_state() -> TestResult {
    for state in 0..5 {
        let (mut heap, mut account, tile, _) = prefix(1, true)?;
        let source = tile.pin().ticket()?;
        let request = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::Reciprocal,
            2,
            &mut account,
        )?;
        let owner = ready(admit(
            &mut heap,
            request,
            Elementwise::Reciprocal,
            &mut account,
        ))?;
        let owner = super::prefixes::phase(&mut heap, &mut account, owner, state)?;
        let mut observer = crate::ExecutionAccount::new(nrz_bindings::Budget::new(
            nrz_bindings::Limits::default(),
        )?)?;
        assert!(matches!(
            heap.reconcile_detached(&mut observer, 100_000, None),
            crate::ScanOutcome::Complete
        ));
        assert!(matches!(
            heap.reclaim(&mut observer, 100_000, None),
            crate::ScanOutcome::Complete
        ));
        assert!(heap.loans.group(source.identity).is_ok());
        leave(&mut account, 0)?;
        assert!(matches!(
            heap.cancel_application(owner, &mut account),
            crate::ApplicationCancellation::ApplicationCancelled(_)
        ));
        assert!(matches!(
            heap.reconcile_detached(&mut observer, 100_000, None),
            crate::ScanOutcome::Complete
        ));
        assert!(matches!(
            heap.reclaim(&mut observer, 100_000, None),
            crate::ScanOutcome::Complete
        ));
        assert!(heap.loans.group(source.identity).is_err());
    }
    Ok(())
}

/// Independent continuation adds one admission pin scan and two publication debit visits.
#[test]
fn chain_account_transition_admission_and_publication_work() -> TestResult {
    for reciprocal in [false, true] {
        let (mut heap, _setup, tile, _) = prefix(3, reciprocal)?;
        let ticket = tile.pin().ticket()?;
        drop(tile);
        let mut account = crate::ExecutionAccount::new(nrz_bindings::Budget::new(
            nrz_bindings::Limits::default(),
        )?)?;
        let input = heap.acquire_pin::<f64>(ticket, &mut account)?;
        let function = if reciprocal {
            Elementwise::Reciprocal
        } else {
            Elementwise::SquareRoot
        };
        let request = request(&mut heap, input, function, 2, &mut account)?;
        let before = account.usage().work;
        let owner = ready(admit(&mut heap, request, function, &mut account))?;
        assert_eq!(
            account.usage().work - before,
            if reciprocal { 447 } else { 377 }
        );
        let owner = candidate(owner, &mut account)?;
        let before = account.usage().work;
        let _tile = published(heap.publish_application(owner, &mut account))?;
        assert_eq!(
            account.usage().work - before,
            if reciprocal { 249 } else { 209 }
        );
    }
    Ok(())
}
