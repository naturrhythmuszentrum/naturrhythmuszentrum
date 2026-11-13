// SPDX-License-Identifier: AGPL-3.0-only
//! Cartesian views keep original factor values and companions without numerical copying.

use super::{TestResult, empty, fixture, owned_selection::publish};
use crate::{CellFactor, Extent, HostChannel, Layout, LoanError};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// Independently enumerated tuples determine every repeated original reading in both formats.
/// ´test:unit:loan-owned-product-original-factor-companions´
#[test]
fn loan_owned_product_original_factor_companions() -> TestResult {
    factors::<f32>()?;
    factors::<f64>()
}

/// Both ordered factors cross mask word boundaries with sparse original cells.
fn factors<P: Scalar + 'static>() -> TestResult {
    for factor in [CellFactor::Outer, CellFactor::Inner] {
        let (mut heap, mut account, declaration) =
            fixture::<P>(65, 2, [true, true], Limits::default())?;
        let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
        let axis = heap.mask(mask, mask.layout())?.axis();
        let other = heap.create_cell_axis(3, &mut account.ledger)?;
        let ordered = match factor {
            CellFactor::Outer => [axis, other],
            CellFactor::Inner => [other, axis],
        };
        let product = heap.create_cell_product(ordered[0], ordered[1], &mut account.ledger)?;
        let layout = heap.create_cell_layout(
            product.axis(),
            Layout::contiguous(Extent::new(195, 2)?)?,
            &mut account.ledger,
        )?;
        let selected =
            heap.expand_product_mask(product, factor, mask, layout, &mut account.ledger)?;
        let owned =
            heap.retain_product_tile(tile.pin(), selected, product, factor, &mut account)?;
        drop(tile);
        let reader = owned.reader(&mut account)?;
        check_factors(&reader, factor, &mut account)?;
        drop(owned);
        heap.retire_mask(selected, &mut account.ledger)?;
        heap.retire_cell_layout(layout, &mut account.ledger)?;
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}

/// The row-major tuple formula is the design oracle, independent of `CellProduct::coordinates`.
fn check_factors<P: Scalar + 'static>(
    reader: &crate::MaskedTileReader<'_, P>,
    factor: CellFactor,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    assert_eq!(reader.capacity(), 195);
    for original in [1_u8, 7, 64] {
        for repeat in 0..3 {
            let cell = match factor {
                CellFactor::Outer => usize::from(original) * 3 + repeat,
                CellFactor::Inner => repeat * 65 + usize::from(original),
            };
            assert_eq!(
                reader.read_component(HostChannel::Center, cell, 0, account)?,
                P::from_u8(original)
            );
            assert_eq!(
                reader.read_component(HostChannel::Total, cell, 1, account)?,
                P::from_u8(3)
            );
            assert_eq!(
                reader.read_component(HostChannel::Error, cell, 0, account)?,
                P::ONE
            );
        }
    }
    super::super::fault::reset_reads();
    assert!(matches!(
        reader.read_component(HostChannel::Center, 0, 0, account),
        Err(LoanError::CoverageMismatch)
    ));
    assert_eq!(super::super::fault::reads(), 0);
    Ok(())
}

