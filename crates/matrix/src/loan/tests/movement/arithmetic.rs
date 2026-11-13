// SPDX-License-Identifier: AGPL-3.0-only
//! Existing square consumers use copied original incidence and total bounds in both formats.

use super::{TestResult, candidate, empty, product_setup, publish, ready};
use crate::{
    ApplicationError, ApplicationRequest, BundlePin, Elementwise, ExecutionAccount,
    ExecutionLimits, Heap, HostChannel, HostTileDeclaration, LoanError, PointwiseRequest,
    PublishedTile,
};
use nrz_bindings::{BindingSymbols, Descriptor};
use nrz_scalar::Scalar;

/// Gathered native copies feed Square without reinterpreting the result as a fresh host reading.
/// ´test:unit:loan-mapped-square-gathered-both-formats´
#[test]
fn loan_mapped_square_gathered_both_formats() -> TestResult {
    gathered::<f32>()?;
    gathered::<f64>()
}

/// Literal interval endpoints are independent of the production copy and enclosure algorithms.
fn gathered<P: Scalar + 'static>() -> TestResult {
    let super::Setup {
        mut heap,
        mut account,
        request,
        demand,
    } = super::setup::<P>()?;
    let owner = ready(&mut heap, &mut account, request)?;
    let owner = candidate(owner, &mut account)?;
    let copied = publish(&mut heap, &mut account, owner)?;
    super::check_values(&copied, &heap, &mut account)?;
    let request = square_request(&mut heap, &mut account, copied.into_pin())?;
    super::super::super::fault::reset_reads();
    let request = match heap.request_pointwise_square(request, &mut account) {
        ApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::UnsupportedContract),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(super::super::super::fault::reads(), 0);
    let squared = apply(&mut heap, &mut account, request)?;
    let reader = squared.reader(&heap, &mut account)?;
    for (cell, component, expected) in [
        (0, 0, [121.0, 64.0, 196.0]),
        (0, 1, [441.0, 289.0, 625.0]),
        (2, 0, [196.0, 121.0, 289.0]),
        (2, 1, [576.0, 400.0, 784.0]),
    ] {
        enclosure(&reader, &mut account, cell, component, expected)?;
    }
    assert!(!reader.contains(1, &mut account)?);
    assert!(!reader.contains(3, &mut account)?);
    drop(squared);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// Repeated original product cells feed Square while an outside original reader remains protected.
/// ´test:unit:loan-mapped-square-product-and-outside-reader´
#[test]
fn loan_mapped_square_product_and_outside_reader() -> TestResult {
    product::<f32>()?;
    product::<f64>()
}

/// Literal f64 endpoint squares give independent extrema at exactly represented small magnitudes.
fn product<P: Scalar + 'static>() -> TestResult {
    let (
        super::Setup {
            mut heap,
            mut account,
            request,
            demand,
        },
        outside,
    ) = product_setup::<P>()?;
    let owner = ready(&mut heap, &mut account, request)?;
    let owner = candidate(owner, &mut account)?;
    let copied = publish(&mut heap, &mut account, owner)?;
    let request = square_request(&mut heap, &mut account, copied.into_pin())?;
    let squared = apply(&mut heap, &mut account, request)?;
    let reader = squared.reader(&heap, &mut account)?;
    for (cells, expected) in [
        ([0, 1], [100.0, 49.0, 169.0]),
        ([2, 3], [121.0, 64.0, 196.0]),
        ([8, 9], [196.0, 121.0, 289.0]),
    ] {
        for cell in cells {
            enclosure(&reader, &mut account, cell, 0, expected)?;
        }
    }
    for cell in [4, 5, 6, 7] {
        assert!(!reader.contains(cell, &mut account)?);
    }
    drop(squared);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        crate::Residence::Pending { .. }
    ));
    assert_eq!(
        outside.reader(&heap, &mut account)?.read_component(
            HostChannel::Error,
            4,
            1,
            &mut account
        )?,
        P::from_u8(2)
    );
    drop(outside);
    empty(&mut heap, &mut account)
}

/// Fresh targets share only the exact original axis; the source's immutable occupation stays captured.
fn square_request<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    input: BundlePin<P>,
) -> TestResult<PointwiseRequest<P>> {
    let source = input
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or("source")?;
    let extent = source.declaration.extent;
    let axis = source.declaration.axis;
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [
            u32::try_from(extent.rows())?,
            u32::try_from(extent.columns())?,
        ],
        [true; 2],
        &mut account.ledger,
    )?;
    let destination =
        HostTileDeclaration::check(symbols, axis, crate::Coverage::RepresentedInputs, account)?;
    let cells = (0..extent.rows()).collect::<Vec<_>>();
    let demand = heap.create_mask(input.ticket()?.identity.layout, &cells, &mut account.ledger)?;
    let declarations = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(extent.columns())?;
    let codomain = crate::kernel_testing::axis(extent.rows())?;
    let original = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
    let output = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
    let operation = crate::OperationDraft::map_elementwise(original, output, Elementwise::Square)?
        .check(ExecutionLimits::default())?;
    Ok(PointwiseRequest {
        operation,
        input,
        destination,
        demand,
        limits: ExecutionLimits::default(),
    })
}

/// Retired ordinary demand storage is not needed after its checked selection has been captured.
fn apply<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    request: PointwiseRequest<P>,
) -> TestResult<PublishedTile<P>> {
    let demand = request.demand;
    let owner = match heap.request_chained_pointwise_square(request, account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    heap.retire_mask(demand, &mut account.ledger)?;
    let owner = candidate(owner, account)?;
    publish(heap, account, owner)
}

/// The exact represented centre and positive endpoint extrema bound the existing Square law.
fn enclosure<P: Scalar + 'static>(
    reader: &crate::TileReader<'_, P>,
    account: &mut ExecutionAccount,
    cell: usize,
    component: usize,
    expected: [f64; 3],
) -> TestResult {
    let center = reader
        .read_component(HostChannel::Center, cell, component, account)?
        .to_f64();
    let total = reader
        .read_component(HostChannel::Total, cell, component, account)?
        .to_f64();
    let error = reader
        .read_component(HostChannel::Error, cell, component, account)?
        .to_f64();
    assert_eq!(center, expected[0]);
    assert!(center - total <= expected[1]);
    assert!(center + total >= expected[2]);
    assert_eq!(error, local_error::<P>(expected[0]));
    assert_eq!(
        reader.numerical_error_scope(),
        crate::NumericalErrorScope::LocalOperation
    );
    Ok(())
}

/// Outward-neighbour multiplication encloses a literal square by one ulp on each side.
/// The exact dyadic endpoint difference is widened once more by subtraction. For the
/// literal squares in binades 6, 7, 8 and 9 this gives next_up(2^(e-(p-1))).
/// These IEEE encodings are derived from that neighbour rule, not from a kernel result.
pub(super) fn local_error<P: Scalar>(square: f64) -> f64 {
    match P::PRECISION {
        nrz_scalar::Precision::F32 => f64::from(f32::from_bits(if square < 128.0 {
            0x3700_0001
        } else if square < 256.0 {
            0x3780_0001
        } else if square < 512.0 {
            0x3800_0001
        } else {
            0x3880_0001
        })),
        nrz_scalar::Precision::F64 => f64::from_bits(if square < 128.0 {
            0x3d10_0000_0000_0001
        } else if square < 256.0 {
            0x3d20_0000_0000_0001
        } else if square < 512.0 {
            0x3d30_0000_0000_0001
        } else {
            0x3d40_0000_0000_0001
        }),
    }
}
