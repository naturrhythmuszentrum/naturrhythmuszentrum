// SPDX-License-Identifier: AGPL-3.0-only
//! Projected unary consumers authenticate application inputs in both native formats.

use super::super::{Setup, TestResult, candidate, empty, publish};
use crate::kernel_oracle::Rational;
use crate::{ApplicationRequest, Elementwise, HostChannel};
use nrz_scalar::Scalar;

/// Square followed by reciprocal/root encloses independent original endpoint images.
/// ´test:unit:loan-projected-unary-application-inputs´
#[test]
fn loan_projected_unary_application_inputs() -> TestResult {
    for function in [Elementwise::Reciprocal, Elementwise::SquareRoot] {
        application_input::<f32>(function)?;
        application_input::<f64>(function)?;
    }
    Ok(())
}

/// Native incidence is rebound once to the published Square result, retaining its holes.
fn application_input<P: Scalar + 'static>(function: Elementwise) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = super::super::setup::<P>()?;
    super::square(&mut request)?;
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let owner = candidate(owner, &mut account)?;
    let tile = publish(&mut heap, &mut account, owner)?;
    let mut request =
        super::super::arithmetic::square_request(&mut heap, &mut account, tile.into_pin())?;
    let selected = request.demand;
    let map = heap.cell_entry_map(
        request.operation.inputs()[0].shape().extent(),
        selected,
        None,
        &mut account.ledger,
    )?;
    request.operation = super::super::operation_map(map)?;
    super::unary::operation(&mut request, function)?;
    crate::loan::fault::reset_reads();
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    assert_eq!(crate::loan::fault::reads(), 0);
    let owner = candidate(owner, &mut account)?;
    assert_eq!(crate::loan::fault::reads(), 8);
    let tile = publish(&mut heap, &mut account, owner)?;
    let reader = tile.reader(&heap, &mut account)?;
    for (cell, component, endpoints) in [
        (0, 0, [8, 14]),
        (0, 1, [17, 25]),
        (2, 0, [11, 17]),
        (2, 1, [20, 28]),
    ] {
        let center = reader
            .read_component(HostChannel::Center, cell, component, &mut account)?
            .to_f64();
        let total = reader
            .read_component(HostChannel::Total, cell, component, &mut account)?
            .to_f64();
        for endpoint in endpoints {
            let expected = match function {
                Elementwise::Reciprocal => Rational::new(1, endpoint * endpoint)?,
                Elementwise::SquareRoot => Rational::new(endpoint, 1)?,
                Elementwise::Square => return Err("consumer fixture".into()),
            };
            assert!(expected.covered(center, total)?);
        }
    }
    assert!(!reader.contains(1, &mut account)?);
    assert!(!reader.contains(3, &mut account)?);
    drop(tile);
    heap.retire_mask(selected, &mut account.ledger)?;
    heap.retire_mask(demand, &mut account.ledger)?;
    super::super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}
