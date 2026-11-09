// SPDX-License-Identifier: AGPL-3.0-only
//! Actual publication visits and bounded diagnostics preserve the charged inspection surface.
use super::{TestResult, candidate, fixture, ready};
use crate::{ApplicationError, ApplicationPublication, LoanError};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// C=65, k=1, h=3: fixed 118 plus 195+14+1+65, then three validations per selected cell.
fn visits<P: Scalar + 'static>() -> TestResult {
    for (selection, required) in [(0, 393), (1, 522)] {
        for succeeds in [false, true] {
            let mut setup = fixture::<P>(65, 1, 3, selection, false, Limits::default())?;
            let owner = candidate(
                ready(
                    setup
                        .heap
                        .request_pointwise_square(setup.request, &mut setup.account),
                )?,
                &mut setup.account,
            )?;
            let remaining = required - if succeeds { 0 } else { 13 };
            setup.account.ledger.work(
                setup.account.limits().work - setup.account.usage().work - remaining,
                crate::loan::AT,
            )?;
            let before = setup.account.usage().work;
            crate::loan::fault::reset_publication_cells();
            match setup.heap.publish_application(owner, &mut setup.account) {
                ApplicationPublication::ApplicationPublished(_, _) => assert!(succeeds),
                ApplicationPublication::ApplicationRepair(
                    ApplicationError::Loan(LoanError::Budget(_)),
                    _,
                ) => assert!(!succeeds),
                other => return Err(format!("{other:?}").into()),
            }
            assert_eq!(crate::loan::fault::publication_cells(), 65);
            assert_eq!(setup.account.usage().work - before, remaining);
        }
    }
    Ok(())
}
/// Empty and sparse publication each perform only the one charged cell-membership traversal.
/// ´test:unit:pointwise-publication-membership-visits-are-paid-once´
#[test]
fn pointwise_publication_membership_visits_are_paid_once() -> TestResult {
    visits::<f32>()?;
    visits::<f64>()
}
/// Exhausted accounts cannot use any owning public formatter to inspect outcomes or tracking.
/// ´test:unit:pointwise-debug-is-bounded-at-exhausted-work´
#[test]
fn pointwise_debug_is_bounded_at_exhausted_work() -> TestResult {
    for evaluated in [false, true] {
        for wrapper in 0..4 {
            let mut setup = fixture::<f64>(65, 3, 3, 2, false, Limits::default())?;
            super::refusal::replace_source(&mut setup, |cell, component| {
                if cell == 1 && component == 0 {
                    (f64::MAX, f64::MAX)
                } else {
                    (2.0, 0.0)
                }
            })?;
            let mut owner = ready(
                setup
                    .heap
                    .request_pointwise_square(setup.request, &mut setup.account),
            )?;
            if evaluated {
                owner = candidate(owner, &mut setup.account)?;
            }
            let remaining = setup.account.limits().work - setup.account.usage().work;
            setup.account.ledger.work(remaining, crate::loan::AT)?;
            assert!(matches!(
                owner.application_outcome(1, &mut setup.account),
                Err(LoanError::Budget(_))
            ));
            let phase = if evaluated { "candidate" } else { "ready" };
            let expected = format!(
                "OwnedApplication {{ ticket: {:?}, phase: {phase:?}, .. }}",
                owner.ticket()
            );
            assert_eq!(format!("{owner:?}"), expected);
            assert_eq!(
                format!("{:?}", owner.core),
                expected.replace("OwnedApplication", "ApplicationCore")
            );
            let rendered = match wrapper {
                0 => format!("{:?}", crate::ApplicationRequest::ApplicationReady(owner)),
                1 => format!(
                    "{:?}",
                    crate::ApplicationEvaluation::ApplicationCandidate(owner)
                ),
                2 => format!(
                    "{:?}",
                    ApplicationPublication::ApplicationRepair(LoanError::WrongLoan.into(), owner)
                ),
                _ => format!(
                    "{:?}",
                    crate::ApplicationCancellation::ApplicationRejected(
                        LoanError::WrongLoan.into(),
                        owner
                    )
                ),
            };
            assert!(rendered.contains(&expected));
            for hidden in [
                "outcomes",
                "filled:",
                "components:",
                "complete:",
                "CellRefused",
                "UnboundedImage",
                "Pending",
                "Completed",
            ] {
                assert!(!rendered.contains(hidden));
            }
        }
    }
    Ok(())
}
