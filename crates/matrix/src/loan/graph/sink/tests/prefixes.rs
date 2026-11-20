// SPDX-License-Identifier: AGPL-3.0-only
//! Independent charged-prefix arithmetic and required receipt/owner histories.

use super::{
    CAPACITY, Encoding, Error, Evidence, NODES, Outcome, Sink, Status, TestResult, account,
    live_bytes,
};
use nrz_bindings::Limits;

/// Admission H3, M18, I55, P11 =87; prior5 is retained. No T or A is hidden here.
/// M: four contract words, twelve copied position words, one attempt reread, one debit update.
const ADMISSION: u64 = 87;

/// Each category is tested at refusal prefixes, never through a production event calculator.
pub(super) fn run() -> TestResult {
    work()?;
    receipts()?;
    acknowledgement_prefixes()?;
    probe_prefixes()?;
    allocation_prefixes()?;
    surviving_debit()?;
    graph_admission()?;
    prepaid_drop()?;
    codecs()
}
/// Five earlier Work precedes every exact and one-below fixture.
fn work() -> TestResult {
    for short in [0, 1] {
        let mut account = account(Limits {
            work: 5 + ADMISSION - short,
            ..Limits::default()
        })?;
        let result = Sink::admit(&NODES, CAPACITY, &[], &mut account);
        if short == 0 {
            drop(result?);
            assert_eq!(account.usage().work, 92);
        } else {
            assert!(result.is_err());
            assert_eq!(account.usage().work, 11);
        }
    }
    // H entry, M status, M six preflight words, then T1 + M6 + I6 + M5 =18 atomic prepayment.
    for spare in 0..=26 {
        let mut account = account(Limits {
            work: 92 + spare,
            ..Limits::default()
        })?;
        let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
        let result = sink.prepare(0, Encoding::Outcome, &mut account);
        if spare == 26 {
            drop(result?);
        } else {
            assert!(result.is_err());
            drop(result);
        }
        let spent = match spare {
            0 => 0,
            1 => 1,
            2..=7 => 2,
            8..=25 => 8,
            _ => 26,
        };
        assert_eq!(account.usage().work, 92 + spent);
        assert_eq!(sink.backing.slots[0].state.get(), 1);
    }
    Ok(())
}
/// Every entered probe and exhaustion adds a required acknowledgement; no second queue exists.
fn receipts() -> TestResult {
    let mut account = account(Limits::default())?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let (remaining, probe) = sink
        .probe(1, 1_u64, &mut account, |attempts, _| {
            *attempts -= 1;
            super::Capacity::Wait {
                required: 23,
                available: 17,
            }
        })
        .map_err(|(_, error)| error)?;
    assert_eq!(remaining, 0);
    // Defensive library fixture: the already filled destination blocks a second entry.
    let mut entered = false;
    let stopped = sink.probe(1, 1_u64, &mut account, |attempts, _| {
        entered = true;
        *attempts -= 1;
        super::Capacity::Fits
    });
    let (remaining, error) = stopped.expect_err("occupied reserved destination");
    assert_eq!(remaining, 1);
    assert!(matches!(error, Error::Full));
    assert!(!entered);
    let exhaustion = sink
        .prepare(2, Encoding::Exhaustion, &mut account)?
        .commit(Evidence::Exhaustion)
        .map_err(|(_, e)| e)?;
    let terminal = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(Outcome::Cancelled))
        .map_err(|(_, e)| e)?;
    sink.acknowledge(terminal, &mut account)?;
    sink.acknowledge(probe, &mut account)?;
    assert_eq!(sink.status(&mut account)?, Status::Incomplete);
    sink.acknowledge(exhaustion, &mut account)?;
    assert_eq!(sink.status(&mut account)?, Status::Complete);
    Ok(())
}
/// Each failed acquisition unwinds its prepaid private prefix; acquired registration remains counted.
fn allocation_prefixes() -> TestResult {
    for fail in 0..5 {
        let mut account = account(Limits::default())?;
        let injection = crate::loan::fault::Injection::after(fail);
        let result = Sink::admit(&NODES, CAPACITY, &[], &mut account);
        drop(injection);
        assert!(matches!(
            result,
            Err(Error::Native(crate::LoanError::AllocationRefused))
        ));
        assert_eq!(live_bytes(&account)?, if fail == 4 { 24 } else { 0 });
        assert_eq!(account.usage().work, 92);
        let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
        assert_eq!(live_bytes(&account)?, 600);
        drop(sink);
        assert_eq!(live_bytes(&account)?, 24);
    }
    Ok(())
}
/// Omitting a dropped graph from caller inventory cannot hide its surviving witness debit.
fn surviving_debit() -> TestResult {
    let mut account = account(Limits {
        payload_bytes: 1_175,
        ..Limits::default()
    })?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let handle = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(Outcome::Published))
        .map_err(|(_, e)| e)?;
    let witness = sink.witness(handle, &mut account)?;
    drop(sink);
    assert!(Sink::admit(&NODES, CAPACITY, &[], &mut account).is_err());
    assert_eq!(witness.status(&mut account)?, Status::Abandoned);
    let before = account.usage().work;
    drop(witness);
    assert_eq!(account.usage().work, before);
    drop(Sink::admit(&NODES, CAPACITY, &[], &mut account)?);
    Ok(())
}
/// Closed dispositions and each original capacity alternative preserve every retained scalar.
fn codecs() -> TestResult {
    for (outcome, expected) in [
        (Outcome::Absent, 1),
        (Outcome::Cancelled, 2),
        (Outcome::DependencyRefused, 3),
    ] {
        assert_eq!(
            Evidence::Outcome(outcome).encode(super::POSITION),
            [42, 7, 19, 3, expected, 1, 0, 0]
        );
    }
    assert_eq!(
        Evidence::Probe(super::Capacity::Fits).encode(super::POSITION),
        [42, 7, 19, 3, 0, 0, 0, 3]
    );
    assert_eq!(
        Evidence::Probe(super::Capacity::Intrinsic {
            required: 101,
            limit: 100
        })
        .encode(super::POSITION),
        [42, 7, 19, 3, 2, 101, 100, 3]
    );
    let mut account = account(Limits::default())?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let handle = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(Outcome::Published))
        .map_err(|(_, e)| e)?;
    let before = account.usage().work;
    let first = sink.witness(handle, &mut account)?;
    assert_eq!(account.usage().work - before, 11); // H1 A1 M2 + M1 + T1 M2 P3.
    let second = sink.witness(handle, &mut account)?;
    assert_eq!(account.usage().work - before, 23); // second examines two witness slots.
    assert!(matches!(
        sink.witness(handle, &mut account),
        Err(Error::Full)
    ));
    let mut foreign = super::account(Limits::default())?;
    let before = foreign.usage();
    assert!(sink.pull(handle, &mut foreign).is_err());
    assert_eq!(foreign.usage(), before);
    drop((first, second));
    Ok(())
}

