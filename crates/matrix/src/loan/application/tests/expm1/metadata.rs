// SPDX-License-Identifier: AGPL-3.0-only
//! Every performed metadata prefix has one typed refusal before any acquired output prefix.
use super::{Fixture, TestResult, fixture};
use crate::loan::fault::Injection;
use crate::{ApplicationError, Expm1ApplicationRequest, LoanError};
use nrz_bindings::readings::algebra::{Exhaustion, FaultKind, limits::BudgetKind};

/// Independently enumerate the C=3,k=1,h=1 performed visits in their public order.
/// Three law visits; account/current pin and source checks; six full shape entries;
/// declaration/attachment checks; plan/vacancy/resource scans; one full cleanup/preparation
/// and initialization prepayment; finally attachment validation plus cancellation.
const CHARGES: [u64; 24] = [
    3,
    1,
    1,
    1,
    1,
    6,
    2,
    1,
    3,
    1,
    1,
    1,
    1,
    1,
    1,
    4,
    4,
    1,
    9,
    9,
    1,
    21,
    (36 + 32 + 21 + 27) + (32 + 22 + 4) + 5 * (3 + 1 + 1) + 5 + 3,
    6,
];

/// No failed metadata prefix can spend a partial prepayment, allocate or consume the original request.
#[test]
fn prepared_every_admission_metadata_work_prefix() -> TestResult {
    assert_eq!(CHARGES.iter().sum::<u64>(), 287);
    for available in 0..287 {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(3, 1, [-2.0, 0.0], 2, false)?;
        let identity = request.input.ticket()?;
        let binding = crate::loan::retained_expm1::Binding::of(&request.law)?;
        let mut paid = 0;
        let mut refused = 0;
        for amount in CHARGES {
            if paid + amount > available {
                refused = amount;
                break;
            }
            paid += amount;
        }
        let ceiling = account.limits().work;
        account
            .ledger
            .work(ceiling - account.usage().work - available, crate::loan::AT)?;
        let before = account.usage().work;
        let injection = Injection::after(0);
        let returned = match heap.request_pointwise_expm1(request, &mut account) {
            Expm1ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::Budget(fault)),
                returned,
            ) => {
                assert_eq!(
                    fault.kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::Work,
                        limit: ceiling,
                        needed: before + paid + refused
                    }
                );
                assert_eq!(fault.location, crate::loan::AT);
                returned
            }
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(returned.input.ticket()?, identity);
        assert_eq!(
            crate::loan::retained_expm1::Binding::of(&returned.law)?,
            binding
        );
        assert_eq!(account.usage().work, before + paid);
        assert_eq!(Injection::attempts(), 0);
        assert_eq!(heap.loans.pending, 0);
        drop(injection);
    }
    Ok(())
}
/// Unrelated native tables consume the same ceiling without changing it after setup.
#[test]
fn prepared_native_table_boundaries_with_unrelated_occupations() -> TestResult {
    for layouts in [false, true] {
        for short in [0, 1] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = super::fixture_limits(
                1,
                1,
                [-2.0, 0.0],
                2,
                false,
                crate::HeapLimits {
                    descriptors: 8 - short,
                    ..crate::HeapLimits::default()
                },
            )?;
            for _ in 0..if layouts { 6 } else { 5 } {
                if layouts {
                    heap.create_cell_layout(
                        request.destination.axis,
                        crate::Layout::contiguous(crate::Extent::new(1, 1)?)?,
                        &mut account.ledger,
                    )?;
                } else {
                    heap.create_mask(request.demand.layout(), &[0], &mut account.ledger)?;
                }
            }
            let injection = Injection::after(usize::MAX);
            match heap.request_pointwise_expm1(request, &mut account) {
                Expm1ApplicationRequest::ApplicationReady(owner) => {
                    assert_eq!(short, 0);
                    drop(owner);
                }
                Expm1ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
                    _,
                ) => {
                    assert_eq!(short, 1);
                    assert_eq!(Injection::attempts(), 0);
                }
                other @ Expm1ApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
            drop(injection);
        }
    }
    Ok(())
}
/// One live source plus two legacy entries and five new outputs require eight aggregate elements.
#[test]
fn prepared_aggregate_elements_include_unrelated_legacy_storage() -> TestResult {
    for short in [0, 1] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = super::fixture_limits(
            1,
            1,
            [-2.0, 0.0],
            2,
            false,
            crate::HeapLimits {
                elements: 8 - short,
                ..crate::HeapLimits::default()
            },
        )?;
        heap.import_represented(
            crate::Extent::new(2, 1)?,
            &nrz_bindings::Descriptor::ratio(),
            &[7.0, 8.0],
        )?;
        let injection = Injection::after(usize::MAX);
        match heap.request_pointwise_expm1(request, &mut account) {
            Expm1ApplicationRequest::ApplicationReady(owner) => {
                assert_eq!(short, 0);
                drop(owner);
            }
            Expm1ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::Heap(crate::HeapError::Capacity)),
                _,
            ) => {
                assert_eq!(short, 1);
                assert_eq!(Injection::attempts(), 0);
            }
            other @ Expm1ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
        drop(injection);
    }
    Ok(())
}
