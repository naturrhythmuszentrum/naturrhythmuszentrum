// SPDX-License-Identifier: AGPL-3.0-only
//! Literal controls for delivery events, persistent stops and multiple receipt destinations.

use super::{
    CAPACITY, Capacities, Capacity, Encoding, Error, Evidence, NODES, Node, Outcome, Position,
    Sink, Status, TestResult, account, live_bytes,
};
use nrz_bindings::Limits;

/// Review cases extend the same K control without a second production oracle.
pub(super) fn run() -> TestResult {
    delivery_prefixes()?;
    witness_prefixes()?;
    overlength_commit()?;
    word_extent()?;
    multiple_receipts()
}
/// Delivery shares preparation's one H; every refused prefix returns its original capability.
fn delivery_prefixes() -> TestResult {
    for spare in 0..=26 {
        let mut account = account(Limits {
            work: 92 + spare,
            ..Limits::default()
        })?;
        let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
        let returned = sink.deliver(
            0,
            73_u64,
            Evidence::Outcome(Outcome::Published),
            &mut account,
        );
        assert_eq!(returned.capability, 73);
        if spare == 26 {
            returned.receipt?;
            assert_eq!(sink.backing.slots[0].state.get(), 2);
            assert_eq!(&sink.backing.words.borrow()[..6], [42, 7, 19, 3, 0, 1]);
        } else {
            assert!(returned.receipt.is_err());
            assert_eq!(sink.backing.slots[0].state.get(), 1);
            assert!(sink.backing.words.borrow().iter().all(|word| *word == 0));
        }
        // H1, M1, M6, then T1 M6 I6 M5 =18 atomic prepayment.
        let spent = match spare {
            0 => 0,
            1 => 1,
            2..=7 => 2,
            8..=25 => 8,
            _ => 26,
        };
        assert_eq!(account.usage().work, 92 + spent);
    }
    Ok(())
}
/// Witness inspection has no owner comparison: H1 M3 is one four-unit prepayment.
fn witness_prefixes() -> TestResult {
    for spare in 0..=4 {
        let mut account = account(Limits {
            work: 129 + spare,
            ..Limits::default()
        })?;
        let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
        let handle = sink
            .prepare(0, Encoding::Outcome, &mut account)?
            .commit(Evidence::Outcome(Outcome::Published))
            .map_err(|(_, error)| error)?;
        let witness = sink.witness(handle, &mut account)?;
        assert_eq!(account.usage().work, 129);
        let mut inspected = false;
        let result = witness.inspect(&mut account, |words, bytes| {
            inspected = true;
            assert_eq!(words, [42, 7, 19, 3, 0, 1]);
            assert_eq!(bytes, 41);
        });
        if spare == 4 {
            result?;
        } else {
            assert!(result.is_err());
        }
        assert_eq!(inspected, spare == 4);
        assert_eq!(account.usage().work, if spare == 4 { 133 } else { 129 });
    }
    Ok(())
}
/// An already prepared terminal may finish, but cannot erase another destination's stop.
fn overlength_commit() -> TestResult {
    let mut account = account(Limits::default())?;
    let mut sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    std::rc::Rc::get_mut(&mut sink.backing)
        .ok_or("exclusive bound fixture")?
        .slots[1]
        .bound = 56;
    let prepared = sink.prepare(0, Encoding::Outcome, &mut account)?;
    assert!(matches!(
        sink.prepare(1, Encoding::Probe, &mut account),
        Err(Error::Overlength)
    ));
    assert_eq!(sink.status(&mut account)?, Status::Overlength);
    let handle = prepared
        .commit(Evidence::Outcome(Outcome::Published))
        .map_err(|(_, error)| error)?;
    assert_eq!(sink.status(&mut account)?, Status::Overlength);
    assert!(
        sink.backing.words.borrow()[6..14]
            .iter()
            .all(|word| *word == 0)
    );
    assert_eq!(sink.backing.slots[1].state.get(), 0);
    sink.acknowledge(handle, &mut account)?;
    assert_eq!(sink.status(&mut account)?, Status::Overlength);
    assert!(matches!(
        sink.prepare(2, Encoding::Exhaustion, &mut account),
        Err(Error::Overlength)
    ));
    Ok(())
}
/// Three records cannot fit in eighteen words: six terminal, eight probe and five exhaustion.
fn word_extent() -> TestResult {
    let mut account = account(Limits::default())?;
    assert!(matches!(
        Sink::admit(
            &NODES,
            Capacities {
                words: 18,
                ..CAPACITY
            },
            &[],
            &mut account
        ),
        Err(Error::Full)
    ));
    assert_eq!(live_bytes(&account)?, 0);
    assert_eq!(account.usage().work, 11);
    Ok(())
}
/// Two nodes and five attempts reserve two terminals plus seven receipt destinations.
fn multiple_receipts() -> TestResult {
    let nodes = [
        Node {
            attempts: 2,
            ..NODES[0]
        },
        Node {
            position: Position {
                node: 8,
                ..super::POSITION
            },
            attempts: 3,
            ..NODES[0]
        },
    ];
    let capacity = Capacities {
        records: 9,
        words: 62,
        witnesses: 0,
    };
    let mut account = account(Limits::default())?;
    assert!(matches!(
        Sink::admit(
            &nodes,
            Capacities {
                words: 61,
                ..capacity
            },
            &[],
            &mut account
        ),
        Err(Error::Full)
    ));
    assert_eq!(live_bytes(&account)?, 0);
    let sink = Sink::admit(&nodes, capacity, &[], &mut account)?;
    // 24 registration +144 shared owner +9*88 records +62*8 words.
    assert_eq!(live_bytes(&account)?, 1_456);
    assert_eq!(sink.backing.slots.len(), 9);
    assert_eq!(sink.backing.words.borrow().len(), 62);
    assert_eq!(sink.backing.outstanding.get(), 2);
    let mut handles = Vec::new();
    for (slot, node, encoding, offset) in [
        (0, 7, Encoding::Outcome, 0),
        (1, 8, Encoding::Outcome, 6),
        (2, 7, Encoding::Probe, 12),
        (3, 7, Encoding::Probe, 20),
        (4, 7, Encoding::Exhaustion, 28),
        (5, 8, Encoding::Probe, 33),
        (6, 8, Encoding::Probe, 41),
        (7, 8, Encoding::Probe, 49),
        (8, 8, Encoding::Exhaustion, 57),
    ] {
        let evidence = match encoding {
            Encoding::Outcome => Evidence::Outcome(Outcome::Cancelled),
            Encoding::Probe => Evidence::Probe(Capacity::Fits),
            _ => Evidence::Exhaustion,
        };
        assert_eq!(sink.backing.slots[slot].offset, offset);
        let handle = sink
            .prepare(slot, encoding, &mut account)?
            .commit(evidence)
            .map_err(|(_, error)| error)?;
        let pull = sink.pull(handle, &mut account)?;
        assert_eq!(&pull.words()[..4], [42, node, 19, 3]);
        drop(pull);
        handles.push(handle);
    }
    assert_eq!(sink.backing.outstanding.get(), 9);
    for (index, handle) in handles.into_iter().enumerate() {
        sink.acknowledge(handle, &mut account)?;
        assert_eq!(sink.backing.outstanding.get(), 8 - index);
        assert_eq!(
            sink.status(&mut account)?,
            if index == 8 {
                Status::Complete
            } else {
                Status::Incomplete
            }
        );
    }
    Ok(())
}
