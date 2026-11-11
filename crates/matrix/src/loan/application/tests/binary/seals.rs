// SPDX-License-Identifier: AGPL-3.0-only
//! Individually valid family members and ordered captures cannot substitute at authenticated boundaries.

use super::{BinaryElementwise, Limits, TestResult, admit, candidate, custom, published, ready};
use crate::loan::{fault, registry::State};
use crate::{
    ApplicationError, ApplicationEvaluation, ApplicationPublication, ExecutionAccount, Heap,
    LoanError, OwnedApplication,
};
use std::rc::Rc;

/// Each family is built through its own public checked operation and admission entry point.
fn owner(family: usize) -> TestResult<(Heap, ExecutionAccount, OwnedApplication<f64>)> {
    if family >= 4 {
        let function = if family == 4 {
            BinaryElementwise::Product
        } else {
            BinaryElementwise::Quotient
        };
        let mut setup = custom(
            1,
            1,
            [1; 2],
            2,
            false,
            function,
            Limits::default(),
            |_, _, _| (1.0, 0.0),
        )?;
        let owner = ready(admit(
            &mut setup.heap,
            setup.request,
            function,
            &mut setup.account,
        ))?;
        return Ok((setup.heap, setup.account, owner));
    }
    if family == 3 {
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
    }
    let mut setup = super::super::fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
    super::super::refusal::replace_source(&mut setup, |_, _| (1.0, 0.0))?;
    let function = match family {
        0 => crate::Elementwise::Square,
        1 => crate::Elementwise::Reciprocal,
        2 => crate::Elementwise::SquareRoot,
        _ => return Err("family".into()),
    };
    setup.request.operation = crate::OperationDraft::map_elementwise(
        setup.request.operation.inputs()[0].shape().clone(),
        setup.request.operation.outputs()[0].shape().clone(),
        function,
    )?
    .check(crate::ExecutionLimits::default())?;
    let request = match function {
        crate::Elementwise::Square => setup
            .heap
            .request_pointwise_square(setup.request, &mut setup.account),
        crate::Elementwise::Reciprocal => setup
            .heap
            .request_pointwise_reciprocal(setup.request, &mut setup.account),
        crate::Elementwise::SquareRoot => setup
            .heap
            .request_pointwise_square_root(setup.request, &mut setup.account),
    };
    Ok((setup.heap, setup.account, super::super::ready(request)?))
}
/// Swap one individually valid metadata object, preserving all other independent seals.
fn swap(owner: &mut OwnedApplication<f64>, other: &mut OwnedApplication<f64>, choice: usize) {
    match choice {
        0 => std::mem::swap(
            &mut owner.core.evidence.operation,
            &mut other.core.evidence.operation,
        ),
        1 => std::mem::swap(&mut owner.core.evidence.law, &mut other.core.evidence.law),
        2 => std::mem::swap(&mut owner.core.contract, &mut other.core.contract),
        _ => unreachable!("fixed substitution cases"),
    }
}
/// Every directed binary/unary and binary/binary pair refuses before the first load or allocation.
#[test]
fn binary_all_directed_family_substitutions_evaluation_and_publication() -> TestResult {
    for family in 0..6 {
        for other in 0..6 {
            if family == other || (family < 4 && other < 4) {
                continue;
            }
            for publish in [false, true] {
                for choice in 0..3 {
                    let (mut heap, mut account, mut target) = owner(family)?;
                    let (_, _, mut replacement) = owner(other)?;
                    if publish {
                        target = candidate(target, &mut account)?;
                    }
                    let pointer = std::ptr::from_ref(target.core.as_ref());
                    swap(&mut target, &mut replacement, choice);
                    fault::reset_reads();
                    fault::reset_scalars();
                    let injection = fault::Injection::after(0);
                    let mut target = refused(target, publish, &mut heap, &mut account)?;
                    assert_eq!(
                        [
                            fault::reads(),
                            fault::scalars(),
                            fault::Injection::attempts()
                        ],
                        [0; 3]
                    );
                    assert_eq!(std::ptr::from_ref(target.core.as_ref()), pointer);
                    drop(injection);
                    swap(&mut target, &mut replacement, choice);
                    assert!(matches!(
                        heap.cancel_application(target, &mut account),
                        crate::ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
            }
        }
    }
    Ok(())
}
/// Authentication returns the rightful owner regardless of the boundary's refusal wrapper.
fn refused(
    owner: OwnedApplication<f64>,
    publish: bool,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<OwnedApplication<f64>> {
    let (error, owner) = if publish {
        match heap.publish_application(owner, account) {
            ApplicationPublication::ApplicationRepair(error, owner)
            | ApplicationPublication::ApplicationRejected(error, owner) => (error, owner),
            other @ ApplicationPublication::ApplicationPublished(..) => {
                return Err(format!("{other:?}").into());
            }
        }
    } else {
        match owner.evaluate(account) {
            ApplicationEvaluation::ApplicationNotStarted(error, owner)
            | ApplicationEvaluation::ApplicationRejected(error, owner) => (error, owner),
            other @ ApplicationEvaluation::ApplicationCandidate(_) => {
                return Err(format!("{other:?}").into());
            }
        }
    };
    assert!(matches!(
        error,
        ApplicationError::Loan(
            LoanError::WrongLoan | LoanError::UnsupportedContract | LoanError::CoverageMismatch
        )
    ));
    Ok(owner)
}
/// Withdraw registry aliases only while editing an immutable test seal, then restore them exactly.
fn alter(
    tile: &mut crate::PublishedTile<f64>,
    heap: &mut Heap,
    change: impl FnOnce(&mut crate::loan::registry::Bundle) -> TestResult,
) -> TestResult {
    let root = tile.pin.ticket()?.identity.root.slot;
    let pin_slot = tile.pin.record.payload.as_ref().ok_or("pin")?.index;
    let mut index = heap.loans.pins[pin_slot].take().ok_or("pin index")?;
    index.witness = None;
    heap.loans.groups[root].as_mut().ok_or("group")?.state = State::Reserved;
    let pin = Rc::get_mut(&mut tile.pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut pin.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    change(bundle)?;
    heap.loans.groups[root].as_mut().ok_or("group")?.state =
        State::Published(Rc::clone(&pin.bundle));
    index.witness = Some(Rc::downgrade(&tile.pin.record));
    heap.loans.pins[pin_slot] = Some(index);
    Ok(())
}
/// Published current and historical readers recheck original operations and law pointers.
#[test]
fn binary_all_directed_family_substitutions_current_and_historical() -> TestResult {
    for family in 0..6 {
        for other in 0..6 {
            if family == other || (family < 4 && other < 4) {
                continue;
            }
            for law_only in [false, true] {
                let (mut heap, mut account, target) = owner(family)?;
                let (_, _, replacement) = owner(other)?;
                let target = candidate(target, &mut account)?;
                let (mut tile, _) = published(heap.publish_application(target, &mut account))?;
                alter(&mut tile, &mut heap, |bundle| {
                    let evidence = bundle.origin.evidence.as_mut().ok_or("evidence")?;
                    if law_only {
                        evidence.law = Rc::clone(&replacement.core.evidence.law);
                    } else {
                        evidence.operation = replacement.core.evidence.operation.clone();
                    }
                    Ok(())
                })?;
                fault::reset_reads();
                assert!(tile.reader(&heap, &mut account).is_err());
                assert_eq!(fault::reads(), 0);
                drop(heap);
                assert!(tile.pin().historical_reader(&mut account).is_err());
                assert_eq!(fault::reads(), 0);
            }
        }
    }
    Ok(())
}
/// Original/captured order, both numerical pins, side owners and original law seals stay paired.
#[test]
fn binary_operand_and_side_substitutions_before_either_load() -> TestResult {
    for publish in [false, true] {
        for choice in 0..5 {
            let (mut heap, mut account, mut target) = owner(4)?;
            let (_, _, mut replacement) = owner(4)?;
            if publish {
                target = candidate(target, &mut account)?;
            }
            let crate::loan::application::binary::Auxiliary::Binary(side) =
                &mut target.core.evidence.endpoint_tokens
            else {
                return Err("side".into());
            };
            match choice {
                0 => std::mem::swap(&mut target.core.input, &mut side.input),
                1 => std::mem::swap(
                    &mut target.core.evidence.source_identity,
                    &mut side.identity,
                ),
                2 => std::mem::swap(&mut target.core.evidence.source, &mut side.source),
                3 => std::mem::swap(
                    &mut target.core.evidence.endpoint_tokens,
                    &mut replacement.core.evidence.endpoint_tokens,
                ),
                4 => side.token.receipt += 1,
                _ => return Err("choice".into()),
            }
            fault::reset_reads();
            fault::reset_scalars();
            let target = refused(target, publish, &mut heap, &mut account)?;
            assert_eq!([fault::reads(), fault::scalars()], [0; 2]);
            drop(target);
        }
    }
    Ok(())
}
/// A published side retains no source pin and cannot replace its original ordered captures.
#[test]
fn binary_immutable_order_token_and_completion_substitutions() -> TestResult {
    for choice in 0..7 {
        let (mut heap, mut account, target) = owner(5)?;
        let target = candidate(target, &mut account)?;
        let (mut tile, _) = published(heap.publish_application(target, &mut account))?;
        alter(&mut tile, &mut heap, |bundle| {
            let evidence = bundle.origin.evidence.as_mut().ok_or("evidence")?;
            let crate::loan::application::binary::Auxiliary::Binary(side) =
                &mut evidence.endpoint_tokens
            else {
                return Err("side".into());
            };
            match choice {
                0 => std::mem::swap(&mut evidence.source_identity, &mut side.identity),
                1 => std::mem::swap(&mut evidence.source, &mut side.source),
                2 => side.token.receipt += 1,
                3 => evidence.completed_mask = None,
                4 => evidence.revision = 0,
                6 => side.token.key = evidence.token.key,
                5 => {
                    evidence.outcomes[0] = crate::CellOutcome::CellRefused {
                        component: 0,
                        reason: crate::KernelError::BinaryDomain {
                            function: BinaryElementwise::Product,
                            entry: 0,
                            reason: crate::DomainReason::UnboundedImage,
                        },
                    }
                }
                _ => return Err("choice".into()),
            }
            Ok(())
        })?;
        fault::reset_reads();
        if choice == 5 {
            let reader = tile.reader(&heap, &mut account)?;
            assert!(reader.application_outcome(0, &mut account).is_err());
            assert!(
                reader
                    .read_component(crate::HostChannel::Center, 0, 0, &mut account)
                    .is_err()
            );
        } else {
            assert!(tile.reader(&heap, &mut account).is_err());
        }
        drop(heap);
        if choice == 5 {
            let reader = tile.pin().historical_reader(&mut account)?;
            assert!(reader.application_outcome(0, &mut account).is_err());
            assert!(
                reader
                    .read_component(crate::HostChannel::Center, 0, 0, &mut account)
                    .is_err()
            );
        } else {
            assert!(tile.pin().historical_reader(&mut account).is_err());
        }
        assert_eq!(fault::reads(), 0);
    }
    Ok(())
}
