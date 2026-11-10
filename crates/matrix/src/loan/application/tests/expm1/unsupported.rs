// SPDX-License-Identifier: AGPL-3.0-only
//! Other numerical contracts and input families cannot enter the closed prepared adapter.
use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::loan::fault::Injection;
use crate::{ApplicationError, Expm1ApplicationRequest, LoanError};

/// A checked Square is not correspondence for the explicit retained scalar request.
#[test]
fn prepared_application_refuses_substituted_square() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        mut request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    request.operation.operation = crate::OperationDraft::map_elementwise(
        request.operation.operation.inputs()[0].shape().clone(),
        request.operation.operation.outputs()[0].shape().clone(),
        crate::Elementwise::Square,
    )?
    .check(crate::ExecutionLimits::default())?;
    let injection = Injection::after(0);
    assert!(matches!(
        heap.request_pointwise_expm1(request, &mut account),
        Expm1ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            _
        )
    ));
    assert_eq!(Injection::attempts(), 0);
    drop(injection);
    Ok(())
}
/// Even a valid five-role result cannot become this adapter's host-source operand.
#[test]
fn prepared_application_chain_is_unsupported_before_allocation() -> TestResult {
    let mut setup = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let mut second = super::shared::additional(&mut setup)?;
    let Fixture {
        mut heap,
        mut account,
        request,
    } = setup;
    let first = candidate(
        ready(heap.request_pointwise_expm1(request, &mut account))?,
        &mut account,
    )?;
    let (tile, _) = published(heap.publish_application(first, &mut account))?;
    second.input = tile.into_pin();
    let ticket = second.input.ticket()?;
    let binding = crate::loan::retained_expm1::Binding::of(&second.law)?;
    let injection = Injection::after(0);
    let returned = match heap.request_pointwise_expm1(second, &mut account) {
        Expm1ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(returned.input.ticket()?, ticket);
    assert_eq!(
        crate::loan::retained_expm1::Binding::of(&returned.law)?,
        binding
    );
    assert_eq!(Injection::attempts(), 0);
    drop(injection);
    Ok(())
}
/// Zero total/error still denotes the unsupported bounded-host representation.
#[test]
fn prepared_bounded_host_refuses_even_with_zero_companions() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        mut request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [1, 1],
        [true; 2],
        &mut account.ledger,
    )?;
    let declaration = crate::HostTileDeclaration::check(
        symbols,
        request.destination.axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let mut loan = match heap.request_loan(declaration, &mut account) {
        crate::LoanRequest::Granted(owner) => owner,
        other @ crate::LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for (channel, value) in crate::HostChannel::ALL.into_iter().zip([-0.5, 0.0, 0.0]) {
        loan.write_component(channel, 0, 0, value, &mut account)?;
    }
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        crate::HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    request.input = tile.into_pin();
    request.demand = heap.create_mask(
        receipt
            .filled(crate::HostChannel::Center)
            .ok_or("filled")?
            .native()
            .layout(),
        &[0],
        &mut account.ledger,
    )?;
    let injection = Injection::after(0);
    crate::loan::fault::reset_reads();
    assert!(matches!(
        heap.request_pointwise_expm1(request, &mut account),
        Expm1ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            _
        )
    ));
    assert_eq!(Injection::attempts(), 0);
    assert_eq!(crate::loan::fault::reads(), 0);
    drop(injection);
    Ok(())
}
