// SPDX-License-Identifier: AGPL-3.0-only
//! Self-valid law uses cannot substitute for the exact operation or historical endpoint seal.

use super::{Fixture, TestResult, candidate, fixture, published, ready};
use crate::loan::{
    fault::Injection,
    retained_expm1::{Attachment, Binding},
};
use crate::{ApplicationError, Expm1ApplicationRequest, LoanError};
use std::rc::Rc;

/// Same owner, different funded use: refusal precedes every managed allocation and native load.
#[test]
fn prepared_application_exact_use_pairing_before_allocation() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        mut request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let original_binding = Binding::of(&request.law)?;
    let original_pin = request.input.ticket()?;
    let other = heap.retain_retained_expm1(&request.law, &mut account)?;
    let other_binding = Binding::of(&other)?;
    assert_ne!(original_binding, other_binding);
    let original = std::mem::replace(&mut request.law, other);
    let injection = Injection::after(0);
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let request = match heap.request_pointwise_expm1(request, &mut account) {
        Expm1ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::WrongLoan),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(Binding::of(&request.law)?, other_binding);
    assert_eq!(request.input.ticket()?, original_pin);
    assert_eq!(Injection::attempts(), 0);
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::scalars(), 0);
    drop(injection);
    drop((request, original));
    Ok(())
}
/// Heap destruction cannot make a substituted self-valid law or swapped endpoint record readable.
#[test]
fn prepared_historical_origin_authentication_before_payload_load() -> TestResult {
    for substitution in 0..5 {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
        let second = heap.retain_retained_expm1(&request.law, &mut account)?;
        let binding = Binding::of(&second)?;
        let attachment = Attachment::prepare(second, binding, &heap, &mut account.ledger)
            .map_err(|(error, _)| error)?;
        let owner = candidate(
            ready(heap.request_pointwise_expm1(request, &mut account))?,
            &mut account,
        )?;
        let (tile, _) = published(heap.publish_application(owner, &mut account))?;
        let mut pin = tile.into_pin();
        drop(heap);
        assert!(
            pin.historical_reader(&mut account)?
                .expm1_endpoints(0, 0, &mut account)
                .is_ok()
        );
        let record = Rc::get_mut(&mut pin.record).ok_or("unique pin")?;
        let bundle = Rc::get_mut(&mut record.payload.as_mut().ok_or("pin")?.bundle)
            .and_then(|owner| owner.payload.as_mut())
            .ok_or("unique bundle")?;
        match substitution {
            0 => bundle.retained_law = Some(attachment),
            1 => bundle.endpoints.as_mut().ok_or("pair")?.swap(0, 1),
            2 => {
                let [lower, upper] = bundle.endpoints.as_mut().ok_or("pair")?;
                std::mem::swap(&mut lower.token, &mut upper.token);
            }
            3 => bundle.origin.evidence.as_mut().ok_or("evidence")?.revision = 0,
            4 => bundle.endpoints = None,
            _ => return Err("substitution fixture".into()),
        }
        crate::loan::fault::reset_reads();
        assert!(pin.historical_reader(&mut account).is_err());
        assert_eq!(crate::loan::fault::reads(), 0);
    }
    Ok(())
}
/// The original target, context, coordinate, domain and declaration identities remain mandatory.
#[test]
fn prepared_correspondence_refuses_substituted_originals() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let other = super::laws::prepare(&mut heap, &mut account)?;
    let expected = request.law.authority()?.request().clone();
    let foreign = other.authority()?.request();
    for changed in 0..9 {
        let original = &request.operation;
        let mut source = original.source.clone();
        let mut destination = original.destination.clone();
        if changed == 6 {
            source = nrz_bindings::BindingSymbols::declare(
                &nrz_bindings::Descriptor::ratio(),
                [1, 1],
                [true, true],
                &mut account.ledger,
            )?;
        }
        if changed == 7 {
            destination = source.clone();
        }
        let input = original.operation.inputs()[0].clone();
        let output = if changed == 8 {
            let declarations = crate::kernel_testing::owner()?;
            let domain = input.shape().domain().clone();
            let codomain = input.shape().codomain().clone();
            let shape = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
            let regions =
                crate::OperationDraft::copy(vec![shape]).check(crate::ExecutionLimits::default())?;
            regions.outputs()[0].clone()
        } else {
            original.operation.outputs()[0].clone()
        };
        let result = crate::PreparedExpm1Operation::check(
            &request.law,
            if changed == 0 {
                &foreign.original.target
            } else {
                &expected.original.target
            },
            if changed == 1 {
                &foreign.original.context
            } else {
                &expected.original.context
            },
            if changed == 2 {
                &foreign.original.coordinates[0].coordinate
            } else {
                &expected.original.coordinates[0].coordinate
            },
            if changed == 3 {
                [-1.0, 0.0]
            } else {
                [-2.0, 0.0]
            },
            if changed == 4 { 39 } else { 40 },
            if changed == 5 { 2 } else { 1 },
            input,
            output,
            source,
            destination,
            original.axis,
            &mut account,
        );
        assert!(result.is_err(), "substitution {changed}");
    }
    Ok(())
}
