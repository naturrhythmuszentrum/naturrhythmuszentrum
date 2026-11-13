// SPDX-License-Identifier: AGPL-3.0-only
//! Original-cell values and companion targets survive borrowed branch selections.

use super::{TestResult, empty, fixture, granted};
use crate::{
    ExecutionAccount, HandbackOutcome, Heap, HostChannel, HostTileDeclaration, LoanError,
    MaskVersion, PublishedTile,
};
use nrz_bindings::{Budget, Limits};
use nrz_scalar::Scalar;

/// Publish independently tagged source cells and all required companion channels.
fn publish<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    declaration: HostTileDeclaration<P>,
) -> TestResult<(PublishedTile<P>, MaskVersion)> {
    let mut loan = granted(heap.request_loan(declaration, account))?;
    for cell in [1, 7, 64] {
        let value = P::from_u8(u8::try_from(cell)?);
        loan.write_cell(HostChannel::Center, cell, &[value, P::ONE], account)?;
        loan.write_cell(HostChannel::Total, cell, &[P::ONE, P::ONE], account)?;
        loan.write_cell(HostChannel::Error, cell, &[P::ZERO, P::ZERO], account)?;
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

/// Nested selected reads retain all original companions and the last numerical reader pin.
/// ´test:unit:loan-masked-reader-nested-companions´
#[test]
fn loan_masked_reader_nested_companions() -> TestResult {
    nested::<f32>()?;
    nested::<f64>()
}

/// Explicit original indices supply the expected readings in both formats.
fn nested<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<P>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let pin = heap.retain_pin(tile.pin(), &mut account)?;
    drop(tile);
    let identity = heap.mask_correspondence(mask.layout(), mask.layout(), &mut account.ledger)?;
    let compact = heap.compact_mask(identity, mask, 4, 3, &mut account.ledger)?;
    let selected = heap.create_mask(compact.layout(), &[0, 2], &mut account.ledger)?;
    let identity =
        heap.mask_correspondence(compact.layout(), compact.layout(), &mut account.ledger)?;
    let nested = heap.compact_mask(identity, selected, 3, 2, &mut account.ledger)?;
    assert!(
        heap.retire_cell_layout(mask.layout(), &mut account.ledger)
            .is_err()
    );
    {
        let reader = pin.historical_reader(&mut account)?;
        let view = reader.masked_view(heap.mask(nested, nested.layout())?, &mut account)?;
        assert_eq!(view.capacity(), 3);
        for (cell, original) in [1, 64].into_iter().enumerate() {
            assert_eq!(
                view.read_component(HostChannel::Center, cell, 0, &mut account)?,
                P::from_u8(original)
            );
            assert_eq!(
                view.read_component(HostChannel::Total, cell, 0, &mut account)?,
                P::ONE
            );
            assert_eq!(
                view.read_component(HostChannel::Error, cell, 0, &mut account)?,
                P::ZERO
            );
            assert_eq!(
                view.read_component(HostChannel::Center, cell, 1, &mut account)?,
                P::ONE
            );
        }
        super::super::fault::reset_reads();
        assert!(matches!(
            view.read_component(HostChannel::Center, 2, 0, &mut account),
            Err(LoanError::CoverageMismatch)
        ));
        assert_eq!(super::super::fault::reads(), 0);
    }
    for version in [nested, selected, compact] {
        heap.retire_mask(version, &mut account.ledger)?;
    }
    heap.retire_cell_layout(nested.layout(), &mut account.ledger)?;
    heap.retire_cell_layout(compact.layout(), &mut account.ledger)?;
    drop(pin);
    empty(&mut heap, &mut account)
}

/// Native selection cannot initialize a source hole or borrow another layout's gather origin.
/// ´test:unit:loan-masked-reader-origin-and-hole-refusals´
#[test]
fn loan_masked_reader_origin_and_hole_refusals() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(65, 2, [true, true], Limits::default())?;
    let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
    let identity = heap.mask_correspondence(mask.layout(), mask.layout(), &mut account.ledger)?;
    let holes = heap.create_mask(mask.layout(), &[2], &mut account.ledger)?;
    let compact = heap.compact_mask(identity, holes, 1, 1, &mut account.ledger)?;
    let foreign = other_gather(&mut heap, mask, &mut account)?;
    let reader = tile.reader(&heap, &mut account)?;
    super::super::fault::reset_reads();
    let view = reader.masked_view(heap.mask(compact, compact.layout())?, &mut account)?;
    assert!(matches!(
        view.read_component(HostChannel::Center, 0, 0, &mut account),
        Err(LoanError::CoverageMismatch)
    ));
    assert!(matches!(
        reader.masked_view(heap.mask(foreign, foreign.layout())?, &mut account),
        Err(LoanError::LayoutMismatch)
    ));
    let mut other = ExecutionAccount::new(Budget::new(Limits::default())?)?;
    assert!(matches!(
        reader.masked_view(heap.mask(mask, mask.layout())?, &mut other),
        Err(LoanError::ForeignAccount)
    ));
    assert_eq!(super::super::fault::reads(), 0);
    Ok(())
}

/// The inventory in ``rule:nrz:matrix-loans-borrowed-reader-work`` gives two selection/map
/// units and one numerical load unit, hence the literal three-unit boundary.
/// The existing account is drained to that allowance; its limit is never replaced or raised.
/// ´test:unit:loan-masked-reader-literal-work-boundary´
#[test]
fn loan_masked_reader_literal_work_boundary() -> TestResult {
    for available in [2, 3] {
        let (mut heap, mut account, declaration) =
            fixture::<f64>(65, 2, [true, true], Limits::default())?;
        let (tile, mask) = publish(&mut heap, &mut account, declaration)?;
        let reader = tile.reader(&heap, &mut account)?;
        let view = reader.masked_view(heap.mask(mask, mask.layout())?, &mut account)?;
        let remaining = account
            .limits()
            .work
            .checked_sub(account.usage().work)
            .and_then(|value| value.checked_sub(available))
            .ok_or("remaining Work")?;
        account.ledger.work(remaining, super::super::AT)?;
        super::super::fault::reset_reads();
        let result = view.read_component(HostChannel::Center, 1, 0, &mut account);
        if available == 3 {
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

/// An equal-axis layout has a distinct numerical occupation and cannot replace the source.
fn other_gather(
    heap: &mut Heap,
    mask: MaskVersion,
    account: &mut ExecutionAccount,
) -> TestResult<MaskVersion> {
    let axis = heap.mask(mask, mask.layout())?.axis();
    let foreign_layout = heap.create_cell_layout(
        axis,
        crate::Layout::contiguous(crate::Extent::new(65, 2)?)?,
        &mut account.ledger,
    )?;
    let transported = heap.transport_mask(mask, foreign_layout, &mut account.ledger)?;
    let identity = heap.mask_correspondence(foreign_layout, foreign_layout, &mut account.ledger)?;
    Ok(heap.compact_mask(identity, transported, 3, 3, &mut account.ledger)?)
}
