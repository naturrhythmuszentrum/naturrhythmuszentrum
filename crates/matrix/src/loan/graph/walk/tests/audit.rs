// SPDX-License-Identifier: AGPL-3.0-only
//! Independent controls for the five completion-audit fixes; invoked only by W1-W4.

use super::{Capacity, Control, Demand, Error, TestResult, Visit, account, drive, edge, graph};
use crate::{Extent, Heap, HeapLimits, Layout, MaskPartition};

/// Three allocated levels exercise actual climb, descent and wrap, with no calculator oracle.
pub(super) fn search() -> TestResult {
    let mut account = account()?;
    let edges = (0..63)
        .map(|slot| edge(slot, slot + 1, slot))
        .collect::<Vec<_>>();
    let mut core = graph(4097, &[8; 65], &edges, &mut account)?;
    let first = core.handle(0, &mut account)?;
    let before = account.usage().work;
    assert_eq!(drive(&mut core, &mut account)?, Visit::Started(first));
    // H entry+cursor2, N phase+application2, M base search1 + complete path3.
    assert_eq!(account.usage().work - before, 8);
    let before = account.usage().work;
    let Visit::Started(second) = drive(&mut core, &mut account)? else {
        return Err("missing second root".into());
    };
    assert_eq!(second.slot, 64);
    // Search base0, summary0, base1: three reads; dispatch's other seven events unchanged.
    assert_eq!(account.usage().work - before, 10);
    core.complete(first, &mut account)?;
    let before = account.usage().work;
    assert_eq!(
        drive(&mut core, &mut account)?,
        Visit::Control(Control::Notified(0))
    );
    // Cursor65: base1 -> summary0 -> top (no successor), wrap base0: M4.
    // H entry+cursor2, N phase+source+target3, E1, both complete paths M6.
    assert_eq!(account.usage().work - before, 16);
    assert_eq!(core.reservation.backing.nodes[1].counts[0], 0);
    Ok(())
}
/// Undispatched, waiting and invalidated occupations cannot publish a fixture completion.
pub(super) fn phases() -> TestResult {
    let mut account = account()?;
    let mut core = graph(2, &[8, 8], &[edge(0, 1, 0)], &mut account)?;
    let first = core.handle(0, &mut account)?;
    let dependent = core.handle(1, &mut account)?;
    for handle in [first, dependent] {
        assert!(matches!(
            core.complete(handle, &mut account),
            Err(Error::InvalidPhase)
        ));
        assert!(matches!(
            core.acknowledge(handle, &mut account),
            Err(Error::InvalidPhase)
        ));
    }
    let wait = Capacity::Wait {
        required: 2,
        available: 1,
    };
    assert_eq!(
        core.drive_one(
            &mut account,
            |_, _| Ok(wait),
            |_, _| Ok(Demand::Satisfied),
            |_| {},
            |_| {},
        )?,
        Visit::Waiting(first, wait)
    );
    assert!(matches!(
        core.complete(first, &mut account),
        Err(Error::InvalidPhase)
    ));
    assert_eq!(core.reservation.backing.nodes[0].counts[1], 1);
    let mut core = graph(1, &[8], &[], &mut account)?;
    let root = core.handle(0, &mut account)?;
    drive(&mut core, &mut account)?;
    core.invalidate(root, &mut account)?;
    assert!(core.resume_invalidation(1, &mut account)?);
    assert!(matches!(
        core.complete(root, &mut account),
        Err(Error::InvalidPhase)
    ));
    Ok(())
}
/// Each terminal edge kind is retained inline, without equating absent input with a value.
pub(super) fn outcomes() -> TestResult {
    for (outcome, missing) in [
        (Demand::Satisfied, false),
        (Demand::Missing, true),
        (Demand::AcceptedAbsence, false),
        (Demand::RequiredAbsence, true),
        (Demand::Inactive, false),
        (Demand::Refused, true),
    ] {
        let mut account = account()?;
        let mut core = graph(2, &[8, 8], &[edge(0, 1, 0)], &mut account)?;
        let root = core.handle(0, &mut account)?;
        drive(&mut core, &mut account)?;
        core.complete(root, &mut account)?;
        assert_eq!(
            core.drive_one(
                &mut account,
                |_, _| Ok(Capacity::Fits),
                |_, _| Ok(outcome),
                |_| {},
                |_| {},
            )?,
            Visit::Control(Control::Notified(0))
        );
        assert_eq!(core.reservation.backing.edges[0].flags >> 3, outcome as u64);
        assert_eq!(
            core.reservation.backing.nodes[1].flags & super::super::control::MISSING != 0,
            missing
        );
        assert_eq!(core.reservation.backing.nodes[0].counts[1], 1);
    }
    Ok(())
}
/// Every wait-scan probe gives the caller its evidence, including intrinsic refusal.
pub(super) fn receipts() -> TestResult {
    let wait = Capacity::Wait {
        required: 2,
        available: 1,
    };
    for capacity in [
        wait,
        Capacity::Fits,
        Capacity::Intrinsic {
            required: 3,
            limit: 2,
        },
    ] {
        let mut account = account()?;
        let mut core = graph(1, &[8], &[], &mut account)?;
        let handle = core.handle(0, &mut account)?;
        core.drive_one(
            &mut account,
            |_, _| Ok(wait),
            |_, _| Ok(Demand::Satisfied),
            |_| {},
            |_| {},
        )?;
        core.begin_capacity_epoch(&mut account)?;
        let mut receipt = None;
        assert!(!core.scan_capacity(
            1,
            &mut account,
            |_, _| Ok(capacity),
            |value| receipt = Some(value)
        )?);
        let receipt = receipt.ok_or("missing capacity receipt")?;
        assert_eq!(receipt.handle, handle);
        assert_eq!(receipt.capacity, Some(capacity));
    }
    Ok(())
}
/// An actual native partition query fails on the original ledger and stops the whole graph.
pub(super) fn nested_work() -> TestResult {
    let mut account = account()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let axis = heap.create_cell_axis(1, &mut account.ledger)?;
    let layout = heap.create_cell_layout(
        axis,
        Layout::contiguous(Extent::new(1, 1)?)?,
        &mut account.ledger,
    )?;
    let correspondence = heap.mask_correspondence(layout, layout, &mut account.ledger)?;
    let mask = heap.create_mask(layout, &[0], &mut account.ledger)?;
    let mut core = graph(2, &[8, 8], &[edge(0, 1, 0)], &mut account)?;
    let root = core.handle(0, &mut account)?;
    drive(&mut core, &mut account)?;
    core.complete(root, &mut account)?;
    // Leave seven units: H entry (one), M wrapped search (two), N phase
    // inspection (one), and the edge plus its two endpoint records (three).
    let remaining = nrz_bindings::Limits::default().work - account.usage().work - 7;
    account.ledger.work(remaining, crate::loan::AT)?;
    let result = core.drive_one(
        &mut account,
        |_, _| Ok(Capacity::Fits),
        |_, budget| {
            heap.check_mask_partition(
                correspondence,
                mask,
                MaskPartition {
                    selected: &[mask],
                    undecided: &[],
                    refusal: &[],
                },
                budget,
            )?;
            Ok(Demand::Satisfied)
        },
        |_| {},
        |_| {},
    );
    assert!(matches!(
        result,
        Err(Error::Native(crate::LoanError::Mask(
            crate::MaskError::Budget(_)
        )))
    ));
    assert_ne!(
        core.reservation.backing.control.flags & super::super::STOPPED,
        0
    );
    assert_eq!(core.reservation.backing.nodes[1].counts[0], 1);
    assert_eq!(core.reservation.backing.nodes[0].counts[1], 1);
    assert_eq!(core.reservation.backing.edges[0].flags & 2, 0);
    let before = account.usage().work;
    let mut released = 0;
    core.abandon(&account, |_| released += 1)
        .map_err(|(error, _)| error)?;
    assert_eq!(released, 1);
    assert_eq!(account.usage().work, before);
    Ok(())
}
