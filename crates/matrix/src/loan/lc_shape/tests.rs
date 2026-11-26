// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC questions and independently enumerated tuples exercise managed input transport.

mod authority;
mod boundaries;
mod exact_scope;
pub(in crate::loan) mod fixture;
mod inventory;
mod observation;
pub(in crate::loan) mod output;
mod refusals;
mod schema;
mod scope;
use super::{LcInputShape, LcShapeAxes};
use crate::loan::application::tests::binary::oracle::Rational;
use crate::{HandbackOutcome, HostChannel, LcShapeRequest, LoanError};
use fixture::{TestResult, request, setup};
use nrz_scalar::Scalar;

/// Original inductance constraints, sample-qualified branches and outside readers survive transport.
/// ´test:unit:loan-lc-shape-original-question-lifecycle´
#[test]
fn loan_lc_shape_original_question_lifecycle() -> TestResult {
    lifecycle::<f32>()?;
    lifecycle::<f64>()
}
fn lifecycle<P: Scalar + 'static>() -> TestResult {
    for cells in [vec![], vec![0, 3, 4, 5, 8, 11], (0..12).collect()] {
        let (mut heap, mut account, schema, axes) = setup::<P>()?;
        let original = schema.question().relation().clone();
        assert_eq!(schema.input_index(), 0);
        assert_eq!(
            original.inputs()[0].quantity(),
            nrz_bindings::LcQuantity::Inductance
        );
        // Reuse this exact specialization with fresh occupations and changed centres.
        for shift in [0, 1] {
            let input = source(&schema, &axes, &mut heap, &mut account, shift)?;
            let request = request(
                &schema,
                &axes,
                input.into_pin(),
                &cells,
                &mut heap,
                &mut account,
            )?;
            let demand = request.demand;
            let mut application = match schema.request(&mut heap, &axes, request, &mut account) {
                LcShapeRequest::Ready(owner) => owner,
                other @ LcShapeRequest::Refused(..) => return Err(format!("{other:?}").into()),
            };
            application.evaluate(&mut account)?;
            let injection = crate::loan::fault::Injection::after(0);
            assert!(matches!(
                application.publish(&mut heap, &mut account),
                Err(crate::ApplicationError::Loan(LoanError::AllocationRefused))
            ));
            drop(injection);
            let result = application.publish(&mut heap, &mut account)?;
            drop(application);
            assert!(
                result
                    .schema()
                    .question()
                    .relation()
                    .shares_allocation(&original)
            );
            let outside = result.retain(&mut heap, &mut account)?;
            drop(result);
            heap.retire_mask(demand, &mut account.ledger)?;
            assert!(matches!(
                heap.residence(&mut account, 100_000)?,
                crate::Residence::Pending { .. }
            ));
            assert_eq!(outside.axes(), axes);
            let reader = outside.reader(&mut account)?;
            for (cell, source) in [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1].into_iter().enumerate() {
                let available = source != 1 || shift != 0;
                let filled = cells.contains(&cell) && available;
                assert_eq!(reader.contains(cell, &mut account)?, filled);
                if filled {
                    let expected = [(6 + source + shift, 16), (1, 16), (1, 32)];
                    for (channel, (numerator, denominator)) in
                        HostChannel::ALL.into_iter().zip(expected)
                    {
                        let value = reader.read_component(channel, cell, 0, &mut account)?;
                        assert_eq!(
                            Rational::binary(value.to_f64())?.compare(&Rational::fraction(
                                i64::try_from(numerator)?,
                                denominator
                            )?),
                            std::cmp::Ordering::Equal
                        );
                    }
                }
            }
            drop(outside);
            fixture::retire_layout(&mut heap, &mut account, demand.layout())?;
            fixture::empty(&mut heap, &mut account)?;
        }
    }
    Ok(())
}

/// Repair preserves the same pending producer; an incomplete instance stays a source hole.
fn source<P: Scalar + 'static>(
    schema: &LcInputShape<P>,
    axes: &LcShapeAxes,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    shift: usize,
) -> TestResult<crate::PublishedTile<P>> {
    let declaration = schema.source(axes, account)?;
    let mut loan = fixture::granted(heap.request_loan(declaration, account))?;
    let ticket = loan.ticket();
    for cell in 0..2 {
        let center = P::from_u8(u8::try_from(6 + cell + shift)?) / P::from_u8(16);
        for (role, value) in [
            (HostChannel::Center, center),
            (HostChannel::Total, P::ONE / P::from_u8(16)),
            (HostChannel::Error, P::ONE / P::from_u8(32)),
        ] {
            if cell != 1 || role != HostChannel::Error || shift != 0 {
                loan.write_component(role, cell, 0, value, account)?;
            }
        }
    }
    loan.write_component(HostChannel::Total, 0, 0, -P::ONE, account)?;
    let mut loan = match heap.handback(loan, account) {
        HandbackOutcome::Repair(LoanError::InvalidPayload { .. }, loan) => loan,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(loan.ticket(), ticket);
    loan.write_component(HostChannel::Total, 0, 0, P::ONE / P::from_u8(16), account)?;
    match heap.handback(loan, account) {
        HandbackOutcome::Published(tile, _) => Ok(tile),
        other => Err(format!("{other:?}").into()),
    }
}
