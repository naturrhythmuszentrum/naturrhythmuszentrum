// SPDX-License-Identifier: AGPL-3.0-only
//! A covered graph reservation neither owns nor releases an outside native reader.

use super::super::{Inventory, Reservation};
use super::{TestResult, args};
use crate::{
    Coverage, ExecutionAccount, HandbackOutcome, Heap, HeapLimits, HostChannel,
    HostTileDeclaration, LoanRequest, Residence,
};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};
use nrz_scalar::Scalar;

/// Existing numerical inventory remains a separate projection with its own retaining pin.
pub(super) fn outside<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [1, 1],
        [false, false],
        &mut account.ledger,
    )?;
    let axis = heap.create_cell_axis(1, &mut account.ledger)?;
    let declaration =
        HostTileDeclaration::<P>::check(symbols, axis, Coverage::RepresentedInputs, &mut account)?;
    let mut loan = match heap.request_loan(declaration, &mut account) {
        LoanRequest::Granted(loan) => loan,
        LoanRequest::Refused(error) => return Err(format!("{error:?}").into()),
    };
    loan.write_cell(HostChannel::Center, 0, &[P::ONE], &mut account)?;
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let pin = heap.retain_pin(tile.pin(), &mut account)?;
    drop(tile);
    let before = heap.loan_inventory(receipt.ticket(), &mut account)?;
    let graph = Reservation::reserve::<P>(args(), &[], &mut account)?;
    let storage = Inventory::inspect(&[graph.borrowed()], &mut account)?;
    assert_eq!(storage.covered_bytes, 832);
    assert_eq!(heap.loan_inventory(receipt.ticket(), &mut account)?, before);
    drop(graph);
    assert!(matches!(
        heap.residence(&mut account, 1_000)?,
        Residence::Pending { .. }
    ));
    let reader = pin.historical_reader(&mut account)?;
    assert_eq!(
        reader.read_component(HostChannel::Center, 0, 0, &mut account)?,
        P::ONE
    );
    drop(pin);
    assert!(matches!(
        heap.residence(&mut account, 1_000)?,
        Residence::Empty { .. }
    ));
    Ok(())
}
