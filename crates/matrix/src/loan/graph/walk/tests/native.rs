// SPDX-License-Identifier: AGPL-3.0-only
//! Native coverage and pins retain their existing ledger and ownership routes.

use super::{TestResult, account, drive, edge, graph};
use crate::loan::graph::walk::{
    control::{Control, Demand},
    drive::{Capacity, Visit},
};
use crate::{
    Coverage, Extent, HandbackOutcome, Heap, HeapLimits, HostChannel, HostTileDeclaration, Layout,
    LoanRequest, MaskError, MaskPartition, Residence,
};
use nrz_bindings::{BindingSymbols, Descriptor};
use nrz_scalar::Scalar;

/// Original cell sets determine coverage; every query uses the existing native tariff.
pub(super) fn masks() -> TestResult {
    for (published, expected) in [
        (&[][..], false),
        (&[0, 1][..], false),
        (&[0, 3][..], true),
        (&[0, 1, 2, 3][..], true),
    ] {
        for demand_cells in [&[0, 3][..], &[][..]] {
            let mut account = account()?;
            let mut heap = Heap::new(HeapLimits::default())?;
            let axis = heap.create_cell_axis(4, &mut account.ledger)?;
            let layout = heap.create_cell_layout(
                axis,
                Layout::contiguous(Extent::new(4, 1)?)?,
                &mut account.ledger,
            )?;
            let correspondence = heap.mask_correspondence(layout, layout, &mut account.ledger)?;
            let published = heap.create_mask(layout, published, &mut account.ledger)?;
            let demand = heap.create_mask(layout, demand_cells, &mut account.ledger)?;
            let selected = heap.intersect_mask_demand(
                correspondence,
                &[published, demand],
                &mut account.ledger,
            )?;
            let mut core = graph(2, &[8, 8], &[edge(0, 1, 0)], &mut account)?;
            let producer = core.handle(0, &mut account)?;
            assert_eq!(drive(&mut core, &mut account)?, Visit::Started(producer));
            core.complete(producer, &mut account)?;
            let before = account.usage().work;
            let mut mask_work = 0;
            let visit = core.drive_one(
                &mut account,
                |_, _| Ok(Capacity::Fits),
                |_, budget| {
                    let before = budget.usage().work;
                    let result = heap.check_mask_partition(
                        correspondence,
                        demand,
                        MaskPartition {
                            selected: &[selected],
                            undecided: &[],
                            refusal: &[],
                        },
                        budget,
                    );
                    mask_work += budget.usage().work - before;
                    match result {
                        Ok(()) => Ok(Demand::Satisfied),
                        Err(MaskError::PartitionCoverage) => Ok(Demand::Missing),
                        Err(error) => Err(error.into()),
                    }
                },
                |_| {},
                |_| {},
            )?;
            assert_eq!(visit, Visit::Control(Control::Notified(0)));
            // Existing tariff: entry, one version, one demand word, one selected word.
            assert_eq!(mask_work, 4);
            // Graph H2 + N3 + E1 + M4 (two search reads, two paths); native4.
            assert_eq!(account.usage().work - before, 14);
            let satisfied = expected || demand_cells.is_empty();
            assert_eq!(
                core.reservation.backing.nodes[1].flags & super::super::control::MISSING == 0,
                satisfied
            );
            assert_eq!(core.reservation.backing.nodes[0].counts[1], 1);
        }
    }
    Ok(())
}
/// Dropping exactly one graph-held original pin leaves the outside pin readable.
pub(super) fn pins<P: Scalar + 'static>(stop_before_release: bool) -> TestResult {
    let mut account = account()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let tile = published::<P>(&mut heap, &mut account)?;
    let outside = heap.retain_pin(tile.pin(), &mut account)?;
    let mut owned = Some(tile.into_pin());
    let consumer_attempts = if stop_before_release { 0 } else { 8 };
    let mut core = graph(2, &[8, consumer_attempts], &[edge(0, 1, 0)], &mut account)?;
    let producer = core.handle(0, &mut account)?;
    drive(&mut core, &mut account)?;
    core.complete(producer, &mut account)?;
    drive(&mut core, &mut account)?;
    let consumer = match drive(&mut core, &mut account)? {
        Visit::Started(handle) if !stop_before_release => {
            core.complete(handle, &mut account)?;
            handle
        }
        Visit::AttemptsExhausted(handle) if stop_before_release => handle,
        other => return Err(format!("unexpected consumer transition: {other:?}").into()),
    };
    assert!(owned.is_some());
    let mut releases = 0;
    assert_eq!(
        core.drive_slice(
            0,
            &mut account,
            |_, _| Ok(Capacity::Fits),
            |_, _| Ok(Demand::Satisfied),
            |_| {
                releases += 1;
                drop(owned.take());
            },
            |_| {},
        )?,
        Visit::Paused
    );
    assert_eq!(releases, 0);
    assert!(owned.is_some());
    if stop_before_release {
        // Unrelated prior work on the same immutable ledger spends its remaining allowance.
        let remaining = nrz_bindings::Limits::default().work - account.usage().work;
        account.ledger.work(remaining, crate::loan::AT)?;
        assert!(matches!(
            drive(&mut core, &mut account),
            Err(super::Error::Native(crate::LoanError::Budget(_)))
        ));
        assert!(owned.is_some());
        assert_eq!(core.reservation.backing.nodes[0].counts[1], 1);
        let before = account.usage().work;
        core.abandon(&account, |_| {
            releases += 1;
            drop(owned.take());
        })
        .map_err(|(error, _)| error)?;
        assert_eq!(account.usage().work, before);
        assert_eq!(releases, 1);
        assert!(owned.is_none());
        // The outside pin remains owned here; abandonment never receives it.
        drop(outside);
        return Ok(());
    }
    assert_eq!(
        core.drive_one(
            &mut account,
            |_, _| Ok(Capacity::Fits),
            |_, _| Ok(Demand::Satisfied),
            |_| {
                releases += 1;
                drop(owned.take());
            },
            |_| {},
        )?,
        Visit::Control(Control::Released(0))
    );
    assert_eq!(releases, 1);
    assert!(owned.is_none());
    assert_eq!(drive(&mut core, &mut account)?, Visit::Quiescent);
    let reader = outside.historical_reader(&mut account)?;
    assert_eq!(
        reader.read_component(HostChannel::Center, 0, 0, &mut account)?,
        P::ONE
    );
    core.acknowledge(producer, &mut account)?;
    core.acknowledge(consumer, &mut account)?;
    drive(&mut core, &mut account)?;
    drive(&mut core, &mut account)?;
    assert!(matches!(
        heap.residence(&mut account, 1_000)?,
        Residence::Pending { .. }
    ));
    drop(outside);
    assert!(matches!(
        heap.residence(&mut account, 1_000)?,
        Residence::Empty { .. }
    ));
    Ok(())
}

/// Construct the original native publication independently of graph control transitions.
fn published<P: Scalar + 'static>(
    heap: &mut Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult<crate::PublishedTile<P>> {
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [1, 1],
        [false, false],
        &mut account.ledger,
    )?;
    let axis = heap.create_cell_axis(1, &mut account.ledger)?;
    let declaration =
        HostTileDeclaration::<P>::check(symbols, axis, Coverage::RepresentedInputs, account)?;
    let mut loan = match heap.request_loan(declaration, account) {
        LoanRequest::Granted(loan) => loan,
        LoanRequest::Refused(error) => return Err(format!("loan fixture: {error:?}").into()),
    };
    loan.write_cell(HostChannel::Center, 0, &[P::ONE], account)?;
    let tile = match heap.handback(loan, account) {
        HandbackOutcome::Published(tile, _) => tile,
        other => return Err(format!("handback fixture: {other:?}").into()),
    };
    Ok(tile)
}
