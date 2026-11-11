// SPDX-License-Identifier: AGPL-3.0-only
//! Every directed family substitution preserves the rightful owner before any numerical load.

use super::{TestResult, candidate, fixture, published, ready};
use crate::loan::fault;
use crate::{
    ApplicationError, ApplicationEvaluation, ApplicationPublication, Elementwise, ExecutionAccount,
    Heap, LoanError, OwnedApplication,
};
use nrz_bindings::Limits;

/// None denotes the retained prepared member, whose fixture keeps its own admitted input and use.
pub(super) const MEMBERS: [Option<Elementwise>; 4] = [
    Some(Elementwise::Square),
    Some(Elementwise::Reciprocal),
    Some(Elementwise::SquareRoot),
    None,
];
/// Public construction creates a valid alternate law over the same original shape witnesses.
pub(super) fn operation(
    original: &crate::CheckedOperation,
    function: Elementwise,
) -> TestResult<crate::CheckedOperation> {
    Ok(crate::OperationDraft::map_elementwise(
        original.inputs()[0].shape().clone(),
        original.outputs()[0].shape().clone(),
        function,
    )?
    .check(crate::ExecutionLimits::default())?)
}
/// Native members all receive x=1 and equal three-role layouts; prepared keeps x=-1/2 and its law.
pub(super) fn owned(
    member: Option<Elementwise>,
) -> TestResult<(Heap, ExecutionAccount, OwnedApplication<f64>)> {
    let Some(function) = member else {
        let mut setup = super::super::expm1::fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let owner = match setup
            .heap
            .request_pointwise_expm1(setup.request, &mut setup.account)
        {
            crate::Expm1ApplicationRequest::ApplicationReady(owner) => owner,
            other @ crate::Expm1ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        return Ok((setup.heap, setup.account, owner));
    };
    let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
    super::super::refusal::replace_source(&mut setup, |_, _| (1.0, 0.0))?;
    setup.request.operation = operation(&setup.request.operation, function)?;
    let result = match function {
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
    Ok((setup.heap, setup.account, ready(result)?))
}
/// Swapping twice restores the exact original evidence and contract, including pointer identities.
fn exchange(
    owner: &mut OwnedApplication<f64>,
    donor: &mut OwnedApplication<f64>,
    choice: usize,
) -> TestResult {
    match choice {
        0 => std::mem::swap(
            &mut owner.core.evidence.operation,
            &mut donor.core.evidence.operation,
        ),
        1 => std::mem::swap(&mut owner.core.contract, &mut donor.core.contract),
        2 => std::mem::swap(&mut owner.core.evidence.law, &mut donor.core.evidence.law),
        3 => std::mem::swap(&mut owner.core.evidence, &mut donor.core.evidence),
        4 => {
            exchange(owner, donor, 0)?;
            exchange(owner, donor, 1)?;
        }
        _ => return Err("substitution".into()),
    }
    Ok(())
}
/// A single substitution preserves the detached address, phase and numerical visits.
fn detached(
    member: Option<Elementwise>,
    other: Option<Elementwise>,
    publish: bool,
    choice: usize,
) -> TestResult {
    let (mut heap, mut account, mut owner) = owned(member)?;
    let (_donor_heap, mut donor_account, mut donor) = owned(other)?;
    if publish {
        owner = candidate(owner, &mut account)?;
        donor = candidate(donor, &mut donor_account)?;
    }
    let pointer = std::ptr::from_ref(owner.core.as_ref());
    let outcomes = format!("{:?}", owner.core.evidence.outcomes);
    let filled = owner.core.filled.clone();
    exchange(&mut owner, &mut donor, choice)?;
    fault::reset_reads();
    fault::reset_scalars();
    fault::reset_stores();
    let injection = fault::Injection::after(0);
    let (error, mut owner) = if publish {
        match heap.publish_application(owner, &mut account) {
            ApplicationPublication::ApplicationRepair(error, owner) => (error, owner),
            ApplicationPublication::ApplicationRejected(error, owner) if choice == 3 => {
                (error, owner)
            }
            other => return Err(format!("{other:?}").into()),
        }
    } else {
        match owner.evaluate(&mut account) {
            ApplicationEvaluation::ApplicationNotStarted(error, owner) => (error, owner),
            other => return Err(format!("{other:?}").into()),
        }
    };
    if choice == 1 || choice == 3 {
        assert!(
            matches!(error, ApplicationError::Loan(LoanError::WrongLoan)),
            "{error:?}"
        );
    } else {
        assert!(
            matches!(
                error,
                ApplicationError::Loan(LoanError::UnsupportedContract)
            ),
            "{error:?}"
        );
    }
    assert_eq!(
        [
            fault::reads(),
            fault::scalars(),
            fault::stores(),
            fault::Injection::attempts()
        ],
        [0; 4]
    );
    assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
    assert_eq!(owner.core.filled, filled);
    drop(injection);
    exchange(&mut owner, &mut donor, choice)?;
    assert_eq!(format!("{:?}", owner.core.evidence.outcomes), outcomes);
    if !publish {
        owner = candidate(owner, &mut account)?;
    }
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    let reader = tile.reader(&heap, &mut account)?;
    assert!(matches!(
        reader.application_outcome(0, &mut account)?,
        crate::CellOutcome::Completed
    ));
    if member.is_some() {
        assert_eq!(
            reader.read_component(crate::HostChannel::Center, 0, 0, &mut account)?,
            1.0
        );
    }
    Ok(())
}
/// All twelve ordered pairs reject original-law disagreement before the one-cell law check.
/// ´test:unit:square-root-all-directed-detached-family-substitutions´
#[test]
fn square_root_all_directed_detached_family_substitutions() -> TestResult {
    let mut pairs = 0;
    for member in MEMBERS {
        for other in MEMBERS.into_iter().filter(|other| *other != member) {
            pairs += 1;
            for publish in [false, true] {
                for choice in 0..5 {
                    detached(member, other, publish, choice)?;
                }
            }
        }
    }
    assert_eq!(pairs, 12);
    Ok(())
}
