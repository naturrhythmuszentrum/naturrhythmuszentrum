// SPDX-License-Identifier: AGPL-3.0-only
//! Combined native products retain sample-specific membership and original companions.

use super::{TestResult, candidate, empty, product_setup, publish, ready};
use crate::{
    BundlePin, CellOutcome, ExecutionAccount, ExecutionLimits, Heap, HostChannel,
    HostTileDeclaration, MaskVersion, PointwiseRequest, PublishedTile,
};
use nrz_bindings::{BindingSymbols, Descriptor};
use nrz_scalar::Scalar;

/// An instance/sample product crossed with branches survives compaction without losing any component.
/// ´test:unit:loan-mapped-movement-combined-products-and-compaction´
#[test]
fn loan_mapped_movement_combined_products_and_compaction() -> TestResult {
    combined::<f32>()?;
    combined::<f64>()
}

/// Two samples select different branches; a requested original hole stays missing after compaction.
fn combined<P: Scalar + 'static>() -> TestResult {
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
    let samples = publish(&mut heap, &mut account, owner)?;
    let request = branch_request(&mut heap, &mut account, samples.into_pin())?;
    let branch_mask = request.demand;
    let owner = ready(&mut heap, &mut account, request)?;
    let owner = candidate(owner, &mut account)?;
    let branches = publish(&mut heap, &mut account, owner)?;
    let request = compact_request(&mut heap, &mut account, branches.into_pin())?;
    let compact = request.demand;
    let owner = ready(&mut heap, &mut account, request)?;
    super::super::super::fault::reset_reads();
    let owner = candidate(owner, &mut account)?;
    assert_eq!(super::super::super::fault::reads(), 36);
    let result = publish(&mut heap, &mut account, owner)?;
    for mask in [demand, branch_mask, compact] {
        heap.retire_mask(mask, &mut account.ledger)?;
    }
    check(&result, &heap, &mut account)?;
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        crate::Residence::Pending { .. }
    ));
    check(&result, &heap, &mut account)?;
    assert_eq!(
        outside.reader(&heap, &mut account)?.read_component(
            HostChannel::Error,
            4,
            1,
            &mut account
        )?,
        P::from_u8(2)
    );
    drop(result);
    drop(outside);
    for mask in [demand, branch_mask, compact] {
        super::retire_copy_layout(&mut heap, &mut account, mask.layout())?;
    }
    empty(&mut heap, &mut account)
}

/// Independent row-major triples choose branches {0,1} for sample 0 and {1,2} for sample 1.
fn branch_request<P: Scalar + 'static>(
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
    let branches = heap.create_cell_axis(3, &mut account.ledger)?;
    let product =
        heap.create_cell_product(source.declaration.axis, branches, &mut account.ledger)?;
    let layout = heap.create_cell_layout(
        product.axis(),
        crate::Layout::contiguous(crate::Extent::new(30, 2)?)?,
        &mut account.ledger,
    )?;
    let demand = heap.create_mask(layout, &[0, 1, 4, 5, 6, 12, 29], &mut account.ledger)?;
    request(
        input,
        demand,
        10,
        30,
        Some((product, crate::CellFactor::Outer)),
        heap,
        account,
    )
}

/// A separate literal map keeps six filled original triples, one hole and one padding cell.
fn compact_request<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    input: BundlePin<P>,
) -> TestResult<PointwiseRequest<P>> {
    let layout = input.ticket()?.identity.layout;
    let selected = heap.create_mask(layout, &[0, 1, 4, 5, 6, 12, 29], &mut account.ledger)?;
    let identity = heap.mask_correspondence(layout, layout, &mut account.ledger)?;
    let demand = heap.compact_mask(identity, selected, 8, 7, &mut account.ledger)?;
    heap.retire_mask(selected, &mut account.ledger)?;
    request(input, demand, 30, 8, None, heap, account)
}

/// Every new numerical target is declared once on the demand's exact combined axis.
fn request<P: Scalar + 'static>(
    input: BundlePin<P>,
    demand: MaskVersion,
    source_rows: usize,
    rows: usize,
    product: Option<(crate::CellProduct, crate::CellFactor)>,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> TestResult<PointwiseRequest<P>> {
    let axis = heap.mask(demand, demand.layout())?.axis();
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [u32::try_from(rows)?, 2],
        [true; 2],
        &mut account.ledger,
    )?;
    let destination =
        HostTileDeclaration::check(symbols, axis, crate::Coverage::RepresentedInputs, account)?;
    Ok(PointwiseRequest {
        operation: super::operation_map(heap.cell_entry_map(
            crate::Extent::new(source_rows, 2)?,
            demand,
            product,
            &mut account.ledger,
        )?)?,
        input,
        destination,
        demand,
        limits: ExecutionLimits::default(),
    })
}

/// Literal original tags supply the oracle; no production product or gather projection is queried.
fn check<P: Scalar + 'static>(
    tile: &PublishedTile<P>,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let reader = tile.reader(heap, account)?;
    assert_eq!(reader.capacity(), 8);
    for (cell, center) in [
        (0, [10, 20]),
        (1, [10, 20]),
        (2, [10, 20]),
        (3, [10, 20]),
        (4, [11, 21]),
        (6, [14, 24]),
    ] {
        for (channel, values) in [
            (HostChannel::Center, center),
            (HostChannel::Total, [3, 4]),
            (HostChannel::Error, [1, 2]),
        ] {
            for (component, expected) in values.into_iter().enumerate() {
                assert_eq!(
                    reader.read_component(channel, cell, component, account)?,
                    P::from_u8(expected)
                );
            }
        }
        assert!(matches!(
            reader.application_outcome(cell, account)?,
            CellOutcome::Completed
        ));
    }
    assert!(matches!(
        reader.application_outcome(5, account)?,
        CellOutcome::MissingInput
    ));
    assert!(matches!(
        reader.application_outcome(7, account)?,
        CellOutcome::NotRequested
    ));
    assert!(!reader.contains(5, account)?);
    assert!(!reader.contains(7, account)?);
    Ok(())
}
