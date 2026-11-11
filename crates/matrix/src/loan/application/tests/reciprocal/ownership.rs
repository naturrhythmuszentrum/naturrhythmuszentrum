// SPDX-License-Identifier: AGPL-3.0-only
//! Actual destruction sentinels distinguish retained edges from final numerical owner release.

use super::{TestResult, candidate, fixture, published, ready};
use crate::loan::fault::{self, Injection};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationPublication, HostChannel, LoanError,
};
use nrz_bindings::Limits;

/// Independent acquisition sites locate the three rounded numerical backings in every prefix.
#[test]
fn reciprocal_all_preparation_unwinds_release_exact_acquired_prefix() -> TestResult {
    for prefix in 0..30 {
        let mut setup = fixture::<f64>(2, 2, 3, 2, false, Limits::default())?;
        let source = std::rc::Rc::downgrade(&setup.request.input.record);
        let bytes = setup.heap.resource_bytes()?;
        let released = fault::numerical_releases();
        let before = setup.account.usage().work;
        let injection = Injection::unwinding_after(prefix);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _result = setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account);
        }));
        assert!(result.is_err());
        assert_eq!(Injection::attempts(), prefix + 1);
        assert_eq!(setup.account.usage().work - before, 280);
        super::super::prefixes::check(&setup.heap, prefix, bytes)?;
        let slabs = [10, 13, 16]
            .into_iter()
            .filter(|site| *site < prefix)
            .count();
        assert_eq!(
            fault::numerical_releases(),
            [
                released[0] + slabs,
                released[1] + 4 * slabs,
                released[2] + 64 * slabs
            ]
        );
        assert_eq!(source.strong_count(), 0);
        assert_eq!(setup.heap.loans.pending, 0);
        drop(injection);
    }
    Ok(())
}
/// A detached owner is sufficient after the heap facade ends, with no new destructor Work.
#[test]
fn reciprocal_detached_and_repair_owners_after_heap_drop() -> TestResult {
    for phase in 0..3 {
        for unwind in [false, true] {
            let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
            let mut owner = ready(
                setup
                    .heap
                    .request_pointwise_reciprocal(setup.request, &mut setup.account),
            )?;
            if phase > 0 {
                owner = candidate(owner, &mut setup.account)?;
            }
            if phase == 2 {
                let injection = Injection::after(0);
                owner = match setup.heap.publish_application(owner, &mut setup.account) {
                    ApplicationPublication::ApplicationRepair(
                        ApplicationError::Loan(LoanError::AllocationRefused),
                        owner,
                    ) => owner,
                    other => return Err(format!("{other:?}").into()),
                };
                drop(injection);
            }
            drop(setup.heap);
            let before = setup.account.usage().work;
            let released = fault::numerical_releases();
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _owner = owner;
                        panic!("reciprocal detached owner unwind");
                    }))
                    .is_err()
                );
            } else {
                drop(owner);
            }
            assert_eq!(
                fault::numerical_releases(),
                [released[0] + 4, released[1] + 4, released[2] + 256]
            );
            assert_eq!(setup.account.usage().work, before);
        }
    }
    Ok(())
}
/// Every detached phase has already funded its terminal cancellation, even with no evaluation Work.
#[test]
fn reciprocal_every_detached_phase_zero_work_cancellation() -> TestResult {
    for phase in 0..3 {
        let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
        let mut owner = ready(
            setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account),
        )?;
        if phase > 0 {
            owner = candidate(owner, &mut setup.account)?;
        }
        if phase == 2 {
            let injection = Injection::after(0);
            owner = match setup.heap.publish_application(owner, &mut setup.account) {
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::AllocationRefused),
                    owner,
                ) => owner,
                other => return Err(format!("{other:?}").into()),
            };
            drop(injection);
        }
        let ticket = owner.ticket();
        setup.account.ledger.work(
            setup.account.limits().work - setup.account.usage().work,
            crate::loan::AT,
        )?;
        let before = setup.account.usage().work;
        assert!(
            matches!(setup.heap.cancel_application(owner, &mut setup.account), ApplicationCancellation::ApplicationCancelled(receipt) if receipt.ticket() == ticket)
        );
        assert_eq!(setup.account.usage().work, before);
        assert_eq!(setup.heap.loans.pending, 0);
    }
    Ok(())
}
/// A registry-only result can issue a new pin, which then suffices after heap destruction.
#[test]
fn reciprocal_registry_only_and_historical_result_retention() -> TestResult {
    for unwind in [false, true] {
        let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
        let owner = candidate(
            ready(
                setup
                    .heap
                    .request_pointwise_reciprocal(setup.request, &mut setup.account),
            )?,
            &mut setup.account,
        )?;
        let (tile, receipt) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        assert!(matches!(
            setup.heap.reclaim(&mut setup.account, 100_000, None),
            crate::ScanOutcome::Complete
        ));
        drop(tile);
        let pin = setup
            .heap
            .acquire_pin::<f64>(receipt.ticket(), &mut setup.account)?;
        drop(setup.heap);
        assert_eq!(
            pin.historical_reader(&mut setup.account)?.read_component(
                HostChannel::Center,
                0,
                0,
                &mut setup.account
            )?,
            0.5
        );
        let released = fault::numerical_releases();
        let before = setup.account.usage().work;
        if unwind {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _pin = pin;
                    panic!("reciprocal historical result unwind");
                }))
                .is_err()
            );
        } else {
            drop(pin);
        }
        assert_eq!(
            fault::numerical_releases(),
            [released[0] + 3, released[1] + 3, released[2] + 192]
        );
        assert_eq!(setup.account.usage().work, before);
    }
    Ok(())
}
