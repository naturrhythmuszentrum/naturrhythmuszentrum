// SPDX-License-Identifier: AGPL-3.0-only
//! Four focused controls; literal expectations are independent of production cost helpers.

mod audit;
mod boundaries;
mod epochs;
mod native;

use super::{
    Admission, Connection, Core, Error, Kind, NONE, READY,
    control::{Control, Demand},
    drive::{Capacity, Visit},
};
use crate::{
    ExecutionAccount,
    loan::graph::{Arguments, Reservation},
};
use nrz_bindings::{Budget, Limits};
use std::sync::atomic::AtomicU64;

/// Preserve typed errors and enough context to diagnose a failed fixture admission.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// A nonzero already-used original ledger is never replaced during a control.
fn account() -> TestResult<ExecutionAccount> {
    let mut budget = Budget::new(Limits::default())?;
    budget.work(5, crate::loan::AT)?;
    Ok(ExecutionAccount::new(budget)?)
}
/// Original caller counts are not derived from Work or bitmap size.
fn reserve(
    capacity: usize,
    attempts: &[u64],
    edges: usize,
    account: &mut ExecutionAccount,
) -> TestResult<Reservation> {
    Ok(Reservation::reserve::<f64>(
        Arguments {
            nodes: capacity,
            edges,
            proposed_edges: edges,
            scopes: 1,
            drive: 10_000,
            attempts,
        },
        &[],
        account,
    )?)
}
/// Core fixture contracts are explicit; unsupported contracts are separately refused.
fn graph(
    capacity: usize,
    attempts: &[u64],
    edges: &[Connection],
    account: &mut ExecutionAccount,
) -> TestResult<Core> {
    let reservation = reserve(capacity, attempts, edges.len(), account)?;
    let kinds = vec![Kind::Fixture; attempts.len()];
    match Core::admit(reservation, &kinds, edges, account) {
        Admission::Admitted(core) => Ok(core),
        other => Err(format!("fixture admission: {other:?}").into()),
    }
}
/// No numerical operation or quantified decision is supplied by these control callbacks.
fn drive(core: &mut Core, account: &mut ExecutionAccount) -> Result<Visit, Error> {
    core.drive_one(
        account,
        |_, _| Ok(Capacity::Fits),
        |_, _| Ok(Demand::Satisfied),
        |_| {},
        |_| {},
    )
}
/// A concrete edge retains its original demand index even after incidence unlinking.
fn edge(producer: usize, consumer: usize, demand: usize) -> Connection {
    Connection {
        producer,
        consumer,
        demand,
        semantic: false,
    }
}

/// Empty graphs, stable order, readers, union cycles and actual admission identity boundaries.
/// ´test:unit:ready-walk-order-and-generations´
#[test]
fn ready_walk_order_and_generations() -> TestResult {
    audit::search()?;
    audit::phases()?;
    boundaries::counters()?;
    boundaries::reuse()?;
    let mut account = account()?;
    let mut empty = graph(0, &[], &[], &mut account)?;
    let before = account.usage().work;
    assert_eq!(drive(&mut empty, &mut account)?, Visit::Quiescent);
    assert_eq!(account.usage().work - before, 1); // H entry, no index words.
    let reservation = reserve(3, &[4, 4, 4], 0, &mut account)?;
    let reader = reservation.reader(&mut account)?;
    let reservation = match Core::admit(reservation, &[Kind::Fixture; 3], &[], &mut account) {
        Admission::OwnershipWait(reservation) => reservation,
        other => return Err(format!("reader did not block: {other:?}").into()),
    };
    drop(reader);
    let mut core = match Core::admit(reservation, &[Kind::Fixture; 3], &[], &mut account) {
        Admission::Admitted(core) => core,
        other => return Err(format!("exclusive admission: {other:?}").into()),
    };
    for slot in 0..3 {
        let Visit::Started(handle) = drive(&mut core, &mut account)? else {
            return Err("expected fixture dispatch".into());
        };
        assert_eq!(handle.slot, slot);
    }
    assert_eq!(drive(&mut core, &mut account)?, Visit::Quiescent);
    let mut cycle = [edge(0, 1, 0), edge(1, 0, 1)];
    cycle[1].semantic = true;
    let reservation = reserve(2, &[4, 4], 2, &mut account)?;
    match Core::admit(reservation, &[Kind::Fixture; 2], &cycle, &mut account) {
        Admission::Refused(Error::Cycle, reservation) => {
            assert_eq!(reservation.backing.nodes.len(), 2);
            assert_eq!(reservation.backing.ready, [0]);
        }
        other => {
            return Err(format!("cycle did not preserve private reservation: {other:?}").into());
        }
    }
    for (seed, success) in [(u64::MAX - 1, true), (u64::MAX, false)] {
        let reservation = reserve(1, &[4], 0, &mut account)?;
        let source = AtomicU64::new(seed);
        let result = Core::admit_with(reservation, &[Kind::Fixture], &[], &mut account, &source);
        assert_eq!(matches!(result, Admission::Admitted(_)), success);
        assert_eq!(source.load(std::sync::atomic::Ordering::Relaxed), u64::MAX);
    }
    let reservation = reserve(1, &[4], 0, &mut account)?;
    assert!(matches!(
        Core::admit(reservation, &[Kind::Unsupported], &[], &mut account),
        Admission::Refused(Error::Native(crate::LoanError::UnsupportedContract), _)
    ));
    Ok(())
}

