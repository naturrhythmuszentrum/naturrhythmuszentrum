// SPDX-License-Identifier: AGPL-3.0-only
//! Mapped native coverage coexists with the immutable numerical bundle's reader lifetime.

use super::{TestResult, empty, fixture, granted};
use crate::{Extent, HandbackOutcome, HostChannel, Layout, LoanError, Residence};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// Coverage transport neither lends numerical storage nor releases protected companions.
/// ´test:unit:loan-component-transport-protected-readers´
#[test]
fn loan_component_transport_protected_readers() -> TestResult {
    protected::<f32>()?;
    protected::<f64>()
}

/// Independently chosen logical cells cross a mask word boundary in both scalar formats.
fn protected<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<P>(65, 2, [true, true], Limits::default())?;
    let mut loan = granted(heap.request_loan(declaration, &mut account))?;
    for cell in [1, 64] {
        loan.write_cell(
            HostChannel::Center,
            cell,
            &[P::ONE, P::from_u8(2)],
            &mut account,
        )?;
        loan.write_cell(HostChannel::Total, cell, &[P::ONE, P::ONE], &mut account)?;
        loan.write_cell(HostChannel::Error, cell, &[P::ZERO, P::ZERO], &mut account)?;
    }
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let mask = receipt
        .filled(HostChannel::Center)
        .ok_or("filled center")?
        .native();
    let axis = heap.mask(mask, mask.layout())?.axis();
    let layout = heap.create_cell_layout(
        axis,
        Layout::strided(Extent::new(65, 2)?, 130, 0, 2, 1)?,
        &mut account.ledger,
    )?;
    let moved = heap.transport_mask(mask, layout, &mut account.ledger)?;
    let pin = heap.acquire_pin::<P>(receipt.ticket(), &mut account)?;
    drop(tile);
    assert!(matches!(
        heap.residence(&mut account, 100_000)?,
        Residence::Pending { .. }
    ));
    let reader = pin.historical_reader(&mut account)?;
    check_reader(&reader, &heap.mask(moved, layout)?, &mut account)?;
    heap.retire_mask(moved, &mut account.ledger)?;
    heap.retire_cell_layout(layout, &mut account.ledger)?;
    drop(pin);
    empty(&mut heap, &mut account)
}

/// Each selected component is checked against its independently written channel value.
fn check_reader<P: Scalar + 'static>(
    reader: &crate::TileReader<'_, P>,
    mask: &crate::MaskView<'_>,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    for cell in 0..65 {
        let selected = cell == 1 || cell == 64;
        assert_eq!(mask.contains(cell)?, selected);
        assert_eq!(reader.contains(cell, account)?, selected);
        if selected {
            assert_eq!(
                reader.read_component(HostChannel::Center, cell, 1, account)?,
                P::from_u8(2)
            );
            assert_eq!(
                reader.read_component(HostChannel::Total, cell, 1, account)?,
                P::ONE
            );
            assert_eq!(
                reader.read_component(HostChannel::Error, cell, 1, account)?,
                P::ZERO
            );
        } else {
            assert!(matches!(
                reader.read_component(HostChannel::Center, cell, 0, account),
                Err(LoanError::CoverageMismatch)
            ));
        }
    }
    Ok(())
}
