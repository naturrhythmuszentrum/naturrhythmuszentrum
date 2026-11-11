// SPDX-License-Identifier: AGPL-3.0-only
//! Entry-point and prepared-use overlays keep the closed family boundary.

use super::{TestResult, fixture, seals};
use crate::loan::fault;
use crate::{ApplicationError, ApplicationRequest, Elementwise, LoanError};
use nrz_bindings::Limits;

/// Every native entry point refuses each other original law, including the retained prepared tag.
#[test]
fn square_root_all_directed_entry_point_mismatches() -> TestResult {
    for expected in [
        Elementwise::Square,
        Elementwise::Reciprocal,
        Elementwise::SquareRoot,
    ] {
        for other in seals::MEMBERS
            .into_iter()
            .filter(|other| *other != Some(expected))
        {
            let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
            let (_heap, _account, donor) = seals::owned(other)?;
            setup.request.operation = donor.core.evidence.operation.clone();
            let pointer = std::rc::Rc::as_ptr(&setup.request.input.record);
            fault::reset_reads();
            fault::reset_scalars();
            let injection = fault::Injection::after(0);
            let result = match expected {
                Elementwise::Square => setup
                    .heap
                    .request_pointwise_square(setup.request, &mut setup.account),
                Elementwise::Reciprocal => setup
                    .heap
                    .request_pointwise_reciprocal(setup.request, &mut setup.account),
                Elementwise::SquareRoot => setup
                    .heap
                    .request_pointwise_square_root(setup.request, &mut setup.account),
            };
            match result {
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::UnsupportedContract),
                    request,
                ) => assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer),
                other => return Err(format!("{other:?}").into()),
            }
            assert_eq!(
                [
                    fault::reads(),
                    fault::scalars(),
                    fault::Injection::attempts()
                ],
                [0; 3]
            );
            drop(injection);
        }
    }
    for function in [
        Elementwise::Square,
        Elementwise::Reciprocal,
        Elementwise::SquareRoot,
    ] {
        let mut setup = super::super::expm1::fixture(1, 1, [-2.0, 0.0], 2, false)?;
        setup.request.operation.operation =
            seals::operation(&setup.request.operation.operation, function)?;
        let pointer = std::rc::Rc::as_ptr(&setup.request.input.record);
        fault::reset_reads();
        fault::reset_scalars();
        let injection = fault::Injection::after(0);
        match setup
            .heap
            .request_pointwise_expm1(setup.request, &mut setup.account)
        {
            crate::Expm1ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::UnsupportedContract),
                request,
            ) => assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer),
            other => return Err(format!("{other:?}").into()),
        }
        assert_eq!(
            [
                fault::reads(),
                fault::scalars(),
                fault::Injection::attempts()
            ],
            [0; 3]
        );
        drop(injection);
    }
    Ok(())
}
/// A self-valid prepared correspondence and its exact use cannot authorize a root identity.
#[test]
fn square_root_prepared_operation_and_use_overlay_refuses() -> TestResult {
    for publish in [false, true] {
        let (mut heap, mut account, mut owner) = seals::owned(Some(Elementwise::SquareRoot))?;
        let (_prepared_heap, _prepared_account, mut donor) = seals::owned(None)?;
        if publish {
            owner = super::candidate(owner, &mut account)?;
        }
        std::mem::swap(
            &mut owner.core.host.retained_law,
            &mut donor.core.host.retained_law,
        );
        std::mem::swap(
            &mut owner.core.evidence.prepared,
            &mut donor.core.evidence.prepared,
        );
        fault::reset_reads();
        fault::reset_scalars();
        let mut owner = if publish {
            match heap.publish_application(owner, &mut account) {
                crate::ApplicationPublication::ApplicationRejected(
                    ApplicationError::Loan(LoanError::ForeignOwner),
                    owner,
                ) => owner,
                other => return Err(format!("{other:?}").into()),
            }
        } else {
            match owner.evaluate(&mut account) {
                crate::ApplicationEvaluation::ApplicationNotStarted(
                    ApplicationError::Loan(LoanError::UnsupportedContract),
                    owner,
                ) => owner,
                other => return Err(format!("{other:?}").into()),
            }
        };
        assert_eq!([fault::reads(), fault::scalars()], [0; 2]);
        std::mem::swap(
            &mut owner.core.host.retained_law,
            &mut donor.core.host.retained_law,
        );
        std::mem::swap(
            &mut owner.core.evidence.prepared,
            &mut donor.core.evidence.prepared,
        );
        if !publish {
            owner = super::candidate(owner, &mut account)?;
        }
        super::published(heap.publish_application(owner, &mut account))?;
    }
    Ok(())
}
