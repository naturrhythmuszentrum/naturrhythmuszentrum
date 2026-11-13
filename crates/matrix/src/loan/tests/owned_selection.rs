// SPDX-License-Identifier: AGPL-3.0-only
//! Owned selections preserve independent cell tags and every retained native owner.

use super::{TestResult, empty, fixture, granted};
use crate::{
    ExecutionAccount, Extent, HandbackOutcome, Heap, HeapError, HostChannel, HostTileDeclaration,
    Layout, LoanError, MaskError, MaskVersion, PublishedTile, Residence,
};
use nrz_bindings::{Budget, Limits};
use nrz_scalar::Scalar;

/// Publish independently tagged source cells and all required companion channels.
pub(super) fn publish<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    declaration: HostTileDeclaration<P>,
) -> TestResult<(PublishedTile<P>, MaskVersion)> {
    let mut loan = granted(heap.request_loan(declaration, account))?;
    for cell in [1, 7, 64] {
        let value = P::from_u8(u8::try_from(cell)?);
        loan.write_cell(HostChannel::Center, cell, &[value, P::ONE], account)?;
        loan.write_cell(
            HostChannel::Total,
            cell,
            &[P::from_u8(2), P::from_u8(3)],
            account,
        )?;
        loan.write_cell(HostChannel::Error, cell, &[P::ONE, P::from_u8(2)], account)?;
    }
    match heap.handback(loan, account) {
        HandbackOutcome::Published(tile, receipt) => {
            let mask = receipt
                .filled(HostChannel::Center)
                .ok_or("filled center")?
                .native();
            Ok((tile, mask))
        }
        other => Err(format!("{other:?}").into()),
    }
}

/// The original source, compacted words and map survive independently of caller pins.
/// ´test:unit:loan-owned-selection-original-companions´
#[test]
fn loan_owned_selection_original_companions() -> TestResult {
    companions::<f32>()?;
    companions::<f64>()
}

/// Explicit source tags, not the gather implementation, determine expected values.
fn companions<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<P>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let identity = heap.mask_correspondence(mask.layout(), mask.layout(), &mut account.ledger)?;
    let compact = heap.compact_mask(identity, mask, 4, 3, &mut account.ledger)?;
    let selected = heap.create_mask(compact.layout(), &[0, 2], &mut account.ledger)?;
    let owned = heap.retain_selected_tile(tile.pin(), selected, &mut account)?;
    drop(tile);
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        Residence::Pending { .. }
    ));
    assert!(matches!(
        heap.retire_mask(selected, &mut account.ledger),
        Err(MaskError::Heap(HeapError::PinnedUses))
    ));
    assert!(matches!(
        heap.retire_cell_layout(selected.layout(), &mut account.ledger),
        Err(MaskError::Heap(HeapError::PinnedUses))
    ));
    assert!(matches!(
        heap.replace_cell_layout(
            selected.layout(),
            Layout::contiguous(Extent::new(4, 2)?)?,
            &mut account.ledger
        ),
        Err(MaskError::Heap(HeapError::PinnedUses))
    ));
    // The immediate parent mask is historical metadata, not a required live allocation.
    heap.retire_mask(compact, &mut account.ledger)?;
    let reader = owned.reader(&mut account)?;
    check_companions(&reader, &mut account)?;
    drop(owned);
    heap.retire_mask(selected, &mut account.ledger)?;
    heap.retire_cell_layout(selected.layout(), &mut account.ledger)?;
    empty(&mut heap, &mut account)
}

