// SPDX-License-Identifier: AGPL-3.0-only
//! Refused requests preserve their exact pin before numerical allocation or kernel entry.

use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::{ApplicationError, ApplicationRequest, LoanError};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};

/// Wrong axes, unequal shapes and missing output companions cannot use equal-sized storage as proof.
/// ´test:unit:square_root-declaration-and-demand-refusals-before-allocation´
#[test]
fn square_root_declaration_and_demand_refusals_before_allocation() -> TestResult {
    for mode in 0..5 {
        let Fixture {
            mut heap,
            mut account,
            mut request,
        } = fixture::<f64>(2, 2, 3, 2, false, Limits::default())?;
        let source = request.input.ticket()?;
        let pointer = std::rc::Rc::as_ptr(&request.input.record);
        let mut symbolic = Budget::new(Limits::default())?;
        match mode {
            0 => request.destination.axis = heap.create_cell_axis(2, &mut symbolic)?,
            1 => {
                let symbols = BindingSymbols::declare(
                    &Descriptor::ratio(),
                    [2, 1],
                    [true, true],
                    &mut symbolic,
                )?;
                request.destination = crate::HostTileDeclaration::check(
                    symbols,
                    request.destination.axis,
                    crate::Coverage::RepresentedInputs,
                    &mut account,
                )?;
            }
            2 => {
                let symbols = BindingSymbols::declare(
                    &Descriptor::ratio(),
                    [2, 2],
                    [false, false],
                    &mut symbolic,
                )?;
                request.destination = crate::HostTileDeclaration::check(
                    symbols,
                    request.destination.axis,
                    crate::Coverage::RepresentedInputs,
                    &mut account,
                )?;
            }
            3 => heap.retire_mask(request.demand, &mut account.ledger)?,
            4 => {
                let layout = heap.create_cell_layout(
                    request.destination.axis,
                    crate::Layout::contiguous(crate::Extent::new(2, 2)?)?,
                    &mut account.ledger,
                )?;
                request.demand = heap.create_mask(layout, &[0], &mut account.ledger)?;
            }
            _ => return Err("fixture mode".into()),
        }
        let injection = crate::loan::fault::Injection::after(0);
        crate::loan::fault::reset_reads();
        crate::loan::fault::reset_scalars();
        let request = match heap.request_pointwise_square_root(request, &mut account) {
            ApplicationRequest::ApplicationRefused(error, request) => {
                let expected = match mode {
                    0 | 4 => matches!(error, ApplicationError::Loan(LoanError::LayoutMismatch)),
                    1 | 2 => matches!(
                        error,
                        ApplicationError::Loan(LoanError::UnsupportedContract)
                    ),
                    3 => matches!(
                        error,
                        ApplicationError::Loan(LoanError::Mask(crate::MaskError::Heap(
                            crate::HeapError::StaleHandle
                        )))
                    ),
                    _ => false,
                };
                assert!(expected, "mode {mode}: {error:?}");
                request
            }
            other @ ApplicationRequest::ApplicationReady(_) => {
                return Err(format!("{other:?}").into());
            }
        };
        assert_eq!(request.input.ticket()?, source);
        assert_eq!(std::rc::Rc::as_ptr(&request.input.record), pointer);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(crate::loan::fault::scalars(), 0);
        drop(injection);
    }
    Ok(())
}
/// A current result's independent authority cannot be mistaken for another host import.
/// ´test:unit:square_root-application-chains-refuse-before-allocation´
#[test]
fn square_root_application_chains_refuse_before_allocation() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
    let operation = request.operation.clone();
    let axis = request.destination.axis;
    let owner = candidate(
        ready(heap.request_pointwise_square_root(request, &mut account))?,
        &mut account,
    )?;
    let (tile, receipt) = published(heap.publish_application(owner, &mut account))?;
    let mut symbolic = Budget::new(Limits::default())?;
    let symbols =
        BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], &mut symbolic)?;
    let destination = crate::HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let demand = receipt
        .filled(crate::HostChannel::Center)
        .ok_or("filled")?
        .native();
    let request = crate::PointwiseRequest {
        operation,
        input: tile.into_pin(),
        destination,
        demand,
        limits: crate::ExecutionLimits::default(),
    };
    let injection = crate::loan::fault::Injection::after(0);
    match heap.request_pointwise_square_root(request, &mut account) {
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            request,
        ) => assert_eq!(request.input.ticket()?, receipt.ticket()),
        other => return Err(format!("{other:?}").into()),
    }
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    Ok(())
}
/// Foreign heap and account authentication never consume the rightful ready owner.
/// ´test:unit:square_root-foreign-authentication-preserves-owning-phases´
#[test]
fn square_root_foreign_authentication_preserves_owning_phases() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<f64>(1, 1, 3, 2, false, Limits::default())?;
    let mut foreign = crate::Heap::new(crate::HeapLimits::default())?;
    let request = match foreign.request_pointwise_square_root(request, &mut account) {
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::ForeignOwner),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    let owner = ready(heap.request_pointwise_square_root(request, &mut account))?;
    let ticket = owner.ticket();
    let mut other = crate::ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let owner = match owner.evaluate(&mut other) {
        crate::ApplicationEvaluation::ApplicationRejected(
            ApplicationError::Loan(LoanError::ForeignAccount),
            owner,
        ) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(owner.ticket(), ticket);
    let owner = match foreign.cancel_application(owner, &mut account) {
        crate::ApplicationCancellation::ApplicationRejected(
            ApplicationError::Loan(LoanError::ForeignOwner),
            owner,
        ) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(owner.ticket(), ticket);
    assert!(matches!(
        heap.cancel_application(owner, &mut account),
        crate::ApplicationCancellation::ApplicationCancelled(_)
    ));
    assert_eq!(
        heap.take_producer_outcome(ticket, &mut account)?,
        crate::ProducerOutcome::AlreadyDelivered
    );
    Ok(())
}
