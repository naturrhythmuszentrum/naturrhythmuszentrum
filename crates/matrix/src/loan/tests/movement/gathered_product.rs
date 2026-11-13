// SPDX-License-Identifier: AGPL-3.0-only
//! Native gathering resolves original product cells before projecting their source factor.

use super::{Setup, TestResult, candidate, empty, product_setup, publish, ready};
use crate::{HostChannel, HostTileDeclaration};
use nrz_scalar::Scalar;

/// Literal original product cells include a numerical hole and undemanded padding.
/// ´test:unit:loan-mapped-movement-gathered-product´
#[test]
fn loan_mapped_movement_gathered_product() -> TestResult {
    gathered::<f32>()?;
    gathered::<f64>()
}

/// Product cells one, three and nine come from original source cells zero, one and four.
fn gathered<P: Scalar + 'static>() -> TestResult {
    let (
        Setup {
            mut heap,
            mut account,
            mut request,
            demand,
        },
        outside,
    ) = product_setup::<P>()?;
    let crate::binding::NativeRecipe::Kernel(law) = &request.operation.recipe else {
        return Err("kernel".into());
    };
    let crate::kernels::contract::Recipe::Transform(map) = &law.recipe else {
        return Err("map".into());
    };
    let product = map.product().ok_or("product")?;
    let selected = heap.create_mask(demand.layout(), &[1, 3, 4, 9], &mut account.ledger)?;
    let identity =
        heap.mask_correspondence(demand.layout(), demand.layout(), &mut account.ledger)?;
    let compact = heap.compact_mask(identity, selected, 5, 4, &mut account.ledger)?;
    let axis = heap.mask(compact, compact.layout())?.axis();
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [5, 2],
        [true; 2],
        &mut account.ledger,
    )?;
    request.destination = HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    request.operation = super::operation_map(heap.cell_entry_map(
        crate::Extent::new(5, 2)?,
        compact,
        Some(product),
        &mut account.ledger,
    )?)?;
    request.demand = compact;
    let owner = ready(&mut heap, &mut account, request)?;
    let owner = candidate(owner, &mut account)?;
    let tile = publish(&mut heap, &mut account, owner)?;
    let reader = tile.reader(&heap, &mut account)?;
    for (cell, expected) in [(0, 10), (1, 11), (3, 14)] {
        assert_eq!(
            reader.read_component(HostChannel::Center, cell, 0, &mut account)?,
            P::from_u8(expected)
        );
        assert_eq!(
            reader.read_component(HostChannel::Error, cell, 1, &mut account)?,
            P::from_u8(2)
        );
    }
    for cell in [2, 4] {
        assert!(!reader.contains(cell, &mut account)?);
    }
    drop(tile);
    drop(outside);
    for mask in [demand, selected, compact] {
        heap.retire_mask(mask, &mut account.ledger)?;
    }
    super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    super::retire_copy_layout(&mut heap, &mut account, compact.layout())?;
    empty(&mut heap, &mut account)
}
