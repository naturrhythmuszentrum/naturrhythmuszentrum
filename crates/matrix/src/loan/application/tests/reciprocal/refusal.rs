// SPDX-License-Identifier: AGPL-3.0-only
//! Full-shape and representation exclusions preserve the exact request before allocation.

use super::super::refusal::replace_source;
use super::{TestResult, fixture};
use crate::{ApplicationError, ApplicationRequest, Elementwise, LoanError, StructureScope};
use nrz_bindings::Limits;

/// Reuse structural witnesses, then check the reciprocal descriptor law over those same shapes.
fn structural(
    request: &mut crate::PointwiseRequest<f64>,
    output: bool,
    entry: usize,
    scope: StructureScope,
) -> TestResult {
    super::super::refusal::structural(request, output, entry, scope)?;
    request.operation = super::seals::operation(&request.operation, Elementwise::Reciprocal)?;
    Ok(())
}
/// Structural refusal is independent of numerical satisfaction and selected demand.
fn zeros() -> TestResult {
    for scope in [StructureScope::CenterOnly, StructureScope::WholeFamily] {
        for output in [false, true] {
            for (cells, components) in [(1, 1), (3, 3)] {
                for entry in [0, cells * components - 1] {
                    for selection in 0..3 {
                        for payload in 0..3 {
                            let mut setup = fixture::<f64>(
                                cells,
                                components,
                                3,
                                selection,
                                false,
                                Limits::default(),
                            )?;
                            if payload > 0 {
                                replace_source(&mut setup, |_, _| {
                                    (0.0, if payload == 1 { 1.0 } else { 0.0 })
                                })?;
                            }
                            structural(&mut setup.request, output, entry, scope)?;
                            let ticket = setup.request.input.ticket()?;
                            let pointer = std::rc::Rc::as_ptr(&setup.request.input.record);
                            let before = setup.account.usage().work;
                            let injection = crate::loan::fault::Injection::after(0);
                            crate::loan::fault::reset_reads();
                            crate::loan::fault::reset_scalars();
                            let result = setup
                                .heap
                                .request_pointwise_reciprocal(setup.request, &mut setup.account);
                            let request = match result {
                                ApplicationRequest::ApplicationRefused(
                                    ApplicationError::UnsupportedStructure {
                                        output: actual,
                                        entry: index,
                                        scope: actual_scope,
                                    },
                                    request,
                                ) => {
                                    assert_eq!(
                                        (actual, index, actual_scope),
                                        (output, entry, scope)
                                    );
                                    request
                                }
                                other => return Err(format!("{other:?}").into()),
                            };
                            assert_eq!(request.input.ticket()?, ticket);
                            assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
                            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                            assert_eq!(crate::loan::fault::reads(), 0);
                            assert_eq!(crate::loan::fault::scalars(), 0);
                            // Request authentication 1, current source 3h+12, then the actual metadata prefix.
                            assert_eq!(
                                setup.account.usage().work - before,
                                u64::try_from(
                                    22 + usize::from(output) * cells * components + entry + 1
                                )?
                            );
                            drop(injection);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
/// Full-shape assertions refuse before all allocations and numerical access in both formats.
/// ´test:unit:reciprocal-structural-exclusion-both-sides-scopes-and-demands´
#[test]
fn reciprocal_structural_exclusion_both_sides_scopes_and_demands() -> TestResult {
    zeros()
}
/// Exhaustion immediately before a metadata visit preserves the original request and pin.
/// ´test:unit:reciprocal-structural-work-prefixes-preserve-request´
#[test]
fn reciprocal_structural_work_prefixes_preserve_request() -> TestResult {
    for output in [false, true] {
        for entry in [0, 5] {
            let mut setup = fixture::<f64>(2, 3, 3, 0, false, Limits::default())?;
            structural(
                &mut setup.request,
                output,
                entry,
                StructureScope::WholeFamily,
            )?;
            let prefix = 22 + usize::from(output) * 6 + entry;
            let spend =
                setup.account.limits().work - setup.account.usage().work - u64::try_from(prefix)?;
            setup.account.ledger.work(spend, crate::loan::AT)?;
            let ticket = setup.request.input.ticket()?;
            let injection = crate::loan::fault::Injection::after(0);
            match setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account)
            {
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    request,
                ) => assert_eq!(request.input.ticket()?, ticket),
                other => return Err(format!("{other:?}").into()),
            }
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            drop(injection);
        }
    }
    Ok(())
}
/// Unsupported family and coverage refuse at their original checks without allocating output.
#[test]
fn reciprocal_entry_point_and_coverage_refusals() -> TestResult {
    for choice in 0..4 {
        let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
        match choice {
            0 | 1 => {
                setup.request.operation = super::seals::operation(
                    &setup.request.operation,
                    if choice == 0 {
                        Elementwise::Square
                    } else {
                        Elementwise::SquareRoot
                    },
                )?;
            }
            2 => {
                let prepared = super::super::expm1::fixture(1, 1, [-2.0, 0.0], 2, false)?;
                setup.request.operation = prepared.request.operation.operation;
            }
            3 => setup.request.destination.coverage = crate::Coverage::DeclaredBounds,
            _ => return Err("choice".into()),
        }
        let pointer = std::rc::Rc::as_ptr(&setup.request.input.record);
        let ticket = setup.request.input.ticket()?;
        let injection = crate::loan::fault::Injection::after(0);
        crate::loan::fault::reset_reads();
        crate::loan::fault::reset_scalars();
        let request = match setup
            .heap
            .request_pointwise_reciprocal(setup.request, &mut setup.account)
        {
            ApplicationRequest::ApplicationRefused(error, request) => {
                if choice == 3 {
                    assert!(matches!(
                        error,
                        ApplicationError::Loan(LoanError::CoverageMismatch)
                    ));
                } else {
                    assert!(matches!(
                        error,
                        ApplicationError::Loan(LoanError::UnsupportedContract)
                    ));
                }
                request
            }
            other @ ApplicationRequest::ApplicationReady(_) => {
                return Err(format!("{other:?}").into());
            }
        };
        assert_eq!(request.input.ticket()?, ticket);
        assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
        assert_eq!(
            [
                crate::loan::fault::reads(),
                crate::loan::fault::scalars(),
                crate::loan::fault::Injection::attempts()
            ],
            [0; 3]
        );
        drop(injection);
    }
    // Matching physical declarations still refuse: equal coverage does not admit this member.
    let mut setup = super::super::fixture_coverage::<f64>(
        1,
        1,
        3,
        2,
        false,
        Limits::default(),
        crate::Coverage::DeclaredBounds,
    )?;
    setup.request.operation =
        super::seals::operation(&setup.request.operation, Elementwise::Reciprocal)?;
    let injection = crate::loan::fault::Injection::after(0);
    assert!(matches!(
        setup
            .heap
            .request_pointwise_reciprocal(setup.request, &mut setup.account),
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::CoverageMismatch),
            _
        )
    ));
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    Ok(())
}
