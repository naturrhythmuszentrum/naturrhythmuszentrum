// SPDX-License-Identifier: AGPL-3.0-only
//! Public native chains retain exact immediate capture and independently enumerated accounting.

mod boundaries;
mod bytes;
mod holes;
mod lifetime;
mod mapped;
mod oracle;
mod prefixes;
mod refusals;
mod resources;
mod seals;
mod spare;
mod stale;
mod states;
mod unary;
mod values;
mod work;

use super::{TestResult, candidate, published};
use crate::{
    ApplicationRequest, BundlePin, Elementwise, ExecutionAccount, ExecutionLimits, Heap,
    HostTileDeclaration, PointwiseRequest, PublishedTile,
};
use nrz_bindings::{BindingSymbols, Descriptor, Limits};

/// Canonical Product prefix keeps both hosts, its ordinary demand and all expired weak rows.
fn product(
    cells: usize,
    components: usize,
) -> TestResult<(
    Heap,
    ExecutionAccount,
    PublishedTile<f64>,
    crate::MaskVersion,
)> {
    let super::Fixture {
        mut heap,
        mut account,
        request,
    } = super::custom::<f64>(
        cells,
        components,
        [1; 2],
        2,
        false,
        crate::BinaryElementwise::Product,
        Limits::default(),
        |operand, _, _| (if operand == 0 { 2.0 } else { 3.0 }, 0.0),
    )?;
    let demand = request.demand;
    let owner = super::ready(heap.request_pointwise_product(request, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    Ok((heap, account, tile, demand))
}

/// Fresh independent symbols and the existing original operation bind the immediate operand.
fn request<P: nrz_scalar::Scalar + 'static>(
    heap: &mut Heap,
    input: BundlePin<P>,
    function: Elementwise,
    selection: usize,
    account: &mut ExecutionAccount,
) -> TestResult<PointwiseRequest<P>> {
    let bundle = input
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or("input")?;
    let extent = bundle.declaration.extent;
    let axis = bundle.declaration.axis;
    let shape = [
        u32::try_from(extent.rows())?,
        u32::try_from(extent.columns())?,
    ];
    let symbols =
        BindingSymbols::declare(&Descriptor::ratio(), shape, [true; 2], &mut account.ledger)?;
    let destination =
        HostTileDeclaration::check(symbols, axis, crate::Coverage::RepresentedInputs, account)?;
    let cells = (0..extent.rows())
        .filter(|cell| super::super::requested(*cell, selection))
        .collect::<Vec<_>>();
    let demand = heap.create_mask(input.ticket()?.identity.layout, &cells, &mut account.ledger)?;
    let declarations = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(extent.columns())?;
    let codomain = crate::kernel_testing::axis(extent.rows())?;
    let source = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
    let output = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
    let operation = crate::OperationDraft::map_elementwise(source, output, function)?
        .check(ExecutionLimits::default())?;
    Ok(PointwiseRequest {
        operation,
        input,
        destination,
        demand,
        limits: ExecutionLimits::default(),
    })
}

/// Dispatch chooses only the three public opt-in entry points.
fn admit(
    heap: &mut Heap,
    request: PointwiseRequest<f64>,
    function: Elementwise,
    account: &mut ExecutionAccount,
) -> ApplicationRequest<f64> {
    match function {
        Elementwise::Square => heap.request_chained_pointwise_square(request, account),
        Elementwise::SquareRoot => heap.request_chained_pointwise_square_root(request, account),
        Elementwise::Reciprocal => heap.request_chained_pointwise_reciprocal(request, account),
    }
}

/// Public admission, evaluation and publication are the only completion constructors.
fn apply(
    heap: &mut Heap,
    input: BundlePin<f64>,
    function: Elementwise,
    selection: usize,
    account: &mut ExecutionAccount,
) -> TestResult<PublishedTile<f64>> {
    let request = request(heap, input, function, selection, account)?;
    let owner = super::super::ready(admit(heap, request, function, account))?;
    let owner = candidate(owner, account)?;
    Ok(published(heap.publish_application(owner, account))?.0)
}

/// Prepare one or two upstream steps without reconciliation, preserving the canonical table history.
fn prefix(
    cells: usize,
    reciprocal: bool,
) -> TestResult<(
    Heap,
    ExecutionAccount,
    PublishedTile<f64>,
    Vec<crate::MaskVersion>,
)> {
    let (mut heap, mut account, mut tile, demand) = product(cells, 1)?;
    let mut demands = vec![demand];
    if reciprocal {
        let request = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
        demands.push(request.demand);
        let owner = super::super::ready(admit(
            &mut heap,
            request,
            Elementwise::SquareRoot,
            &mut account,
        ))?;
        let owner = candidate(owner, &mut account)?;
        (tile, _) = published(heap.publish_application(owner, &mut account))?;
    }
    Ok((heap, account, tile, demands))
}
