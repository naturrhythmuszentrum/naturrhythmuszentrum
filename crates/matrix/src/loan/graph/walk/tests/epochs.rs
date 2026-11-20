// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded wait scans and caller traversal slices retain the original envelopes.

use super::{
    Admission, Arguments, Capacity, Core, Demand, Error, Kind, Reservation, TestResult, Visit,
    account, drive, graph,
};

/// Reinserted membership lies beyond the captured watermark; cancellation preserves members.
pub(super) fn membership() -> TestResult {
    let mut account = account()?;
    let mut core = graph(3, &[8, 8, 8], &[], &mut account)?;
    let wait = Capacity::Wait {
        required: 2,
        available: 1,
    };
    for slot in 0..3 {
        let Visit::Waiting(handle, _) = core.drive_one(
            &mut account,
            |_, _| Ok(wait),
            |_, _| Ok(Demand::Satisfied),
            |_| {},
            |_| {},
        )?
        else {
            return Err("expected wait member".into());
        };
        assert_eq!(handle.slot, slot);
    }
    core.begin_capacity_epoch(&mut account)?;
    let mut visited = Vec::new();
    assert!(!core.scan_capacity(
        1,
        &mut account,
        |slot, _| {
            visited.push(slot);
            Ok(Capacity::Fits)
        },
        |_| {}
    )?);
    let Visit::Waiting(reinserted, _) = core.drive_one(
        &mut account,
        |_, _| Ok(wait),
        |_, _| Ok(Demand::Satisfied),
        |_| {},
        |_| {},
    )?
    else {
        return Err("expected reinserted member".into());
    };
    assert_eq!(reinserted.slot, 0);
    assert_eq!(core.reservation.backing.nodes[0].epochs[3], 4);
    assert!(core.scan_capacity(
        3,
        &mut account,
        |slot, _| {
            visited.push(slot);
            Ok(wait)
        },
        |_| {}
    )?);
    assert_eq!(visited, [0, 1, 2]);
    let members = core.reservation.backing.control.waiting;
    let attempts = core
        .reservation
        .backing
        .nodes
        .iter()
        .map(|node| node.attempts)
        .collect::<Vec<_>>();
    core.begin_capacity_epoch(&mut account)?;
    core.cancel_capacity_scan(&mut account)?;
    assert_eq!(
        &core.reservation.backing.control.waiting[..2],
        &members[..2]
    );
    assert_eq!(
        core.reservation
            .backing
            .nodes
            .iter()
            .map(|node| node.attempts)
            .collect::<Vec<_>>(),
        attempts
    );
    core.begin_capacity_epoch(&mut account)?;
    visited.clear();
    assert!(core.scan_capacity(
        4,
        &mut account,
        |slot, _| {
            visited.push(slot);
            Ok(wait)
        },
        |_| {}
    )?);
    assert_eq!(visited, [1, 2, 0]);
    Ok(())
}
/// Three ready nodes share two original traversal units across separate calls.
pub(super) fn traversal() -> TestResult {
    let mut account = account()?;
    let reservation = Reservation::reserve::<f64>(
        Arguments {
            nodes: 3,
            edges: 0,
            proposed_edges: 0,
            scopes: 0,
            drive: 2,
            attempts: &[10, 10, 10],
        },
        &[],
        &mut account,
    )?;
    let mut core = match Core::admit(reservation, &[Kind::Fixture; 3], &[], &mut account) {
        Admission::Admitted(core) => core,
        other => return Err(format!("admission: {other:?}").into()),
    };
    assert_eq!(
        core.drive_slice(
            0,
            &mut account,
            |_, _| Ok(Capacity::Fits),
            |_, _| Ok(Demand::Satisfied),
            |_| {},
            |_| {},
        )?,
        Visit::Paused
    );
    for expected in [0, 1] {
        let Visit::Started(handle) = drive(&mut core, &mut account)? else {
            return Err("missing dispatch".into());
        };
        assert_eq!(handle.slot, expected);
    }
    for _ in 0..2 {
        assert!(matches!(
            drive(&mut core, &mut account),
            Err(Error::TraversalExhausted)
        ));
    }
    assert_eq!(core.reservation.backing.nodes[2].attempts, 10);
    let before = account.usage().work;
    core.abandon(&account, |_| {}).map_err(|(error, _)| error)?;
    assert_eq!(account.usage().work, before);
    let mut core = graph(1, &[2], &[], &mut account)?;
    let handle = core.handle(0, &mut account)?;
    let intrinsic = Capacity::Intrinsic {
        required: 3,
        limit: 2,
    };
    assert_eq!(
        core.drive_one(
            &mut account,
            |_, _| Ok(intrinsic),
            |_, _| Ok(Demand::Satisfied),
            |_| {},
            |_| {},
        )?,
        Visit::Refused(handle, intrinsic)
    );
    assert_eq!(
        core.reservation.backing.nodes[0].flags & super::super::WAITING,
        0
    );
    Ok(())
}

/// A later dispatch in the same slice cannot erase the earlier exhaustion receipt.
pub(super) fn exhaustion() -> TestResult {
    let mut account = account()?;
    let mut core = graph(2, &[0, 1], &[], &mut account)?;
    let exhausted = core.handle(0, &mut account)?;
    let started = core.handle(1, &mut account)?;
    let mut receipts = [None; 2];
    let mut count = 0;
    let mut probes = 0;
    assert_eq!(
        core.drive_slice(
            2,
            &mut account,
            |slot, _| {
                assert_eq!(slot, 1);
                probes += 1;
                Ok(Capacity::Fits)
            },
            |_, _| Ok(Demand::Satisfied),
            |_| {},
            |receipt| {
                receipts[count] = Some(receipt);
                count += 1;
            },
        )?,
        Visit::Started(started)
    );
    assert_eq!(probes, 1);
    assert_eq!(count, 2);
    assert_eq!(
        receipts,
        [
            Some(super::super::waiting::ScanReceipt {
                handle: exhausted,
                capacity: None,
            }),
            Some(super::super::waiting::ScanReceipt {
                handle: started,
                capacity: Some(Capacity::Fits),
            }),
        ]
    );
    assert_eq!(core.reservation.backing.nodes[0].attempts, 0);
    assert_ne!(
        core.reservation.backing.nodes[0].flags & super::super::ATTEMPTS,
        0
    );
    Ok(())
}
