// SPDX-License-Identifier: AGPL-3.0-only
//! Literal fixed-metadata totals accompany the independently specified traversal counts.

use super::{Fixture, TestResult, candidate, fixture, ready};
use crate::{
    ApplicationCancellation, ApplicationError, ApplicationPublication, ApplicationRequest,
    LoanError,
};
use nrz_bindings::Limits;

/// Leave the literal boundary on the original account, retaining its nonzero prior usage.
fn leave(account: &mut crate::ExecutionAccount, remaining: u64) -> TestResult {
    account.ledger.work(
        account.limits().work - account.usage().work - remaining,
        crate::loan::AT,
    )?;
    Ok(())
}
/// Fresh host geometry has h descriptor/group rows, one layout, two masks and 3*h+6 debits.
/// Request/current checks and planning scans: 16*h+49, plus the structural 2*L prefix.
/// Cleanup: 19+11*I allocations, 26 records, h+13 non-pin edges, 27 cancellation steps.
/// Preparation: 26 records, h+13 edges, one initial pin creation and h+3 slot visits.
/// Thus fixed admission is 19*h+177+11*I; explicit loops are 5*L+3*t+(h+7)q+C.
/// ´test:unit:square_root-admission-literal-work-neighbours-with-prior-usage´
#[test]
fn square_root_admission_literal_work_neighbours_with_prior_usage() -> TestResult {
    for (cells, components, channels, required) in [
        (3, 1, 1, 236),
        (3, 1, 2, 256),
        (3, 1, 3, 276),
        (0, 1, 3, 234),
        (1, 1, 3, 264),
        (63, 1, 3, 636),
        (64, 1, 3, 642),
        (65, 1, 3, 661),
        (65, 3, 3, 1317),
    ] {
        for succeeds in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(cells, components, channels, 2, false, Limits::default())?;
            let ticket = request.input.ticket()?;
            leave(&mut account, required - u64::from(!succeeds))?;
            let before = account.usage().work;
            let injection =
                crate::loan::fault::Injection::after(if succeeds { usize::MAX } else { 0 });
            match heap.request_pointwise_square_root(request, &mut account) {
                ApplicationRequest::ApplicationReady(owner) => {
                    assert!(succeeds);
                    assert_eq!(account.usage().work - before, required);
                    assert!(matches!(
                        heap.cancel_application(owner, &mut account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    request,
                ) => {
                    assert!(!succeeds);
                    assert_eq!(request.input.ticket()?, ticket);
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                    assert_eq!(
                        account.usage().work - before,
                        u64::try_from(16 * channels + 49 + 2 * cells * components)?
                    );
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
/// Publication authenticates 25 owner facts; inventory visits 2*h+47 rows; shell cleanup costs 5.
/// Current source costs 3*h+12, evidence 1, shells 4 and prepared records 9: fixed 5*h+103.
/// The independent loops are 3*L+7*q+1+C+3*kz, including normalization at zero capacity.
/// ´test:unit:square_root-publication-literal-work-neighbours-and-repeated-attempts´
#[test]
fn square_root_publication_literal_work_neighbours_and_repeated_attempts() -> TestResult {
    for (cells, channels, required) in [(3, 1, 137), (3, 2, 142), (3, 3, 147), (0, 3, 119)] {
        for succeeds in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture::<f64>(cells, 1, channels, 2, false, Limits::default())?;
            let owner = candidate(
                ready(heap.request_pointwise_square_root(request, &mut account))?,
                &mut account,
            )?;
            // One complete failed shell attempt pays the same performed validation inventory.
            let injection = crate::loan::fault::Injection::after(0);
            let before = account.usage().work;
            let owner = match heap.publish_application(owner, &mut account) {
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::AllocationRefused),
                    owner,
                ) => owner,
                other => return Err(format!("{other:?}").into()),
            };
            assert_eq!(account.usage().work - before, required);
            drop(injection);
            leave(&mut account, required - u64::from(!succeeds))?;
            let before = account.usage().work;
            match heap.publish_application(owner, &mut account) {
                ApplicationPublication::ApplicationPublished(tile, _) => {
                    assert!(succeeds);
                    assert_eq!(account.usage().work - before, required);
                    drop(tile);
                }
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    owner,
                ) => {
                    assert!(!succeeds);
                    assert_eq!(account.usage().work - before, required - 9);
                    assert!(matches!(
                        heap.cancel_application(owner, &mut account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                other => return Err(format!("{other:?}").into()),
            }
        }
    }
    Ok(())
}