/// Selected original tags and companion channels are independent of the retained map.
fn check_companions<P: Scalar + 'static>(
    reader: &crate::MaskedTileReader<'_, P>,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    assert_eq!(reader.capacity(), 4);
    for (cell, tag) in [(0, 1), (2, 64)] {
        for (channel, values) in [
            (HostChannel::Center, [P::from_u8(tag), P::ONE]),
            (HostChannel::Total, [P::from_u8(2), P::from_u8(3)]),
            (HostChannel::Error, [P::ONE, P::from_u8(2)]),
        ] {
            for (component, expected) in values.into_iter().enumerate() {
                assert_eq!(
                    reader.read_component(channel, cell, component, account)?,
                    expected
                );
            }
        }
    }
    super::super::fault::reset_reads();
    for cell in [1, 3] {
        assert!(matches!(
            reader.read_component(HostChannel::Center, cell, 0, account),
            Err(LoanError::CoverageMismatch)
        ));
    }
    assert_eq!(super::super::fault::reads(), 0);
    Ok(())
}

/// Empty selections and masks selecting source holes create no numerical observations.
/// ´test:unit:loan-owned-selection-empty-and-holes´
#[test]
fn loan_owned_selection_empty_and_holes() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    for cells in [&[][..], &[2][..], &[1, 7, 64][..]] {
        let selected = heap.create_mask(mask.layout(), cells, &mut account.ledger)?;
        let owned = heap.retain_selected_tile(tile.pin(), selected, &mut account)?;
        let reader = owned.reader(&mut account)?;
        super::super::fault::reset_reads();
        assert!(matches!(
            reader.read_component(HostChannel::Center, 2, 0, &mut account),
            Err(LoanError::CoverageMismatch)
        ));
        assert_eq!(super::super::fault::reads(), 0);
        drop(owned);
        heap.retire_mask(selected, &mut account.ledger)?;
    }
    drop(tile);
    empty(&mut heap, &mut account)
}

/// A foreign account or equal-sized unrelated axis cannot acquire a selected source.
/// ´test:unit:loan-owned-selection-authority-refusals´
#[test]
fn loan_owned_selection_authority_refusals() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let mut foreign = crate::ExecutionAccount::new(Budget::new(Limits::default())?)?;
    assert!(matches!(
        heap.retain_selected_tile(tile.pin(), mask, &mut foreign),
        Err(LoanError::ForeignAccount)
    ));
    let axis = heap.create_cell_axis(65, &mut account.ledger)?;
    let layout = heap.create_cell_layout(
        axis,
        Layout::contiguous(Extent::new(65, 2)?)?,
        &mut account.ledger,
    )?;
    let unrelated = heap.create_mask(layout, &[1], &mut account.ledger)?;
    assert!(matches!(
        heap.retain_selected_tile(tile.pin(), unrelated, &mut account),
        Err(LoanError::LayoutMismatch)
    ));
    heap.retire_mask(unrelated, &mut account.ledger)?;
    assert!(
        heap.retain_selected_tile(tile.pin(), unrelated, &mut account)
            .is_err()
    );
    heap.retire_cell_layout(layout, &mut account.ledger)?;
    drop(tile);
    empty(&mut heap, &mut account)
}

/// Three host-reader units, two binding units and six retention units precede pin admission.
/// Ten remaining units cannot acquire a shared native edge; the limit is never increased.
/// ´test:unit:loan-owned-selection-retention-work-refusal´
#[test]
fn loan_owned_selection_retention_work_refusal() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let selected = heap.create_mask(mask.layout(), &[1], &mut account.ledger)?;
    let remaining = account
        .limits()
        .work
        .checked_sub(account.usage().work)
        .and_then(|work| work.checked_sub(10))
        .ok_or("remaining Work")?;
    account.ledger.work(remaining, super::super::AT)?;
    super::super::fault::reset_reads();
    assert!(matches!(
        heap.retain_selected_tile(tile.pin(), selected, &mut account),
        Err(LoanError::Budget(_))
    ));
    assert_eq!(super::super::fault::reads(), 0);
    // Inspect the unchanged root without replenishing the exhausted operation ledger.
    heap.masks.masks.get(selected.key)?.unprotected()?;
    Ok(())
}