/// The graph reservation itself includes account-resident sink bytes without a caller owner list.
fn graph_admission() -> TestResult {
    use crate::loan::graph::{Arguments, Reservation};
    let args = Arguments {
        nodes: 2,
        edges: 1,
        proposed_edges: 1,
        scopes: 1,
        drive: 1_000,
        attempts: &[100, 200],
    };
    for staged in [false, true] {
        for short in [0, 1] {
            // Independent O fixture832 plus live K600; dropping the K graph cannot subtract600.
            let limits = if staged {
                Limits {
                    staged_payload_bytes: 1_432 - short,
                    ..Limits::default()
                }
            } else {
                Limits {
                    payload_bytes: 1_432 - short,
                    ..Limits::default()
                }
            };
            let mut account = account(limits)?;
            let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
            let handle = sink
                .prepare(0, Encoding::Outcome, &mut account)?
                .commit(Evidence::Outcome(Outcome::Published))
                .map_err(|(_, e)| e)?;
            let witness = sink.witness(handle, &mut account)?;
            drop(sink);
            let graph = Reservation::reserve::<f32>(args, &[], &mut account);
            if short == 0 {
                drop(graph?);
            } else {
                assert!(graph.is_err());
            }
            drop(witness);
            drop(Reservation::reserve::<f64>(args, &[], &mut account)?);
        }
    }
    Ok(())
}
/// Final edge/storage/debit release succeeds at exactly exhausted original Work.
fn prepaid_drop() -> TestResult {
    let mut account = account(Limits {
        work: 129,
        ..Limits::default()
    })?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let handle = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(Outcome::Published))
        .map_err(|(_, e)| e)?;
    let witness = sink.witness(handle, &mut account)?;
    assert_eq!(account.usage().work, 129);
    drop(sink);
    assert_eq!(live_bytes(&account)?, 600);
    drop(witness);
    assert_eq!(live_bytes(&account)?, 24);
    assert_eq!(account.usage().work, 129);
    Ok(())
}

/// Work refusal cannot partially acknowledge a record or release its required obligation.
fn acknowledgement_prefixes() -> TestResult {
    for spare in 0..=8 {
        let mut account = account(Limits {
            work: 118 + spare,
            ..Limits::default()
        })?;
        let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
        let handle = sink
            .prepare(0, Encoding::Outcome, &mut account)?
            .commit(Evidence::Outcome(Outcome::Published))
            .map_err(|(_, e)| e)?;
        let result = sink.acknowledge(handle, &mut account);
        if spare == 8 {
            result?;
            assert_eq!(sink.backing.slots[0].state.get(), 3);
        } else {
            assert!(result.is_err());
            assert_eq!(sink.backing.slots[0].state.get(), 2);
        }
        let spent = match spare {
            0..=1 => 0,
            2..=3 => 2,
            4..=7 => 4,
            _ => 8,
        };
        assert_eq!(account.usage().work, 118 + spent);
    }
    Ok(())
}
/// Every failed probe prepayment leaves the native attempt and its original capability intact.
fn probe_prefixes() -> TestResult {
    for spare in 0..=30 {
        let mut account = account(Limits {
            work: 92 + spare,
            ..Limits::default()
        })?;
        let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
        let result = sink.probe(1, 1_u64, &mut account, |attempts, _| {
            *attempts -= 1;
            super::Capacity::Fits
        });
        if spare == 30 {
            assert_eq!(result.map_err(|(_, e)| e)?.0, 0);
        } else {
            let (attempts, _) = result.expect_err("pre-probe Work stop");
            assert_eq!(attempts, 1);
        }
        // H1, M1 status, M6 preflight, then T1 M8 I8 M5 =22 atomic prepayment.
        let spent = match spare {
            0 => 0,
            1 => 1,
            2..=7 => 2,
            8..=29 => 8,
            _ => 30,
        };
        assert_eq!(account.usage().work, 92 + spent);
    }
    Ok(())
}
