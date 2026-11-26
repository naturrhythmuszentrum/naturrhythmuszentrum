// SPDX-License-Identifier: AGPL-3.0-only
//! Isolated private representation and malformed-geometry guards precede all native reads.

use super::{TestResult, empty, fixture, granted, publish};
use crate::loan::{fault, payload::Numerical, registry::Bundle, retained_expm1::tests as laws};
use crate::{EndpointReadError, HandbackOutcome, HostChannel, LoanError};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Extract an ordinary complete private bundle after releasing its registry root.
/// This is a fault fixture only; public code cannot mutate a published bundle.
fn detached<P: Scalar + 'static>() -> TestResult<Bundle> {
    let (heap, _, tile, _) = publish(&[[P::ONE; 3]])?;
    let mut pin = tile.into_pin();
    drop(heap);
    let payload = Rc::get_mut(&mut pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    Rc::get_mut(&mut payload.bundle)
        .and_then(|owner| owner.payload.take())
        .ok_or_else(|| "unique bundle".into())
}

/// Isolate stored endpoint presence in both formats, and exercise genuine host law attachments.
/// Binary32 prepared publication is unsupported; its presence guard uses a private fault fixture.
pub(super) fn representations<P: Scalar + 'static>() -> TestResult {
    let mut bundle = detached::<P>()?;
    let mut donor = detached::<P>()?;
    bundle.endpoints = Some([
        donor.channels[0].take().ok_or("lower owner")?,
        donor.channels[1].take().ok_or("upper owner")?,
    ]);
    assert!(bundle.channels.iter().all(Option::is_some));
    assert!(bundle.retained_law.is_none());
    let (mut heap, mut account, tile, mask) = publish(&[[P::ONE; 3]])?;
    let source = tile.reader(&heap, &mut account)?;
    let mut view = source.masked_view(heap.mask(mask, mask.layout())?, &mut account)?;
    // Isolate the endpoint binding boundary from the prepared-origin validator.
    view.source.bundle = &bundle;
    refuse(view, &mut account);
    drop(tile);
    empty(&mut heap, &mut account)?;
    attached::<P>()
}

/// A complete host publication retains a real prepared law but no stored endpoints.
fn attached<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) = fixture::<P>(1, 1, [true; 2], laws::limits())?;
    let law = laws::prepare(&mut heap, &mut account)?;
    let mut loan = granted(heap.request_loan(declaration, &mut account))?;
    for role in HostChannel::ALL {
        loan.write_component(role, 0, 0, P::ONE, &mut account)?;
    }
    let attachment = laws::attach(law, &heap, &mut account)?;
    crate::loan::retained_law::RetainedLawAttachment::account_fixture(&mut heap, &mut loan.core)?;
    loan.core.retained_law = Some(Box::new(
        crate::loan::retained_law::RetainedLawAttachment::wrap_expm1(attachment),
    ));
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let mask = receipt.filled(HostChannel::Center).ok_or("mask")?.native();
    let source = tile.reader(&heap, &mut account)?;
    assert!(source.bundle.channels.iter().all(Option::is_some));
    assert!(source.bundle.endpoints.is_none());
    assert!(source.bundle.retained_law.is_some());
    let view = source.masked_view(heap.mask(mask, mask.layout())?, &mut account)?;
    refuse(view, &mut account);
    drop(tile);
    // Reclaiming the bundle destroys its retained law after the debit scan;
    // the next scan reconciles that law's prepaid release, as in attachment controls.
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        crate::Residence::Pending {
            elements: Some([0, 0]),
            bytes: Some(31_832)
        }
    ));
    empty(&mut heap, &mut account)
}

/// Each independent representation guard refuses before native reads or allocation attempts.
fn refuse<P: Scalar + 'static>(
    view: crate::MaskedTileReader<'_, P>,
    account: &mut crate::ExecutionAccount,
) {
    fault::reset_reads();
    let injection = fault::Injection::after(0);
    assert!(matches!(
        view.into_endpoint_view(account),
        Err(EndpointReadError::Loan(
            LoanError::UnsupportedEndpointRepresentation
        ))
    ));
    assert_eq!([fault::reads(), fault::Injection::attempts()], [0; 2]);
    drop(injection);
}

/// Corrupt each plane's columns or length, then all lengths together to isolate read bounds.
/// ´test:unit:loan-endpoint-view-layout-refusal´
#[test]
fn loan_endpoint_view_layout_refusal() -> TestResult {
    geometry::<f32>()?;
    geometry::<f64>()
}
fn geometry<P: Scalar + 'static>() -> TestResult {
    for case in 0..7 {
        let mut bundle = detached::<P>()?;
        for (index, channel) in bundle.channels.iter_mut().enumerate() {
            if case == 6 || index == case / 2 {
                let channel = channel.as_mut().ok_or("channel")?;
                let (rows, columns) = if case % 2 == 0 { (0, 1) } else { (1, 2) };
                match &mut channel.matrix {
                    Numerical::Single(matrix) => *matrix = faer::Mat::zeros(rows, columns),
                    Numerical::Double(matrix) => *matrix = faer::Mat::zeros(rows, columns),
                }
            }
        }
        let (mut heap, mut account, tile, mask) = publish(&[[P::ONE; 3]])?;
        let source = tile.reader(&heap, &mut account)?;
        let mut view = source.masked_view(heap.mask(mask, mask.layout())?, &mut account)?;
        view.source.bundle = &bundle;
        fault::reset_reads();
        let injection = fault::Injection::after(0);
        let result = view.into_endpoint_view(&mut account);
        let result = if case == 6 {
            result?.read_component(0, 0, &mut account).map(|_| ())
        } else {
            result.map(|_| ())
        };
        assert!(matches!(
            result,
            Err(EndpointReadError::Loan(LoanError::LayoutMismatch))
        ));
        assert_eq!([fault::reads(), fault::Injection::attempts()], [0; 2]);
        drop(injection);
        drop(tile);
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}
