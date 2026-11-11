// SPDX-License-Identifier: AGPL-3.0-only
//! Literal traversal and complete-plan boundaries are independent of production estimators.

use super::{Fixture, TestResult, candidate, fixture, ready};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationEvaluation, ApplicationRequest,
    ExecutionLimits, LoanError,
};
use nrz_bindings::Limits;

/// Exhaust only the original account; no replacement or replenished continuation is introduced.
fn leave(account: &mut crate::ExecutionAccount, remaining: u64) -> TestResult {
    let spend = account.limits().work - account.usage().work - remaining;
    account.ledger.work(spend, crate::loan::AT)?;
    Ok(())
}
/// Preserve every ready field and output bit pattern at the insufficient traversal boundary.
/// ´test:unit:reciprocal-evaluation-literal-work-boundaries-and-atomic-refusal´
#[test]
fn reciprocal_evaluation_literal_work_boundaries_and_atomic_refusal() -> TestResult {
    for (channels, selection, cells, required) in [
        (1, 2, 3, 57),
        (2, 2, 3, 60),
        (3, 2, 3, 60),
        (3, 0, 3, 3),
        (3, 0, 0, 1),
    ] {
        for succeeds in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(cells, 1, channels, selection, false, Limits::default())?;
            let owner = ready(heap.request_pointwise_reciprocal(request, &mut account))?;
            let ticket = owner.ticket();
            let pointer = std::ptr::from_ref(owner.core.as_ref());
            let filled = owner.core.filled.clone();
            let outcomes = format!("{:?}", owner.core.evidence.outcomes);
            leave(&mut account, required - u64::from(!succeeds))?;
            crate::loan::fault::reset_reads();
            crate::loan::fault::reset_scalars();
            let before = account.usage().work;
            let owner = match owner.evaluate(&mut account) {
                ApplicationEvaluation::ApplicationCandidate(owner) => {
                    assert!(succeeds);
                    owner
                }
                ApplicationEvaluation::ApplicationNotStarted(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    owner,
                ) => {
                    assert!(!succeeds);
                    assert!(!owner.core.candidate);
                    assert_eq!(owner.core.filled, filled);
                    assert_eq!(format!("{:?}", owner.core.evidence.outcomes), outcomes);
                    assert_eq!(crate::loan::fault::reads(), 0);
                    assert_eq!(crate::loan::fault::scalars(), 0);
                    for channel in owner.core.host.channels.iter().flatten() {
                        assert!(channel.components.iter().all(|word| *word == 0));
                        for entry in 0..cells {
                            assert_eq!(
                                channel.matrix.read::<f64>(entry)?.to_bits(),
                                0.0_f64.to_bits()
                            );
                        }
                    }
                    owner
                }
                other => return Err(format!("{other:?}").into()),
            };
            assert_eq!(owner.ticket(), ticket);
            assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
            assert_eq!(
                account.usage().work - before,
                if succeeds {
                    required
                } else {
                    u64::from(required > 1)
                }
            );
            assert!(matches!(
                heap.cancel_application(owner, &mut account),
                ApplicationCancellation::ApplicationCancelled(_)
            ));
        }
    }
    Ok(())
}
/// Execution work and scratch keep their existing shared maximum semantics independently.
/// ´test:unit:reciprocal-execution-ceilings-refuse-before-allocation´
#[test]
fn reciprocal_execution_ceilings_refuse_before_allocation() -> TestResult {
    for (cells, components, required) in [(1, 3, 36), (65, 1, 195)] {
        for ceiling in 0..3 {
            for succeeds in [false, true] {
                let Fixture {
                    mut heap,
                    mut account,
                    mut request,
                } = fixture::<f64>(cells, components, 3, 2, false, Limits::default())?;
                let slots = usize::from(ceiling != 0 || succeeds);
                let work = required - usize::from(ceiling == 1 && !succeeds);
                let scratch = required - usize::from(ceiling == 2 && !succeeds);
                request.limits = ExecutionLimits::new(slots, work, scratch);
                let ticket = request.input.ticket()?;
                let injection =
                    crate::loan::fault::Injection::after(if succeeds { usize::MAX } else { 0 });
                match heap.request_pointwise_reciprocal(request, &mut account) {
                    ApplicationRequest::ApplicationReady(owner) => {
                        assert!(succeeds);
                        assert!(matches!(
                            heap.cancel_application(owner, &mut account),
                            ApplicationCancellation::ApplicationCancelled(_)
                        ));
                    }
                    ApplicationRequest::ApplicationRefused(
                        ApplicationError::Operation(crate::OperationError::ResourceLimit),
                        request,
                    ) => {
                        assert!(!succeeds);
                        assert_eq!(request.input.ticket()?, ticket);
                        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                    }
                    other @ ApplicationRequest::ApplicationRefused(..) => {
                        return Err(format!("{other:?}").into());
                    }
                }
                drop(injection);
            }
        }
    }
    Ok(())
}
/// Each outcome inspection costs precisely one original-account Work and no numerical read.
/// ´test:unit:reciprocal-outcome-inspection-work-neighbours´
#[test]
fn reciprocal_outcome_inspection_work_neighbours() -> TestResult {
    for immutable in [false, true] {
        for remaining in [0, 1] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
            let owner = ready(heap.request_pointwise_reciprocal(request, &mut account))?;
            if immutable {
                let owner = candidate(owner, &mut account)?;
                let (tile, _) = super::published(heap.publish_application(owner, &mut account))?;
                let reader = tile.reader(&heap, &mut account)?;
                leave(&mut account, remaining)?;
                assert_eq!(
                    reader.application_outcome(0, &mut account).is_ok(),
                    remaining == 1
                );
            } else {
                leave(&mut account, remaining)?;
                assert_eq!(
                    owner.application_outcome(0, &mut account).is_ok(),
                    remaining == 1
                );
                assert!(matches!(
                    heap.cancel_application(owner, &mut account),
                    ApplicationCancellation::ApplicationCancelled(_)
                ));
            }
        }
    }
    Ok(())
}
/// Original full-shape allowance and one-cell allowance are checked independently, even at C=0.
#[test]
fn reciprocal_original_and_zero_capacity_invocation_ceilings() -> TestResult {
    for (cells, components) in [(0, 3), (1, 3), (65, 1)] {
        let setup = fixture::<f64>(cells, components, 1, 0, false, Limits::default())?;
        for ceiling in 0..3 {
            for short in [false, true] {
                let required = 12 * cells * components;
                if required == 0 && ceiling > 0 && short {
                    continue;
                }
                let slots = usize::from(ceiling != 0 || !short);
                let work = required - usize::from(ceiling == 1 && short);
                let scratch = required - usize::from(ceiling == 2 && short);
                let result = crate::OperationDraft::map_elementwise(
                    setup.request.operation.inputs()[0].shape().clone(),
                    setup.request.operation.outputs()[0].shape().clone(),
                    crate::Elementwise::Reciprocal,
                )?
                .check(ExecutionLimits::new(slots, work, scratch));
                if short {
                    assert!(matches!(result, Err(crate::OperationError::ResourceLimit)));
                } else {
                    assert!(result.is_ok());
                }
            }
        }
    }
    for ceiling in 0..3 {
        for short in [false, true] {
            let mut setup = fixture::<f64>(0, 3, 1, 0, false, Limits::default())?;
            setup.request.limits = ExecutionLimits::new(
                usize::from(ceiling != 0 || !short),
                36 - usize::from(ceiling == 1 && short),
                36 - usize::from(ceiling == 2 && short),
            );
            let injection =
                crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
            match setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account)
            {
                ApplicationRequest::ApplicationReady(owner) => {
                    assert!(!short);
                    assert!(matches!(
                        setup.heap.cancel_application(owner, &mut setup.account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Operation(crate::OperationError::ResourceLimit),
                    _,
                ) => {
                    assert!(short);
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                }
                other @ ApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
            drop(injection);
        }
    }
    Ok(())
}
/// Historical closed authentication costs three; current access includes its live occupation visits.
#[test]
fn reciprocal_reader_and_selected_load_work_boundaries() -> TestResult {
    for historical in [false, true] {
        for short in [false, true] {
            let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
            let owner = candidate(
                ready(
                    setup
                        .heap
                        .request_pointwise_reciprocal(setup.request, &mut setup.account),
                )?,
                &mut setup.account,
            )?;
            let (tile, _) =
                super::published(setup.heap.publish_application(owner, &mut setup.account))?;
            // Current result has three roles: 3 pin checks + 9 owner facts + 4 origin facts + 3 descriptors + 3 native facts + 3 layout facts.
            let required = if historical { 3 } else { 25 };
            leave(&mut setup.account, required - u64::from(short))?;
            let before = setup.account.usage().work;
            crate::loan::fault::reset_reads();
            let result = if historical {
                tile.pin().historical_reader(&mut setup.account)
            } else {
                tile.reader(&setup.heap, &mut setup.account)
            };
            if short {
                assert!(matches!(result, Err(LoanError::Budget(_))));
            } else {
                assert!(result.is_ok());
                assert_eq!(setup.account.usage().work - before, required);
            }
            assert_eq!(crate::loan::fault::reads(), 0);
        }
    }
    for remaining in [0, 1] {
        let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
        let owner = candidate(
            ready(
                setup
                    .heap
                    .request_pointwise_reciprocal(setup.request, &mut setup.account),
            )?,
            &mut setup.account,
        )?;
        let (tile, _) =
            super::published(setup.heap.publish_application(owner, &mut setup.account))?;
        let reader = tile.reader(&setup.heap, &mut setup.account)?;
        leave(&mut setup.account, remaining)?;
        crate::loan::fault::reset_reads();
        assert_eq!(
            reader
                .read_component(crate::HostChannel::Center, 0, 0, &mut setup.account)
                .is_ok(),
            remaining == 1
        );
        assert_eq!(crate::loan::fault::reads(), usize::try_from(remaining)?);
    }
    Ok(())
}
