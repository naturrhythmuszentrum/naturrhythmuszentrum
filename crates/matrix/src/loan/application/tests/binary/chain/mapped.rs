// SPDX-License-Identifier: AGPL-3.0-only
//! Independently permuted physical operands retain logical correspondence through native chains.

use super::{TestResult, candidate, published, request};
use crate::{
    BinaryElementwise, BundlePin, Coverage, Elementwise, ExecutionAccount, ExecutionLimits,
    HandbackOutcome, Heap, HeapLimits, HostChannel, HostTileDeclaration, Layout, LoanRequest,
    PointwiseBinaryOperation, PointwiseBinaryRequest, RegionContract,
};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};

/// Different dense maps preserve sparse operand intersection, four-corner bounds and lineage.
/// ´test:unit:loan-dense-layout-binary-and-chained-kernels´
#[test]
fn loan_dense_layout_binary_and_chained_kernels() -> TestResult {
    binary_kernels::<f32>(false)?;
    binary_kernels::<f64>(false)?;
    kernels(false)
}

/// Gapped operands preserve the same independent enclosure and lineage laws.
/// ´test:unit:loan-affine-layout-binary-and-chained-kernels´
#[test]
fn loan_affine_layout_binary_and_chained_kernels() -> TestResult {
    binary_kernels::<f32>(true)?;
    binary_kernels::<f64>(true)?;
    kernels(true)
}

/// Run the same original mathematical problem through independently chosen physical maps.
fn kernels(gapped: bool) -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        let (mut heap, mut account, tile) = binary::<f64>(function, gapped, false)?;
        let next = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
        let owner = super::super::super::ready(
            heap.request_chained_pointwise_square_root(next, &mut account),
        )?;
        let owner = candidate(owner, &mut account)?;
        let (tile, _) = published(heap.publish_application(owner, &mut account))?;
        let reader = tile.reader(&heap, &mut account)?;
        assert!(!reader.contains(1, &mut account)?);
        for cell in [0, 2] {
            for component in 0..2 {
                let expected = match function {
                    BinaryElementwise::Product => [[2.0, 8.0], [12.0, 3.0]][cell / 2][component],
                    BinaryElementwise::Quotient => [2.0, 3.0][cell / 2],
                };
                assert_eq!(
                    reader.read_component(HostChannel::Center, cell, component, &mut account)?,
                    expected
                );
            }
        }
    }
    Ok(())
}

/// Sparse demand and a distinct error-only hole exercise each native mapped binary route.
fn binary_kernels<P: nrz_scalar::Scalar + 'static>(gapped: bool) -> TestResult {
    for sparse in [false, true] {
        for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
            binary::<P>(function, gapped, sparse)?;
        }
    }
    Ok(())
}

/// Publish through the public boundary and compare with literal original operand values.
fn binary<P: nrz_scalar::Scalar + 'static>(
    function: BinaryElementwise,
    gapped: bool,
    sparse: bool,
) -> TestResult<(Heap, ExecutionAccount, crate::PublishedTile<P>)> {
    let super::super::Fixture {
        mut heap,
        mut account,
        request: binary,
    } = fixture::<P>(function, gapped, sparse)?;
    let owner = match function {
        BinaryElementwise::Product => heap.request_pointwise_product(binary, &mut account),
        BinaryElementwise::Quotient => heap.request_pointwise_quotient(binary, &mut account),
    };
    let owner = candidate(super::super::ready(owner)?, &mut account)?;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    check_binary(&tile, &heap, &mut account, function, sparse)?;
    Ok((heap, account, tile))
}

/// The exact rational oracle receives literal original inputs, never recovered production addresses.
fn check_binary<P: nrz_scalar::Scalar + 'static>(
    tile: &crate::PublishedTile<P>,
    heap: &Heap,
    account: &mut ExecutionAccount,
    function: BinaryElementwise,
    sparse: bool,
) -> TestResult {
    let reader = tile.reader(heap, account)?;
    assert!(!reader.contains(1, account)?);
    assert_eq!(reader.contains(2, account)?, !sparse);
    for cell in [0, 2].into_iter().filter(|cell| !sparse || *cell == 0) {
        for component in 0..2 {
            let readings = [
                reader.read_component(HostChannel::Center, cell, component, account)?,
                reader.read_component(HostChannel::Total, cell, component, account)?,
                reader.read_component(HostChannel::Error, cell, component, account)?,
            ];
            super::super::oracle::tight_native::<P>(
                [values(0, cell)[component], values(1, cell)[component]],
                [bounds(0, cell)[component], bounds(1, cell)[component]],
                function,
                readings.map(nrz_scalar::Scalar::to_f64),
            )?;
        }
    }
    Ok(())
}

