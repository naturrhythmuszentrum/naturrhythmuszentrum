// SPDX-License-Identifier: AGPL-3.0-only
//! Native allowance is zero; each execution ceiling still admits the complete five-reading payload.
use super::{Fixture, TestResult, fixture, ready};
use crate::{ApplicationError, ExecutionLimits, Expm1ApplicationRequest, LoanError};

/// Work and scratch each independently admit five scalar outputs, including empty demand.
#[test]
fn prepared_execution_limits_and_elements() -> TestResult {
    for cells in [0, 1, 65] {
        for selection in [0, 2] {
            for limit in 0..3 {
                for short in [0, 1] {
                    if cells == 0 && limit != 0 && short != 0 {
                        continue;
                    }
                    let Fixture {
                        mut heap,
                        mut account,
                        mut request,
                    } = fixture(cells, 1, [-2.0, 0.0], selection, false)?;
                    let mut values = [1, 5 * cells, 5 * cells];
                    values[limit] -= short;
                    request.limits = ExecutionLimits::new(values[0], values[1], values[2]);
                    let injection = crate::loan::fault::Injection::after(usize::MAX);
                    match heap.request_pointwise_expm1(request, &mut account) {
                        Expm1ApplicationRequest::ApplicationReady(owner) => {
                            assert_eq!(short, 0);
                            assert_eq!(owner.core.contract.work, 0);
                            assert_eq!(owner.core.evidence.law.work, 0);
                            assert_eq!(heap.loans.elements()?, [0, 6 * cells]);
                        }
                        Expm1ApplicationRequest::ApplicationRefused(
                            ApplicationError::Operation(crate::OperationError::ResourceLimit),
                            _,
                        ) => {
                            assert_eq!(short, 1);
                            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                        }
                        other @ Expm1ApplicationRequest::ApplicationRefused(..) => {
                            return Err(format!("{other:?}").into());
                        }
                    }
                    drop(injection);
                }
            }
        }
    }
    Ok(())
}
/// The aggregate capacity is one live input plus five new logical outputs.
#[test]
fn prepared_aggregate_six_element_boundary() -> TestResult {
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
                elements: 6 - short,
                ..crate::HeapLimits::default()
            },
        )?;
        let injection = crate::loan::fault::Injection::after(usize::MAX);
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
                assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            }
            other @ Expm1ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
        drop(injection);
    }
    Ok(())
}
/// The prepared tag has no dense numerical adapter or physical scratch allocation route.
#[test]
fn prepared_dense_recipe_refuses() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let owner = ready(heap.request_pointwise_expm1(request, &mut account))?;
    let injection = crate::loan::fault::Injection::after(0);
    assert!(owner.core.contract.physical_scratch::<f64, f64>(0).is_err());
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    Ok(())
}