/// Fallible numerical-pin admission releases already acquired native selection edges.
/// ´test:unit:loan-owned-selection-pin-allocation-refusal´
#[test]
fn loan_owned_selection_pin_allocation_refusal() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let identity = heap.mask_correspondence(mask.layout(), mask.layout(), &mut account.ledger)?;
    let compact = heap.compact_mask(identity, mask, 4, 3, &mut account.ledger)?;
    let injection = super::super::fault::Injection::after(0);
    let refused = heap.retain_selected_tile(tile.pin(), compact, &mut account);
    drop(injection);
    assert!(matches!(refused, Err(LoanError::AllocationRefused)));
    heap.retire_mask(compact, &mut account.ledger)?;
    heap.retire_cell_layout(compact.layout(), &mut account.ledger)?;
    assert_eq!(
        tile.reader(&heap, &mut account)?.read_component(
            HostChannel::Center,
            64,
            0,
            &mut account
        )?,
        64.0
    );
    drop(tile);
    empty(&mut heap, &mut account)
}

/// Zero-capacity and fully initialized selections keep exact cell coverage in both formats.
/// ´test:unit:loan-owned-selection-empty-and-full-capacity´
#[test]
fn loan_owned_selection_empty_and_full_capacity() -> TestResult {
    full::<f32>()?;
    full::<f64>()
}

/// The host's explicit ascending tags supply an independent dense value oracle.
fn full<P: Scalar + 'static>() -> TestResult {
    for cells in [0, 1, 65] {
        let (mut heap, mut account, declaration) =
            fixture::<P>(cells, 1, [true, true], Limits::default())?;
        let mut loan = super::granted(heap.request_loan(declaration, &mut account))?;
        for cell in 0..cells {
            loan.write_cell(
                HostChannel::Center,
                cell,
                &[P::from_u8(u8::try_from(cell)?)],
                &mut account,
            )?;
            loan.write_cell(HostChannel::Total, cell, &[P::ONE], &mut account)?;
            loan.write_cell(HostChannel::Error, cell, &[P::ZERO], &mut account)?;
        }
        let (tile, mask) = match heap.handback(loan, &mut account) {
            crate::HandbackOutcome::Published(tile, receipt) => (
                tile,
                receipt
                    .filled(HostChannel::Center)
                    .ok_or("filled center")?
                    .native(),
            ),
            other => return Err(format!("{other:?}").into()),
        };
        let owned = heap.retain_selected_tile(tile.pin(), mask, &mut account)?;
        drop(tile);
        let reader = owned.reader(&mut account)?;
        assert_eq!(reader.capacity(), cells);
        for cell in 0..cells {
            assert_eq!(
                reader.read_component(HostChannel::Center, cell, 0, &mut account)?,
                P::from_u8(u8::try_from(cell)?)
            );
        }
        assert!(
            reader
                .read_component(HostChannel::Center, cells, 0, &mut account)
                .is_err()
        );
        drop(owned);
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}

/// Nonempty gathered values and companions outlive both the original publication and heap.
/// ´test:unit:loan-owned-selection-detached-heap-lifetime´
#[test]
fn loan_owned_selection_detached_heap_lifetime() -> TestResult {
    detached::<f32>()?;
    detached::<f64>()
}

/// No registry lookup or borrowed heap is available during these original-value reads.
fn detached<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<P>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let identity = heap.mask_correspondence(mask.layout(), mask.layout(), &mut account.ledger)?;
    let compact = heap.compact_mask(identity, mask, 3, 3, &mut account.ledger)?;
    let owned = heap.retain_selected_tile(tile.pin(), compact, &mut account)?;
    drop(tile);
    drop(heap);
    let reader = owned.reader(&mut account)?;
    assert_eq!(
        reader.read_component(HostChannel::Center, 2, 0, &mut account)?,
        P::from_u8(64)
    );
    assert_eq!(
        reader.read_component(HostChannel::Total, 2, 1, &mut account)?,
        P::from_u8(3)
    );
    assert_eq!(
        reader.read_component(HostChannel::Error, 2, 0, &mut account)?,
        P::ONE
    );
    Ok(())
}