/// The original axis and symbols are independent of the two physical permutations.
fn fixture<P: nrz_scalar::Scalar + 'static>(
    function: BinaryElementwise,
    gapped: bool,
    sparse: bool,
) -> TestResult<super::super::Fixture<P>> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let axis = heap.create_cell_axis(3, &mut account.ledger)?;
    let output =
        BindingSymbols::declare(&Descriptor::ratio(), [3, 2], [true; 2], &mut account.ledger)?;
    let destination = HostTileDeclaration::check(
        output.clone(),
        axis,
        Coverage::RepresentedInputs,
        &mut account,
    )?;
    let left = source::<P>(&mut heap, &mut account, axis, 0, gapped)?;
    let right = source::<P>(&mut heap, &mut account, axis, 1, gapped)?;
    let demand = heap.create_mask(
        left.ticket()?.identity.layout,
        if sparse { &[0, 1] } else { &[0, 1, 2] },
        &mut account.ledger,
    )?;
    let owner = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(2)?;
    let codomain = crate::kernel_testing::axis(3)?;
    let region = RegionContract::check(crate::kernel_testing::shape(&owner, &domain, &codomain)?)?;
    let operation = PointwiseBinaryOperation::check(
        [region.clone(), region.clone()],
        region,
        [&left, &right],
        output,
        axis,
        function,
        ExecutionLimits::default(),
        &mut account,
    )?;
    Ok(super::super::Fixture {
        heap,
        account,
        request: PointwiseBinaryRequest {
            operation,
            left,
            right,
            destination,
            demand,
            limits: ExecutionLimits::default(),
        },
    })
}

/// The left map interleaves components; the right reverses separate planes with different padding.
fn source<P: nrz_scalar::Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    axis: crate::CellAxis,
    operand: usize,
    gapped: bool,
) -> TestResult<BundlePin<P>> {
    let symbols =
        BindingSymbols::declare(&Descriptor::ratio(), [3, 2], [true; 2], &mut account.ledger)?;
    let declaration =
        HostTileDeclaration::check(symbols, axis, Coverage::RepresentedInputs, account)?;
    let extent = crate::Extent::new(3, 2)?;
    let map = match (operand, gapped) {
        (0, false) => Layout::strided(extent, 9, 1, 2, 1)?,
        (0, true) => Layout::strided(extent, 12, 1, 3, 1)?,
        (_, false) => Layout::strided(extent, 11, 8, -1, -3)?,
        (_, true) => Layout::strided(extent, 9, 0, 2, 3)?,
    };
    let mut loan = match heap.request_loan_with_layout(declaration, map, account) {
        LoanRequest::Granted(loan) => loan,
        other @ LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for cell in 0..3 {
        loan.write_cell(
            HostChannel::Center,
            cell,
            &values(operand, cell).map(super::super::native::<P>),
            account,
        )?;
        let total = bounds(operand, cell);
        loan.write_cell(
            HostChannel::Total,
            cell,
            &total.map(super::super::native::<P>),
            account,
        )?;
        if operand == 0 || cell != 1 {
            loan.write_cell(
                HostChannel::Error,
                cell,
                &[total[0] / 2.0, total[1] / 2.0].map(super::super::native::<P>),
                account,
            )?;
        }
    }
    match heap.handback(loan, account) {
        HandbackOutcome::Published(tile, _) => Ok(tile.into_pin()),
        other => Err(format!("{other:?}").into()),
    }
}

/// Literal positive controls give exact nominal products, quotients and subsequent roots.
const fn values(operand: usize, cell: usize) -> [f64; 2] {
    if operand == 0 {
        [[4.0, 16.0], [25.0, 100.0], [36.0, 9.0]][cell]
    } else {
        [[1.0, 4.0], [4.0, 1.0], [4.0, 1.0]][cell]
    }
}

/// Distinct companion magnitudes expose wrong component and cell addressing on either operand.
const fn bounds(operand: usize, cell: usize) -> [f64; 2] {
    if operand == 0 {
        [[0.25, 0.5], [0.5, 0.25], [0.5, 0.25]][cell]
    } else {
        [[0.125, 0.25], [0.25, 0.125], [0.25, 0.125]][cell]
    }
}
