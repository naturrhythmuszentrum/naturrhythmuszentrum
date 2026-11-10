// SPDX-License-Identifier: AGPL-3.0-only
//! Executable loop inventories pin cumulative original-account Work and exact failure prefixes.

use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::{
    ApplicationError, ApplicationEvaluation, ApplicationPublication, Expm1ApplicationRequest,
    LoanError,
};

/// Keep the original immutable ceiling and spend a caller prefix down to the tested boundary.
fn leave(account: &mut crate::ExecutionAccount, remaining: usize) -> TestResult {
    account.ledger.work(
        account.limits().work - account.usage().work - u64::try_from(remaining)?,
        crate::loan::AT,
    )?;
    Ok(())
}
/// Geometry: one source descriptor/group, one layout, two masks, nine debit rows and one pin.
/// Current law authentication is three. Constructor metadata has already been paid.
fn admission(cells: usize, components: usize) -> usize {
    let length = cells * components;
    let q = cells.div_ceil(64);
    let t = length.div_ceil(64);
    let allocations = 19 + 17 * usize::from(cells != 0);
    let records = 3 + 23 + 6;
    let edges = 3 + 3 + 1 + 7 + 7;
    let checks = 3 + 24 + 1 + (3 + 2 * 3) + 9 + 1 + (1 + 2 + 2 * 9);
    let cleanup = allocations + records + edges + (15 + 4 * 3);
    let prepare = records + edges + 1 + (1 + 1 + 2);
    checks + cleanup + prepare + 6 + 2 * length + 5 * (length + t + q) + 5 * q + cells
}
/// Five roles, common normalization and outcome gating precede the five selected reading checks.
fn publication(cells: usize, components: usize, completed: usize) -> usize {
    let q = cells.div_ceil(64);
    let length = cells * components;
    let authentication = 25 + 6 + 1 + 3;
    let inventory = 2 + 3 + 2 * 30;
    let source = 3 + (1 + 5) + (1 + 1) + 1 + 3;
    authentication
        + inventory
        + 5
        + source
        + 1
        + 4
        + 9
        + 5 * length
        + 11 * q
        + 1
        + cells
        + 5 * components * completed
}
/// Every evaluation prepays two words, every emitted cell, one load and five stores per component.
#[test]
fn prepared_evaluation_work_neighbours_and_unchanged_ready() -> TestResult {
    for (cells, components, selection, available) in
        [(3_usize, 1, 2, 3), (0, 3, 2, 0), (65, 3, 0, 0)]
    {
        let required = 3 + 2 * cells.div_ceil(64) + available * (6 * components + 2);
        for short in [0, 1] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(cells, components, [-2.0, 0.0], selection, false)?;
            let owner = ready(heap.request_pointwise_expm1(request, &mut account))?;
            let before_words = owner.core.filled.clone();
            let before_outcomes = format!("{:?}", owner.core.evidence.outcomes);
            leave(&mut account, required - short)?;
            let before = account.usage().work;
            crate::loan::fault::reset_reads();
            crate::loan::fault::reset_scalars();
            crate::loan::fault::reset_stores();
            match owner.evaluate(&mut account) {
                ApplicationEvaluation::ApplicationCandidate(owner) => {
                    assert_eq!(short, 0);
                    assert_eq!(account.usage().work - before, u64::try_from(required)?);
                    assert!(owner.core.candidate);
                    assert_eq!(crate::loan::fault::reads(), available * components);
                    assert_eq!(crate::loan::fault::scalars(), available * components);
                    assert_eq!(crate::loan::fault::stores(), 5 * available * components);
                }
                ApplicationEvaluation::ApplicationNotStarted(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    owner,
                ) => {
                    assert_eq!(short, 1);
                    assert!(!owner.core.candidate);
                    assert_eq!(owner.core.filled, before_words);
                    assert_eq!(
                        format!("{:?}", owner.core.evidence.outcomes),
                        before_outcomes
                    );
                    assert_eq!(crate::loan::fault::reads(), 0);
                    assert_eq!(crate::loan::fault::scalars(), 0);
                    assert_eq!(crate::loan::fault::stores(), 0);
                    for channel in owner
                        .core
                        .host
                        .channels
                        .iter()
                        .flatten()
                        .chain(owner.core.endpoints.iter().flatten())
                    {
                        assert!(
                            channel
                                .components
                                .iter()
                                .chain(&channel.complete)
                                .all(|word| *word == 0)
                        );
                    }
                    assert_eq!(
                        account.usage().work - before,
                        u64::try_from(required.min(3) - usize::from(required == 3))?
                    );
                }
                other => return Err(format!("{other:?}").into()),
            }
        }
    }
    Ok(())
}
/// The independent metadata inventory and design loop formula admit exactly N with prior Work.
#[test]
fn prepared_admission_work_neighbours() -> TestResult {
    for (cells, components) in [(0, 1), (1, 1), (3, 1), (65, 3)] {
        for short in [0, 1] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(cells, components, [-2.0, 0.0], 2, false)?;
            let required = admission(cells, components);
            leave(&mut account, required - short)?;
            let before = account.usage().work;
            let injection = crate::loan::fault::Injection::after(usize::MAX);
            match heap.request_pointwise_expm1(request, &mut account) {
                Expm1ApplicationRequest::ApplicationReady(owner) => {
                    assert_eq!(short, 0);
                    assert_eq!(account.usage().work - before, u64::try_from(required)?);
                    drop(owner);
                }
                Expm1ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    _,
                ) => {
                    assert_eq!(short, 1);
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                    assert_eq!(account.usage().work - before, u64::try_from(required - 6)?);
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
/// Repair repeats the same completed-cell checks and shell preparation, with no native invocation.
#[test]
fn prepared_publication_work_neighbours_and_repair() -> TestResult {
    for cells in [0, 1, 3] {
        for short in [0, 1] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(cells, 1, [-2.0, 0.0], 2, false)?;
            let owner = candidate(
                ready(heap.request_pointwise_expm1(request, &mut account))?,
                &mut account,
            )?;
            let required = publication(cells, 1, cells.min(2));
            let injection = crate::loan::fault::Injection::after(0);
            let before = account.usage().work;
            let owner = match heap.publish_application(owner, &mut account) {
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::AllocationRefused),
                    owner,
                ) => owner,
                other => return Err(format!("{other:?}").into()),
            };
            assert_eq!(account.usage().work - before, u64::try_from(required)?);
            drop(injection);
            leave(&mut account, required - short)?;
            let before = account.usage().work;
            crate::loan::fault::reset_scalars();
            match heap.publish_application(owner, &mut account) {
                ApplicationPublication::ApplicationPublished(_, _) => {
                    assert_eq!(short, 0);
                    assert_eq!(account.usage().work - before, u64::try_from(required)?);
                }
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    owner,
                ) => {
                    assert_eq!(short, 1);
                    assert_eq!(account.usage().work - before, u64::try_from(required - 9)?);
                    assert!(matches!(
                        heap.cancel_application(owner, &mut account),
                        crate::ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                other => return Err(format!("{other:?}").into()),
            }
            assert_eq!(crate::loan::fault::scalars(), 0);
        }
    }
    Ok(())
}
/// The pair's two loads are one prepayment, so N-1 exposes no first endpoint.
#[test]
fn prepared_endpoint_pair_load_boundary() -> TestResult {
    for short in [0, 1] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let owner = candidate(
            ready(heap.request_pointwise_expm1(request, &mut account))?,
            &mut account,
        )?;
        let (tile, _) = published(heap.publish_application(owner, &mut account))?;
        let reader = tile.reader(&heap, &mut account)?;
        leave(&mut account, 2 - short)?;
        crate::loan::fault::reset_reads();
        assert_eq!(
            reader.expm1_endpoints(0, 0, &mut account).is_ok(),
            short == 0
        );
        assert_eq!(crate::loan::fault::reads(), if short == 0 { 2 } else { 0 });
    }
    Ok(())
}
