// SPDX-License-Identifier: AGPL-3.0-only
//! Acquired prefixes, all detached states and historical pins retain exactly their funded edges.
use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::loan::fault::Injection;
use crate::{ApplicationError, ApplicationPublication, Expm1ApplicationRequest, LoanError};

/// Every acquisition boundary includes both endpoint slabs and their four tracking backings.
/// Prefixes 3..9 are the descriptor/layout/mask key and replacement pairs: they map to `Capacity`.
/// All other 30 sites use the direct typed allocation refusal. Each site asserts its one cause.
#[test]
fn prepared_allocation_failure_prefixes_and_unwind() -> TestResult {
    for unwind in [false, true] {
        for prefix in 0..36 {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(2, 2, [-2.0, 0.0], 2, false)?;
            let released = crate::loan::fault::numerical_releases();
            let ticket = request.input.ticket()?;
            let binding = crate::loan::retained_expm1::Binding::of(&request.law)?;
            let injection = if unwind {
                Injection::unwinding_after(prefix)
            } else {
                Injection::after(prefix)
            };
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _result = heap.request_pointwise_expm1(request, &mut account);
                    }))
                    .is_err()
                );
            } else {
                let request = match heap.request_pointwise_expm1(request, &mut account) {
                    Expm1ApplicationRequest::ApplicationRefused(
                        ApplicationError::Loan(error),
                        request,
                    ) => {
                        if (3..9).contains(&prefix) {
                            assert!(matches!(error, LoanError::Heap(crate::HeapError::Capacity)));
                        } else {
                            assert!(matches!(error, LoanError::AllocationRefused));
                        }
                        request
                    }
                    other => return Err(format!("{other:?}").into()),
                };
                assert_eq!(request.input.ticket()?, ticket);
                assert_eq!(
                    crate::loan::retained_expm1::Binding::of(&request.law)?,
                    binding
                );
            }
            assert_eq!(Injection::attempts(), prefix + 1);
            let retained = match prefix {
                0..=3 => prefix,
                4 => 3,
                5 | 6 => 4,
                7 | 8 => 5,
                _ => 6,
            };
            assert_eq!(Injection::prefix(), [prefix, retained, prefix - retained]);
            let slabs = [10, 13, 16, 19, 22]
                .into_iter()
                .filter(|site| *site < prefix)
                .count();
            assert_eq!(
                crate::loan::fault::numerical_releases(),
                [
                    released[0] + slabs,
                    released[1] + 4 * slabs,
                    released[2] + 64 * slabs
                ]
            );
            assert_eq!(heap.loans.pending, 0);
            drop(injection);
        }
    }
    Ok(())
}
/// Ready, candidate and repair all release the law and source before their last witness expires.
#[test]
fn prepared_every_detached_state_drop_and_unwind() -> TestResult {
    for state in 0..3 {
        for unwind in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
            let mut owner = ready(heap.request_pointwise_expm1(request, &mut account))?;
            if state > 0 {
                owner = candidate(owner, &mut account)?;
            }
            if state == 2 {
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
            let destroyed = super::laws::destroyed();
            drop(heap);
            assert_eq!(super::laws::destroyed(), destroyed);
            let before = account.usage().work;
            let released = crate::loan::fault::numerical_releases();
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _owner = owner;
                        panic!("prepared application unwind fixture");
                    }))
                    .is_err()
                );
            } else {
                drop(owner);
            }
            assert_eq!(super::laws::destroyed(), destroyed + 1);
            assert_eq!(
                crate::loan::fault::numerical_releases(),
                [released[0] + 6, released[1] + 6, released[2] + 6 * 64]
            );
            assert_eq!(account.usage().work, before);
        }
    }
    Ok(())
}
/// Independent result readers survive source retirement, original facade drop and heap destruction.
#[test]
fn prepared_result_retention_source_retirement_and_reconciliation() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let source = request.input.ticket()?;
    let demand = request.demand;
    let owner = ready(heap.request_pointwise_expm1(request, &mut account))?;
    heap.retire_mask(demand, &mut account.ledger)?;
    let owner = candidate(owner, &mut account)?;
    let (tile, receipt) = published(heap.publish_application(owner, &mut account))?;
    let pin = heap.retain_pin(tile.pin(), &mut account)?;
    drop(tile);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 10_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(&mut account, 10_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(heap.acquire_pin::<f64>(source, &mut account).is_err());
    let current = heap.acquire_pin::<f64>(receipt.ticket(), &mut account)?;
    assert!(
        current
            .historical_reader(&mut account)?
            .expm1_endpoints(0, 0, &mut account)
            .is_ok()
    );
    drop(current);
    let destroyed = super::laws::destroyed();
    drop(heap);
    assert_eq!(super::laws::destroyed(), destroyed);
    assert!(
        pin.historical_reader(&mut account)?
            .expm1_endpoints(0, 0, &mut account)
            .is_ok()
    );
    drop(pin);
    assert_eq!(super::laws::destroyed(), destroyed + 1);
    Ok(())
}