/// Equal capacities cannot exchange original factors or product identities through compaction.
/// ´test:unit:loan-owned-product-association-refusals´
#[test]
fn loan_owned_product_association_refusals() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let axis = heap.mask(mask, mask.layout())?.axis();
    let other = heap.create_cell_axis(65, &mut account.ledger)?;
    let product = heap.create_cell_product(axis, other, &mut account.ledger)?;
    let reversed = heap.create_cell_product(other, axis, &mut account.ledger)?;
    let layout = heap.create_cell_layout(
        product.axis(),
        Layout::contiguous(Extent::new(4225, 2)?)?,
        &mut account.ledger,
    )?;
    let selected = heap.create_mask(layout, &[65], &mut account.ledger)?;
    for (map, factor) in [(product, CellFactor::Inner), (reversed, CellFactor::Inner)] {
        assert!(matches!(
            heap.retain_product_tile(tile.pin(), selected, map, factor, &mut account),
            Err(LoanError::LayoutMismatch)
        ));
    }
    let identity = heap.mask_correspondence(layout, layout, &mut account.ledger)?;
    let compact = heap.compact_mask(identity, selected, 1, 1, &mut account.ledger)?;
    assert!(matches!(
        heap.retain_product_tile(
            tile.pin(),
            compact,
            reversed,
            CellFactor::Inner,
            &mut account
        ),
        Err(LoanError::LayoutMismatch)
    ));
    let owned = heap.retain_product_tile(
        tile.pin(),
        compact,
        product,
        CellFactor::Outer,
        &mut account,
    )?;
    drop(tile);
    heap.retire_mask(selected, &mut account.ledger)?;
    heap.retire_cell_layout(layout, &mut account.ledger)?;
    let reader = owned.reader(&mut account)?;
    assert_eq!(reader.capacity(), 1);
    assert_eq!(
        reader.read_component(HostChannel::Center, 0, 0, &mut account)?,
        1.0
    );
    assert_eq!(
        reader.read_component(HostChannel::Total, 0, 1, &mut account)?,
        3.0
    );
    assert_eq!(
        reader.read_component(HostChannel::Error, 0, 0, &mut account)?,
        1.0
    );
    drop(owned);
    heap.retire_mask(compact, &mut account.ledger)?;
    heap.retire_cell_layout(compact.layout(), &mut account.ledger)?;
    empty(&mut heap, &mut account)
}

/// Selection/projection costs three native units before the one numerical load unit.
/// ´test:unit:loan-owned-product-literal-load-boundary´
#[test]
fn loan_owned_product_literal_load_boundary() -> TestResult {
    for available in [3, 4] {
        let (mut heap, mut account, declaration) =
            fixture::<f64>(65, 2, [true, true], Limits::default())?;
        let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
        let axis = heap.mask(mask, mask.layout())?.axis();
        let sample = heap.create_cell_axis(2, &mut account.ledger)?;
        let product = heap.create_cell_product(axis, sample, &mut account.ledger)?;
        let layout = heap.create_cell_layout(
            product.axis(),
            Layout::contiguous(Extent::new(130, 2)?)?,
            &mut account.ledger,
        )?;
        let selected = heap.create_mask(layout, &[2], &mut account.ledger)?;
        let owned = heap.retain_product_tile(
            tile.pin(),
            selected,
            product,
            CellFactor::Outer,
            &mut account,
        )?;
        let reader = owned.reader(&mut account)?;
        let remaining = account
            .limits()
            .work
            .checked_sub(account.usage().work)
            .and_then(|work| work.checked_sub(available))
            .ok_or("remaining Work")?;
        account.ledger.work(remaining, super::super::AT)?;
        super::super::fault::reset_reads();
        let result = reader.read_component(HostChannel::Center, 2, 0, &mut account);
        if available == 4 {
            assert_eq!(result?, 1.0);
            assert_eq!(super::super::fault::reads(), 1);
        } else {
            assert!(matches!(result, Err(LoanError::Budget(_))));
            assert_eq!(super::super::fault::reads(), 0);
        }
        assert_eq!(account.usage().work, account.limits().work);
    }
    Ok(())
}

/// An empty sample factor gives no addressable product cells and never reads source storage.
/// ´test:unit:loan-owned-product-empty-factor´
#[test]
fn loan_owned_product_empty_factor() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let axis = heap.mask(mask, mask.layout())?.axis();
    let sample = heap.create_cell_axis(0, &mut account.ledger)?;
    let product = heap.create_cell_product(axis, sample, &mut account.ledger)?;
    let layout = heap.create_cell_layout(
        product.axis(),
        Layout::contiguous(Extent::new(0, 2)?)?,
        &mut account.ledger,
    )?;
    let selected = heap.expand_product_mask(
        product,
        CellFactor::Outer,
        mask,
        layout,
        &mut account.ledger,
    )?;
    let owned = heap.retain_product_tile(
        tile.pin(),
        selected,
        product,
        CellFactor::Outer,
        &mut account,
    )?;
    drop(tile);
    // All retained payloads remain readable independently of the registry's lifetime.
    drop(heap);
    let reader = owned.reader(&mut account)?;
    assert_eq!(reader.capacity(), 0);
    super::super::fault::reset_reads();
    assert!(
        reader
            .read_component(HostChannel::Center, 0, 0, &mut account)
            .is_err()
    );
    assert_eq!(super::super::fault::reads(), 0);
    Ok(())
}
