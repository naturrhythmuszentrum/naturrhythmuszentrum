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
/// ´test:unit:pointwise-evaluation-literal-work-boundaries-and-atomic-refusal´
#[test]
fn pointwise_evaluation_literal_work_boundaries_and_atomic_refusal() -> TestResult {
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
            let owner = ready(heap.request_pointwise_square(request, &mut account))?;
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
/// ´test:unit:pointwise-execution-ceilings-refuse-before-allocation´
#[test]
fn pointwise_execution_ceilings_refuse_before_allocation() -> TestResult {
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
                match heap.request_pointwise_square(request, &mut account) {
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
/// ´test:unit:pointwise-outcome-inspection-work-neighbours´
#[test]
fn pointwise_outcome_inspection_work_neighbours() -> TestResult {
    for immutable in [false, true] {
        for remaining in [0, 1] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
            let owner = ready(heap.request_pointwise_square(request, &mut account))?;
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
