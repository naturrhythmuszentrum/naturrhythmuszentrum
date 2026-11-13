// SPDX-License-Identifier: AGPL-3.0-only
//! Native same-format lineage is exercised independently of mapped movement.

use super::{TestResult, candidate, fixture, published, ready};
use crate::{HostChannel, HostTileDeclaration, PointwiseRequest};
use nrz_bindings::{BindingSymbols, Descriptor, Limits};
use nrz_scalar::Scalar;

/// Exact two-to-four-to-sixteen chains preserve both native formats and literal local numerical errors.
/// ´test:unit:loan-square-chain-native-formats´
#[test]
fn loan_square_chain_native_formats() -> TestResult {
    chain::<f32>()?;
    chain::<f64>()
}

/// Fresh symbols belong to the original source axis; equal dimensions are not substituted.
fn chain<P: Scalar + 'static>() -> TestResult {
    let super::Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<P>(1, 1, 1, 2, false, Limits::default())?;
    let first_demand = request.demand;
    let operation = request.operation.clone();
    let axis = request.destination.axis;
    let owner = ready(heap.request_pointwise_square(request, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    let input = tile.into_pin();
    let demand = heap.create_mask(input.ticket()?.identity.layout, &[0], &mut account.ledger)?;
    let symbols =
        BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true; 2], &mut account.ledger)?;
    let destination = HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let request = PointwiseRequest {
        operation,
        input,
        destination,
        demand,
        limits: crate::ExecutionLimits::default(),
    };
    let owner = ready(heap.request_chained_pointwise_square(request, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    let reader = tile.reader(&heap, &mut account)?;
    assert_eq!(
        reader.read_component(HostChannel::Center, 0, 0, &mut account)?,
        P::from_u8(16)
    );
    // The literal square 16 has upper ulp 2^-19 / 2^-48. The outward
    // subtraction rule gives the immediately following float as its local error.
    let expected_error = match P::PRECISION {
        nrz_scalar::Precision::F32 => f64::from(f32::from_bits(0x3600_0001)),
        nrz_scalar::Precision::F64 => f64::from_bits(0x3cf0_0000_0000_0001),
    };
    assert_eq!(
        reader
            .read_component(HostChannel::Error, 0, 0, &mut account)?
            .to_f64(),
        expected_error
    );
    assert!(
        reader
            .read_component(HostChannel::Total, 0, 0, &mut account)?
            .to_f64()
            >= expected_error
    );
    drop(tile);
    for mask in [first_demand, demand] {
        heap.retire_mask(mask, &mut account.ledger)?;
    }
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        crate::Residence::Empty { .. }
    ));
    Ok(())
}
