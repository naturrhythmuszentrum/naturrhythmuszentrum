// SPDX-License-Identifier: AGPL-3.0-only
//! Equal shape, allowance and numerical answers cannot substitute another closed family.

use super::{TestResult, candidate, fixture, published, ready};
use crate::loan::{fault, registry::State};
use crate::{
    ApplicationError, ApplicationEvaluation, ApplicationPublication, Elementwise, LoanError,
};
use nrz_bindings::Limits;
use std::rc::Rc;

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
/// Private substitutions replace an individually valid operation, one-cell contract or captured law.
fn substitute(
    owner: &mut crate::OwnedApplication<f64>,
    other: &crate::CheckedOperation,
    choice: usize,
) -> TestResult {
    let crate::binding::NativeRecipe::Kernel(law) = &other.recipe else {
        return Err("kernel".into());
    };
    match choice {
        0 => owner.core.evidence.operation = other.clone(),
        1 => owner.core.contract = law.as_ref().clone(),
        2 => owner.core.evidence.law = Rc::clone(law),
        _ => return Err("substitution".into()),
    }
    Ok(())
}
/// Both directions refuse before loads and preserve the exact detached capability and evidence.
/// ´test:unit:reciprocal-square-detached-family-substitutions´
#[test]
fn reciprocal_square_detached_family_substitutions() -> TestResult {
    for square in [false, true] {
        for publish in [false, true] {
            for choice in 0..3 {
                let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
                super::super::refusal::replace_source(&mut setup, |_, _| (1.0, 0.0))?;
                let other = operation(&setup.request.operation, Elementwise::Square)?;
                let other = if square {
                    std::mem::replace(&mut setup.request.operation, other)
                } else {
                    other
                };
                let request = if square {
                    setup
                        .heap
                        .request_pointwise_square(setup.request, &mut setup.account)
                } else {
                    setup
                        .heap
                        .request_pointwise_reciprocal(setup.request, &mut setup.account)
                };
                let mut owner = ready(request)?;
                if publish {
                    owner = candidate(owner, &mut setup.account)?;
                }
                let original = owner.core.evidence.operation.clone();
                let contract = owner.core.contract.clone();
                let law = Rc::clone(&owner.core.evidence.law);
                let pointer = std::ptr::from_ref(owner.core.as_ref());
                let outcomes = format!("{:?}", owner.core.evidence.outcomes);
                substitute(&mut owner, &other, choice)?;
                fault::reset_reads();
                fault::reset_scalars();
                let injection = fault::Injection::after(0);
                let (error, mut owner) = if publish {
                    match setup.heap.publish_application(owner, &mut setup.account) {
                        ApplicationPublication::ApplicationRepair(error, owner) => (error, owner),
                        other => return Err(format!("{other:?}").into()),
                    }
                } else {
                    match owner.evaluate(&mut setup.account) {
                        ApplicationEvaluation::ApplicationNotStarted(error, owner) => {
                            (error, owner)
                        }
                        other => return Err(format!("{other:?}").into()),
                    }
                };
                if choice == 1 {
                    assert!(matches!(
                        error,
                        ApplicationError::Loan(LoanError::WrongLoan)
                    ));
                } else {
                    assert!(matches!(
                        error,
                        ApplicationError::Loan(LoanError::UnsupportedContract)
                    ));
                }
                assert_eq!(
                    [
                        fault::reads(),
                        fault::scalars(),
                        fault::Injection::attempts()
                    ],
                    [0; 3]
                );
                assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
                assert_eq!(format!("{:?}", owner.core.evidence.outcomes), outcomes);
                drop(injection);
                owner.core.evidence.operation = original;
                owner.core.evidence.law = law;
                owner.core.contract = contract;
                if !publish {
                    owner = candidate(owner, &mut setup.account)?;
                }
                published(setup.heap.publish_application(owner, &mut setup.account))?;
            }
        }
    }
    Ok(())
}
/// Temporarily detach registry aliases so a test can alter the immutable private origin safely.
fn alter_bundle(
    tile: &mut crate::PublishedTile<f64>,
    heap: &mut crate::Heap,
    choice: usize,
    other: &crate::CheckedOperation,
) -> TestResult {
    let root = tile.pin().ticket()?.identity.root.slot;
    let pin_slot = heap.loans.groups[root].as_ref().ok_or("group")?.pin;
    let mut index = heap.loans.pins[pin_slot].take().ok_or("index")?;
    index.witness = None;
    heap.loans.groups[root].as_mut().ok_or("group")?.state = State::Reserved;
    let pin = Rc::get_mut(&mut tile.pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut pin.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    let evidence = bundle.origin.evidence.as_mut().ok_or("origin")?;
    match choice {
        0 => evidence.operation = other.clone(),
        1 => {
            let crate::binding::NativeRecipe::Kernel(law) = &other.recipe else {
                return Err("kernel".into());
            };
            evidence.law = Rc::clone(law);
        }
        2 => evidence.revision = 0,
        3 => evidence.completed_mask = None,
        _ => return Err("substitution".into()),
    }
    heap.loans.groups[root].as_mut().ok_or("group")?.state =
        State::Published(Rc::clone(&pin.bundle));
    index.witness = Some(Rc::downgrade(&tile.pin.record));
    heap.loans.pins[pin_slot] = Some(index);
    Ok(())
}
/// Historical authentication needs no source occupation, but always checks the closed family seal.
/// ´test:unit:reciprocal-square-current-and-historical-family-substitutions´
#[test]
fn reciprocal_square_current_and_historical_family_substitutions() -> TestResult {
    for square in [false, true] {
        for choice in 0..4 {
            let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
            super::super::refusal::replace_source(&mut setup, |_, _| (1.0, 0.0))?;
            let other = operation(&setup.request.operation, Elementwise::Square)?;
            let other = if square {
                std::mem::replace(&mut setup.request.operation, other)
            } else {
                other
            };
            let request = if square {
                setup
                    .heap
                    .request_pointwise_square(setup.request, &mut setup.account)
            } else {
                setup
                    .heap
                    .request_pointwise_reciprocal(setup.request, &mut setup.account)
            };
            let owner = candidate(ready(request)?, &mut setup.account)?;
            let (mut tile, _) =
                published(setup.heap.publish_application(owner, &mut setup.account))?;
            alter_bundle(&mut tile, &mut setup.heap, choice, &other)?;
            fault::reset_reads();
            let result = tile.reader(&setup.heap, &mut setup.account);
            if choice < 2 {
                assert!(matches!(result, Err(LoanError::UnsupportedContract)));
            } else {
                assert!(matches!(result, Err(LoanError::WrongLoan)));
            }
            assert_eq!(fault::reads(), 0);
            drop(setup.heap);
            let result = tile.pin().historical_reader(&mut setup.account);
            if choice < 2 {
                assert!(matches!(result, Err(LoanError::UnsupportedContract)));
            } else {
                assert!(matches!(result, Err(LoanError::WrongLoan)));
            }
            assert_eq!(fault::reads(), 0);
        }
    }
    Ok(())
}
/// A self-valid prepared operation and use cannot authorize a closed captured identity.
#[test]
fn reciprocal_refuses_prepared_operation_overlay() -> TestResult {
    for publish in [false, true] {
        let mut setup = fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
        let mut prepared = super::super::expm1::fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let attachment = crate::loan::retained_expm1::tests::attach(
            prepared.request.law,
            &prepared.heap,
            &mut prepared.account,
        )?;
        let mut owner = ready(
            setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account),
        )?;
        if publish {
            owner = candidate(owner, &mut setup.account)?;
        }
        owner.core.host.retained_law = Some(attachment);
        owner.core.evidence.prepared = Some(prepared.request.operation);
        fault::reset_reads();
        fault::reset_scalars();
        if publish {
            assert!(matches!(
                setup.heap.publish_application(owner, &mut setup.account),
                ApplicationPublication::ApplicationRejected(_, _)
            ));
        } else {
            assert!(matches!(
                owner.evaluate(&mut setup.account),
                ApplicationEvaluation::ApplicationNotStarted(
                    ApplicationError::Loan(LoanError::UnsupportedContract),
                    _
                )
            ));
        }
        assert_eq!([fault::reads(), fault::scalars()], [0; 2]);
    }
    Ok(())
}
