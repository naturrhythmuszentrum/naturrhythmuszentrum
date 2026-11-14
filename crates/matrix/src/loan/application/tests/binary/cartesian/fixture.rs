// SPDX-License-Identifier: AGPL-3.0-only
//! Independent factor tables retain different axes, extents and coordinate-dependent readings.

use super::super::{BinaryElementwise, Fixture, TestResult};
use crate::{
    BundlePin, Coverage, ExecutionAccount, ExecutionLimits, Heap, HostChannel, HostTileDeclaration,
    PointwiseBinaryOperation, PointwiseBinaryRequest, RegionContract,
};

/// The oracle's literal input formula is independent of production mapping or enclosure arithmetic.
pub(super) fn center(operand: usize, cell: usize, component: usize) -> TestResult<f64> {
    let sign = if (operand + cell) % 2 == 0 { 1.0 } else { -1.0 };
    Ok(sign
        * ([8.0, 3.0][operand]
            + f64::from(u32::try_from(cell)?)
            + f64::from(u32::try_from(component)?)))
}

/// Dyadic radii distinguish both original coordinates independently of physical layout.
pub(super) fn total(operand: usize, cell: usize, component: usize) -> TestResult<f64> {
    Ok([0.25, 0.5][operand]
        + f64::from(u32::try_from(cell)?) / 1024.0
        + f64::from(u32::try_from(component)?) / 256.0)
}

/// Error-only holes close the entire original factor cell before product expansion.
pub(super) fn host(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    axis: crate::CellAxis,
    operand: usize,
    holes: bool,
    components: usize,
    domain: bool,
) -> TestResult<BundlePin<f64>> {
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [u32::try_from(axis.capacity())?, u32::try_from(components)?],
        [true; 2],
        &mut account.ledger,
    )?;
    let declaration =
        HostTileDeclaration::check(symbols, axis, Coverage::RepresentedInputs, account)?;
    let extent = crate::Extent::new(axis.capacity(), components)?;
    let layout = if operand == 0 {
        crate::Layout::strided(extent, extent.entries(), 0, isize::try_from(components)?, 1)?
    } else {
        let offset = isize::try_from(extent.entries().saturating_sub(1))?;
        crate::Layout::strided(
            extent,
            extent.entries(),
            offset,
            -isize::try_from(components)?,
            -1,
        )?
    };
    let mut loan = match heap.request_loan_with_layout(declaration, layout, account) {
        crate::LoanRequest::Granted(loan) => loan,
        crate::LoanRequest::Refused(error) => return Err(error.into()),
    };
    for cell in 0..axis.capacity() {
        for component in 0..components {
            let inner = operand == 1;
            let selected_cell = cell == 1;
            let selected_component = component == 1;
            let crossing = domain && inner && selected_cell && selected_component;
            loan.write_component(
                HostChannel::Center,
                cell,
                component,
                if crossing {
                    0.0
                } else {
                    center(operand, cell, component)?
                },
                account,
            )?;
            loan.write_component(
                HostChannel::Total,
                cell,
                component,
                total(operand, cell, component)?,
                account,
            )?;
            let missing = holes && cell == operand;
            if !missing {
                loan.write_component(HostChannel::Error, cell, component, 0.0, account)?;
            }
        }
    }
    match heap.handback(loan, account) {
        crate::HandbackOutcome::Published(tile, _) => Ok(tile.into_pin()),
        other => Err(format!("{other:?}").into()),
    }
}

/// Original factor rows and their Cartesian output are frozen in one descriptor snapshot.
pub(super) fn setup(
    rows: [usize; 2],
    selection: usize,
    holes: bool,
    function: BinaryElementwise,
) -> TestResult<(Fixture, crate::CellProduct)> {
    setup_domain(rows, selection, holes, function, false)
}

/// Domain fixtures alter only one inner factor component, preserving every original coordinate.
pub(super) fn setup_domain(
    rows: [usize; 2],
    selection: usize,
    holes: bool,
    function: BinaryElementwise,
    domain: bool,
) -> TestResult<(Fixture, crate::CellProduct)> {
    let mut heap = Heap::new(crate::HeapLimits::default())?;
    let mut account =
        ExecutionAccount::new(nrz_bindings::Budget::new(nrz_bindings::Limits::default())?)?;
    let axes = [
        heap.create_cell_axis(rows[0], &mut account.ledger)?,
        heap.create_cell_axis(rows[1], &mut account.ledger)?,
    ];
    let left = host(&mut heap, &mut account, axes[0], 0, holes, 2, domain)?;
    let right = host(&mut heap, &mut account, axes[1], 1, holes, 2, domain)?;
    let product = heap.create_cell_product(axes[0], axes[1], &mut account.ledger)?;
    let cells = rows[0] * rows[1];
    let extent = crate::Extent::new(cells, 2)?;
    let layout = heap.create_cell_layout(
        product.axis(),
        crate::Layout::contiguous(extent)?,
        &mut account.ledger,
    )?;
    let selected = (0..cells)
        .filter(|cell| super::super::requested(*cell, selection))
        .collect::<Vec<_>>();
    let demand = heap.create_mask(layout, &selected, &mut account.ledger)?;
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [u32::try_from(cells)?, 2],
        [true; 2],
        &mut account.ledger,
    )?;
    let destination = HostTileDeclaration::check(
        symbols,
        product.axis(),
        Coverage::RepresentedInputs,
        &mut account,
    )?;
    let operation = operation(
        [&left, &right],
        &destination,
        product,
        layout,
        function,
        &mut account,
    )?;
    Ok((
        Fixture {
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
        },
        product,
    ))
}

/// Construct original regions once from independently retained factor extents.
pub(super) fn operation(
    sources: [&BundlePin<f64>; 2],
    destination: &HostTileDeclaration<f64>,
    product: crate::CellProduct,
    layout: crate::CellLayoutVersion,
    function: BinaryElementwise,
    account: &mut ExecutionAccount,
) -> TestResult<PointwiseBinaryOperation> {
    let rows = product.factors().map(crate::CellAxis::capacity);
    let cells = product.axis().capacity();
    let declarations = crate::kernel_testing::owner()?;
    let region = |rows, components| -> TestResult<RegionContract> {
        Ok(RegionContract::check(crate::kernel_testing::shape(
            &declarations,
            &crate::kernel_testing::axis(components)?,
            &crate::kernel_testing::axis(rows)?,
        )?)?)
    };
    Ok(PointwiseBinaryOperation::check_cartesian(
        [
            region(
                rows[0],
                crate::loan::application::binary::bundle(sources[0])?
                    .declaration
                    .extent
                    .columns(),
            )?,
            region(
                rows[1],
                crate::loan::application::binary::bundle(sources[1])?
                    .declaration
                    .extent
                    .columns(),
            )?,
        ],
        region(cells, 2)?,
        sources,
        destination.symbols.clone(),
        product,
        layout,
        function,
        ExecutionLimits::default(),
        account,
    )?)
}