/// Settlement releases readiness only; the separately scheduled use release retires incidence.
/// ´test:unit:ready-walk-demand-and-release´
#[test]
fn ready_walk_demand_and_release() -> TestResult {
    audit::outcomes()?;
    demand_join()?;
    native::masks()?;
    native::pins::<f32>(false)?;
    native::pins::<f64>(false)?;
    let mut account = account()?;
    let mut core = graph(3, &[8, 8, 8], &[edge(0, 1, 0), edge(0, 2, 1)], &mut account)?;
    let producer = core.handle(0, &mut account)?;
    assert_eq!(drive(&mut core, &mut account)?, Visit::Started(producer));
    core.complete(producer, &mut account)?;
    assert_eq!(
        drive(&mut core, &mut account)?,
        Visit::Control(Control::Notified(0))
    );
    assert_eq!(core.reservation.backing.nodes[0].counts[1], 2);
    assert_eq!(core.reservation.backing.nodes[1].counts[0], 0);
    let Visit::Started(first) = drive(&mut core, &mut account)? else {
        return Err("first consumer not ready".into());
    };
    assert_eq!(first.slot, 1);
    assert_eq!(
        drive(&mut core, &mut account)?,
        Visit::Control(Control::Notified(1))
    );
    let Visit::Started(second) = drive(&mut core, &mut account)? else {
        return Err("second consumer not ready".into());
    };
    assert_eq!(second.slot, 2);
    core.complete(first, &mut account)?;
    core.complete(second, &mut account)?;
    let mut released = Vec::new();
    for expected in [0, 1] {
        let visit = core.drive_one(
            &mut account,
            |_, _| Ok(Capacity::Fits),
            |_, _| Ok(Demand::Satisfied),
            |demand| released.push(demand),
            |_| {},
        )?;
        assert_eq!(visit, Visit::Control(Control::Released(expected)));
    }
    assert_eq!(released, [0, 1]);
    assert_eq!(core.reservation.backing.nodes[0].counts[1], 0);
    assert_eq!(drive(&mut core, &mut account)?, Visit::Quiescent); // Evidence still retained.
    for handle in [producer, first, second] {
        core.acknowledge(handle, &mut account)?;
    }
    for slot in 0..3 {
        assert_eq!(
            drive(&mut core, &mut account)?,
            Visit::Control(Control::Retired(slot))
        );
    }
    assert_eq!(drive(&mut core, &mut account)?, Visit::Quiescent);
    assert!(matches!(
        core.complete(producer, &mut account),
        Err(Error::Native(crate::LoanError::StaleOccupation))
    ));
    assert_eq!(core.reservation.backing.nodes[0].generation, 2);
    Ok(())
}

/// The only ready leaf lies in a three-level allocated bitmap; visits use the affected closure.
/// ´test:unit:ready-walk-invalidation-cost´
#[test]
fn ready_walk_invalidation_cost() -> TestResult {
    boundaries::invalidation()?;
    boundaries::unaffected()?;
    let mut account = account()?;
    let mut core = graph(
        4097,
        &[8, 8, 8],
        &[edge(0, 1, 0), edge(1, 2, 1)],
        &mut account,
    )?;
    assert_eq!(core.reservation.backing.ready.len(), 68); // 65 base + 2 summaries + top.
    let root = core.handle(0, &mut account)?;
    let before = account.usage().work;
    core.invalidate(root, &mut account)?;
    // H entry+epoch=2, A=1, N root=1, M mark+three path words=4, X=1.
    assert_eq!(account.usage().work - before, 9);
    let generation = core.reservation.backing.control.epochs[1];
    assert!(matches!(
        core.invalidate(root, &mut account),
        Err(Error::ControlInProgress)
    ));
    assert_eq!(core.reservation.backing.control.epochs[1], generation);
    assert_eq!(drive(&mut core, &mut account)?, Visit::ControlPending);
    let before = account.usage().work;
    assert!(!core.resume_invalidation(1, &mut account)?);
    // H entry+cursor=2, E=1, N source+target=2, M mark=1, X push=1.
    assert_eq!(account.usage().work - before, 7);
    assert_eq!(drive(&mut core, &mut account)?, Visit::ControlPending);
    assert!(!core.resume_invalidation(1, &mut account)?);
    assert!(core.resume_invalidation(3, &mut account)?);
    assert_eq!(drive(&mut core, &mut account)?, Visit::Quiescent);
    for node in &core.reservation.backing.nodes[..3] {
        assert_ne!(node.flags & super::INVALID, 0);
        assert_eq!(node.flags & READY, 0);
        assert_eq!(node.attempts, 8);
    }
    assert_eq!(core.reservation.backing.nodes[0].counts[1], 1);
    assert_eq!(core.reservation.backing.nodes[1].counts[1], 1);
    Ok(())
}

