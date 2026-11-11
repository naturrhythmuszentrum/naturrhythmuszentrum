// SPDX-License-Identifier: AGPL-3.0-only
//! Independent square root values, masks, family seals and resource/lifetime boundaries.

mod authority;
mod bytes;
mod correspondence;
mod lifecycle;
mod metadata;
mod observation;
mod oracle;
mod ownership;
mod pairing;
mod prefixes;
mod reader_seals;
mod refusal;
mod resources;
mod scans;
mod seals;
mod spare;
mod unwind;
mod values;
mod work;

use super::{Fixture, TestResult, available, candidate, published, ready, requested};
use crate::{
    Coverage, ExecutionAccount, ExecutionLimits, HandbackOutcome, Heap, HeapLimits, HostChannel,
    HostTileDeclaration, LoanRequest, PointwiseRequest,
};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};
use nrz_scalar::Scalar;

/// Fresh exact-growth source: positive centers, nonnegative boxes, and an included input error.
fn fixture<P: Scalar + 'static>(
    cells: usize,
    components: usize,
    channels: usize,
    selection: usize,
    holes: bool,
    limits: Limits,
) -> TestResult<Fixture<P>> {
    fixture_with_heap(
        cells,
        components,
        channels,
        selection,
        holes,
        limits,
        HeapLimits::default(),
    )
}
/// All resource ceilings are fixed before the host-source setup begins.
fn fixture_with_heap<P: Scalar + 'static>(
    cells: usize,
    components: usize,
    channels: usize,
    selection: usize,
    holes: bool,
    limits: Limits,
    heap_limits: HeapLimits,
) -> TestResult<Fixture<P>> {
    let coverage = Coverage::RepresentedInputs;
    let mut heap = Heap::new(heap_limits)?;
    let mut symbolic = Budget::new(Limits::default())?;
    let shape = [u32::try_from(cells)?, u32::try_from(components)?];
    let input = BindingSymbols::declare(
        &Descriptor::ratio(),
        shape,
        [channels > 1, channels > 2],
        &mut symbolic,
    )?;
    let output = BindingSymbols::declare(&Descriptor::ratio(), shape, [true, true], &mut symbolic)?;
    let axis = heap.create_cell_axis(cells, &mut symbolic)?;
    let mut budget = Budget::new(limits)?;
    budget.work(3, crate::loan::AT)?;
    let mut account = ExecutionAccount::new(budget)?;
    let input = HostTileDeclaration::<P>::check(input, axis, coverage, &mut account)?;
    let destination = HostTileDeclaration::<P>::check(output, axis, coverage, &mut account)?;
    let mut loan = match heap.request_loan(input, &mut account) {
        LoanRequest::Granted(loan) => loan,
        other @ LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for cell in 0..cells {
        for component in 0..components {
            let center = if cell % 2 == 0 {
                P::from_u8(4)
            } else {
                P::from_u8(9)
            };
            let partial = holes && components > 1 && cell % 13 == 4 && component == components - 1;
            if !partial {
                loan.write_component(HostChannel::Center, cell, component, center, &mut account)?;
            }
            if channels > 1 && !(holes && cell % 5 == 1) {
                let total = P::ONE / P::from_u8(4);
                loan.write_component(HostChannel::Total, cell, component, total, &mut account)?;
            }
            if channels > 2 && !(holes && cell % 7 == 2) {
                loan.write_component(
                    HostChannel::Error,
                    cell,
                    component,
                    P::ONE / P::from_u8(8),
                    &mut account,
                )?;
            }
        }
        if holes && cell % 11 == 3 {
            loan.write_component(
                HostChannel::Center,
                cell,
                0,
                P::parse_nearest("NaN").ok_or("native NaN")?,
                &mut account,
            )?;
            loan.clear_cell(HostChannel::Center, cell, &mut account)?;
        }
    }
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let layout = receipt
        .filled(HostChannel::Center)
        .ok_or("filled")?
        .native()
        .layout();
    let demand = (0..cells)
        .filter(|cell| requested(*cell, selection))
        .collect::<Vec<_>>();
    let demand = heap.create_mask(layout, &demand, &mut account.ledger)?;
    let declarations = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(components)?;
    let codomain = crate::kernel_testing::axis(cells)?;
    let input = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
    let output = crate::kernel_testing::shape(&declarations, &domain, &codomain)?;
    let operation =
        crate::OperationDraft::map_elementwise(input, output, crate::Elementwise::SquareRoot)?
            .check(ExecutionLimits::default())?;
    Ok(Fixture {
        heap,
        account,
        request: PointwiseRequest {
            operation,
            input: tile.into_pin(),
            destination,
            demand,
            limits: ExecutionLimits::default(),
        },
    })
}
