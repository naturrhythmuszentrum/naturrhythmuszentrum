// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed seals authenticate every detached and reader boundary without a numerical load.
use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::loan::{
    fault,
    registry::State,
    retained_expm1::{Attachment, Binding},
};
use crate::{ApplicationError, ApplicationEvaluation, ApplicationPublication, LoanError};
use std::rc::Rc;

/// Every changed pairing was individually valid before the private substitution.
fn alter(
    core: &mut super::super::super::ApplicationCore<f64>,
    choice: usize,
    law: Attachment,
) -> TestResult {
    match choice {
        0 => core.host.retained_law = Some(law),
        1 => core.endpoints.as_mut().ok_or("endpoints")?.swap(0, 1),
        2 => {
            let [lower, upper] = core.endpoints.as_mut().ok_or("endpoints")?;
            std::mem::swap(&mut lower.tokens[0], &mut upper.tokens[0]);
        }
        3 => core.evidence.prepared.as_mut().ok_or("operation")?.bits = 39,
        4 => {
            let operation = core.evidence.prepared.as_mut().ok_or("operation")?;
            operation.binding = super::laws::substituted_environment(operation.binding);
        }
        5 => core.endpoints = None,
        _ => return Err("substitution".into()),
    }
    Ok(())
}
/// Evaluation and publication reject wrong uses, endpoint roles, target and environment before loads.
#[test]
fn prepared_detached_pairing_at_evaluation_and_publication() -> TestResult {
    for publish in [false, true] {
        for substitution in 0..6 {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
            let second = heap.retain_retained_expm1(&request.law, &mut account)?;
            let law = super::laws::attach(second, &heap, &mut account)?;
            let mut owner = ready(heap.request_pointwise_expm1(request, &mut account))?;
            if publish {
                owner = candidate(owner, &mut account)?;
            }
            alter(&mut owner.core, substitution, law)?;
            fault::reset_reads();
            fault::reset_scalars();
            let injection = fault::Injection::after(0);
            if publish {
                assert!(matches!(
                    heap.publish_application(owner, &mut account),
                    ApplicationPublication::ApplicationRejected(
                        ApplicationError::Loan(LoanError::WrongLoan),
                        _
                    )
                ));
            } else {
                assert!(matches!(
                    owner.evaluate(&mut account),
                    ApplicationEvaluation::ApplicationNotStarted(
                        ApplicationError::Loan(LoanError::WrongLoan),
                        _
                    )
                ));
            }
            assert_eq!(fault::reads(), 0);
            assert_eq!(fault::scalars(), 0);
            assert_eq!(fault::Injection::attempts(), 0);
            drop(injection);
        }
    }
    Ok(())
}
/// Private current-root substitution leaves all exact debit records valid, isolating the origin gate.
#[test]
fn prepared_current_origin_pairing_and_endpoint_authentication() -> TestResult {
    for substitution in 0..6 {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let second = heap.retain_retained_expm1(&request.law, &mut account)?;
        let law = super::laws::attach(second, &heap, &mut account)?;
        let owner = candidate(
            ready(heap.request_pointwise_expm1(request, &mut account))?,
            &mut account,
        )?;
        let (mut tile, receipt) = published(heap.publish_application(owner, &mut account))?;
        let root = receipt.ticket().identity.root.slot;
        let pin_slot = heap.loans.groups[root].as_ref().ok_or("group")?.pin;
        let mut index = heap.loans.pins[pin_slot].take().ok_or("pin index")?;
        index.witness = None;
        heap.loans.groups[root].as_mut().ok_or("group")?.state = State::Reserved;
        let pin = Rc::get_mut(&mut tile.pin.record)
            .and_then(|record| record.payload.as_mut())
            .ok_or("unique pin")?;
        let bundle = Rc::get_mut(&mut pin.bundle)
            .and_then(|owner| owner.payload.as_mut())
            .ok_or("unique bundle")?;
        match substitution {
            0 => bundle.retained_law = Some(law),
            1 => bundle.endpoints.as_mut().ok_or("pair")?.swap(0, 1),
            2 => {
                let [lower, upper] = bundle.endpoints.as_mut().ok_or("pair")?;
                std::mem::swap(&mut lower.token, &mut upper.token);
            }
            3 => {
                bundle
                    .origin
                    .evidence
                    .as_mut()
                    .ok_or("origin")?
                    .prepared
                    .as_mut()
                    .ok_or("operation")?
                    .bits = 39;
            }
            4 => bundle.origin.evidence.as_mut().ok_or("origin")?.revision = 0,
            5 => bundle.origin.evidence = None,
            _ => return Err("substitution".into()),
        }
        heap.loans.groups[root].as_mut().ok_or("group")?.state =
            State::Published(Rc::clone(&pin.bundle));
        index.witness = Some(Rc::downgrade(&tile.pin.record));
        heap.loans.pins[pin_slot] = Some(index);
        fault::reset_reads();
        assert!(matches!(
            tile.reader(&heap, &mut account),
            Err(LoanError::WrongLoan)
        ));
        assert_eq!(fault::reads(), 0);
        drop(heap);
        assert!(matches!(
            tile.pin().historical_reader(&mut account),
            Err(LoanError::WrongLoan)
        ));
        assert_eq!(fault::reads(), 0);
    }
    Ok(())
}
/// Equal statements and represented intervals still belong to distinct proof-bound owners and accounts.
#[test]
fn prepared_foreign_owners_and_targets_refuse_before_allocation() -> TestResult {
    for substitution in 0..4 {
        let Fixture {
            mut heap,
            mut account,
            mut request,
        } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let foreign_account = substitution == 1;
        match substitution {
            0 | 1 => {
                let mut other_account = crate::ExecutionAccount::new(nrz_bindings::Budget::new(
                    super::laws::limits(),
                )?)?;
                request.law = super::laws::prepare(
                    &mut heap,
                    if foreign_account {
                        &mut other_account
                    } else {
                        &mut account
                    },
                )?;
            }
            2 => request.operation.bits = 39,
            3 => {
                request.operation.binding =
                    super::laws::substituted_environment(request.operation.binding);
            }
            _ => return Err("substitution".into()),
        }
        let expected = Binding::of(&request.law)?;
        let ticket = request.input.ticket()?;
        let injection = fault::Injection::after(0);
        let returned = match heap.request_pointwise_expm1(request, &mut account) {
            crate::Expm1ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(error),
                returned,
            ) => {
                if foreign_account {
                    assert!(matches!(error, LoanError::ForeignAccount));
                } else {
                    assert!(matches!(error, LoanError::WrongLoan));
                }
                returned
            }
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(Binding::of(&returned.law)?, expected);
        assert_eq!(returned.input.ticket()?, ticket);
        assert_eq!(fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