/// Capacity scans retain cumulative attempts and stop at the captured membership watermark.
/// ´test:unit:ready-walk-capacity-epoch´
#[test]
fn ready_walk_capacity_epoch() -> TestResult {
    boundaries::drive_receipt()?;
    audit::receipts()?;
    audit::nested_work()?;
    epochs::membership()?;
    epochs::traversal()?;
    epochs::exhaustion()?;
    native::pins::<f32>(true)?;
    native::pins::<f64>(true)?;
    let mut account = account()?;
    let mut core = graph(2, &[2, 2], &[], &mut account)?;
    let first = core.handle(0, &mut account)?;
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
    let Visit::Started(other) = drive(&mut core, &mut account)? else {
        return Err("unrelated leaf did not progress".into());
    };
    assert_eq!(other.slot, 1);
    core.begin_capacity_epoch(&mut account)?;
    assert!(matches!(
        core.begin_capacity_epoch(&mut account),
        Err(Error::ControlInProgress)
    ));
    let mut probes = 0;
    assert!(!core.scan_capacity(
        1,
        &mut account,
        |_, _| {
            probes += 1;
            Ok(wait)
        },
        |_| {}
    )?);
    assert!(core.scan_capacity(
        1,
        &mut account,
        |_, _| {
            probes += 1;
            Ok(wait)
        },
        |_| {}
    )?);
    assert_eq!(probes, 1);
    assert_eq!(core.reservation.backing.nodes[0].attempts, 0);
    core.begin_capacity_epoch(&mut account)?;
    assert!(core.scan_capacity(
        2,
        &mut account,
        |_, _| {
            probes += 1;
            Ok(wait)
        },
        |_| {}
    )?);
    assert_eq!(probes, 1);
    assert_eq!(core.reservation.backing.nodes[0].flags & super::WAITING, 0);
    assert_ne!(core.reservation.backing.nodes[0].flags & super::ATTEMPTS, 0);
    assert_eq!(core.reservation.backing.control.waiting, [NONE; 3]);
    Ok(())
}

/// Both original producer contributions settle; absence/inactive handling is an explicit predicate.
fn demand_join() -> TestResult {
    for outcomes in [
        [Demand::Satisfied, Demand::Satisfied],
        [Demand::Missing, Demand::Satisfied],
    ] {
        let mut account = account()?;
        let mut core = graph(3, &[8, 8, 8], &[edge(0, 2, 0), edge(1, 2, 1)], &mut account)?;
        let first = core.handle(0, &mut account)?;
        let second = core.handle(1, &mut account)?;
        assert_eq!(drive(&mut core, &mut account)?, Visit::Started(first));
        assert_eq!(drive(&mut core, &mut account)?, Visit::Started(second));
        core.complete(first, &mut account)?;
        core.complete(second, &mut account)?;
        for (index, producer) in [first, second].into_iter().enumerate() {
            let unsettled = core.reservation.backing.nodes[2].counts[0];
            assert_eq!(
                core.drive_slice(
                    0,
                    &mut account,
                    |_, _| Ok(Capacity::Fits),
                    |demand, _| Ok(outcomes[demand]),
                    |_| {},
                    |_| {},
                )?,
                Visit::Paused
            );
            assert_eq!(core.reservation.backing.nodes[2].counts[0], unsettled);
            assert_eq!(
                core.drive_one(
                    &mut account,
                    |_, _| Ok(Capacity::Fits),
                    |demand, _| Ok(outcomes[demand]),
                    |_| {},
                    |_| {},
                )?,
                Visit::Control(Control::Notified(index))
            );
            assert_eq!(core.reservation.backing.nodes[2].counts[0], 1 - index);
            core.complete(producer, &mut account)?; // Idempotent outcome, no cursor restart.
            assert_eq!(core.reservation.backing.nodes[2].counts[0], 1 - index);
            assert_eq!(core.reservation.backing.nodes[producer.slot].counts[1], 1);
        }
        let missing = outcomes[0] == Demand::Missing;
        assert_eq!(
            core.reservation.backing.nodes[2].flags & super::control::MISSING != 0,
            missing
        );
        // No truth/refinement or quantified decision exists in a core outcome.
        let mut releases = Vec::new();
        core.abandon(&account, |demand| releases.push(demand))
            .map_err(|(error, _)| error)?;
        assert_eq!(releases, [0, 1]);
    }
    Ok(())
}
