// SPDX-License-Identifier: AGPL-3.0-only
//! Independent primitive layouts, encoded words and literal operation prefixes.

mod native;
pub(in crate::loan) use native::returned;
mod prefixes;
mod review;

use super::admission::{Capacities, Node, Route};
use super::codec::{Capacity, Encoding, Evidence, Outcome, Position};
use super::{Backing, Error, Handle, Sink, Slot, Status, live_bytes};
use crate::ExecutionAccount;
use nrz_bindings::{Budget, Limits};

/// Controls use original failures without formatting them into evidence.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// Actual one-node allocation: shared144 + records264 + words152 + witness16 + registration24.
const BYTES: usize = 600;
/// Caller extents include one terminal, one probe and one exhaustion destination.
const CAPACITY: Capacities = Capacities {
    records: 3,
    words: 19,
    witnesses: 2,
};
/// Original evidence position is independent of the sink slot index.
const POSITION: Position = Position {
    owner: 42,
    node: 7,
    generation: 19,
    phase: 3,
};
/// Original finite attempts are unrelated to the Work ceiling.
const NODES: [Node; 1] = [Node {
    position: POSITION,
    attempts: 1,
    route: Route::ConsumingReturn,
}];

/// Full independent sink controls are joined with actual native consuming carriers.
pub(in crate::loan) fn run() -> TestResult {
    layout();
    capacities()?;
    borrowed_record()?;
    codecs()?;
    lifecycle()?;
    boundaries()?;
    interruptions()?;
    prefixes::run()?;
    review::run()?;
    refusals()
}
/// Every fixture includes five original Work, before account creation.
fn account(limits: Limits) -> TestResult<ExecutionAccount> {
    let mut budget = Budget::new(limits)?;
    budget.work(5, crate::loan::AT)?;
    Ok(ExecutionAccount::new(budget)?)
}
/// Independent primitive mirrors never call the production plan or byte calculator.
fn layout() {
    #[repr(C)]
    struct SlotMirror([u64; 11]);
    #[repr(C)]
    struct OwnerMirror {
        identity_status: [u64; 4],
        slots: [usize; 3],
        words_borrow_and_vector: [usize; 4],
        witnesses: [usize; 3],
        debit: [usize; 2],
    }
    assert_eq!(size_of::<SlotMirror>(), 88);
    assert_eq!(size_of::<Slot>(), 88);
    assert_eq!(size_of::<OwnerMirror>(), 128);
    assert_eq!(size_of::<Backing>(), 128);
}
/// Every supported codec has hand-enumerated fields, lengths, encoded words and event totals.
fn codecs() -> TestResult {
    let cases = [
        (
            0,
            Encoding::Outcome,
            Evidence::Outcome(Outcome::Published),
            41,
            vec![42, 7, 19, 3, 0, 1],
            26,
        ),
        (
            1,
            Encoding::Probe,
            Evidence::Probe(Capacity::Wait {
                required: 23,
                available: 17,
            }),
            57,
            vec![42, 7, 19, 3, 1, 23, 17, 3],
            30,
        ),
        (
            2,
            Encoding::Exhaustion,
            Evidence::Exhaustion,
            33,
            vec![42, 7, 19, 3, 4],
            24,
        ),
    ];
    for (slot, encoding, evidence, length, words, work) in cases {
        let mut account = account(Limits::default())?;
        let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
        assert_eq!(live_bytes(&account)?, BYTES);
        let before = account.usage().work;
        let prepared = sink.prepare(slot, encoding, &mut account)?;
        let handle = prepared.commit(evidence).map_err(|(_, error)| error)?;
        // H1 + M7 preflight; T1 + M(fields+5) + I(encoded words).
        assert_eq!(account.usage().work - before, work);
        let before = account.usage().work;
        let pull = sink.pull(handle, &mut account)?;
        assert_eq!(pull.bytes(), length);
        assert_eq!(pull.words(), words);
        assert_eq!(pull.handle, handle);
        assert_eq!(account.usage().work - before, 6); // H1 A1 M4.
        drop(pull);
        sink.acknowledge(handle, &mut account)?;
        assert_eq!(account.usage().work - before, 14); // pull6, ack H1 A1 M2 T1 M3.
    }
    Ok(())
}
/// An acknowledged witness blocks reuse and survives abandonment without owning the graph.
fn lifecycle() -> TestResult {
    let mut account = account(Limits::default())?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let handle = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(Outcome::Published))
        .map_err(|(_, e)| e)?;
    let witness = sink.witness(handle, &mut account)?;
    sink.acknowledge(handle, &mut account)?;
    assert_eq!(sink.status(&mut account)?, Status::Complete);
    assert!(matches!(
        sink.prepare(0, Encoding::Outcome, &mut account),
        Err(Error::Full)
    ));
    let before = account.usage().work;
    drop(sink);
    assert_eq!(account.usage().work, before);
    assert_eq!(live_bytes(&account)?, BYTES);
    assert_eq!(witness.status(&mut account)?, Status::Abandoned);
    witness.inspect(&mut account, |words, bytes| {
        assert_eq!(words, [42, 7, 19, 3, 0, 1]);
        assert_eq!(bytes, 41);
    })?;
    let newer = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    assert!(newer.acknowledge(handle, &mut account).is_err());
    witness.inspect(&mut account, |words, _| {
        assert_eq!(words, [42, 7, 19, 3, 0, 1]);
    })?;
    assert_eq!(live_bytes(&account)?, 1_176);
    drop(newer);
    let before = account.usage().work;
    drop(witness);
    assert_eq!(account.usage().work, before);
    assert_eq!(live_bytes(&account)?, 24);
    Ok(())
}
/// Exact and one-below limits reach sink admission with prior Work and surviving bytes.
fn boundaries() -> TestResult {
    for staged in [false, true] {
        for owners in [1, 2] {
            for short in [0, 1] {
                let boundary = 24 + 576 * owners - short;
                let limits = if staged {
                    Limits {
                        staged_payload_bytes: boundary,
                        ..Limits::default()
                    }
                } else {
                    Limits {
                        payload_bytes: boundary,
                        ..Limits::default()
                    }
                };
                let mut account = account(limits)?;
                let old = if owners == 2 {
                    Some(Sink::admit(&NODES, CAPACITY, &[], &mut account)?)
                } else {
                    None
                };
                let result = Sink::admit(&NODES, CAPACITY, &[], &mut account);
                if short == 0 {
                    drop(result?);
                } else {
                    assert!(matches!(result, Err(Error::Native(_))));
                }
                drop(old);
            }
        }
    }
    Ok(())
}
/// Dropping a prepaid private transaction changes no record; retry retains spent Work.
fn interruptions() -> TestResult {
    let mut account = account(Limits::default())?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let before = account.usage().work;
    drop(sink.prepare(0, Encoding::Outcome, &mut account)?);
    assert_eq!(sink.backing.slots[0].state.get(), 1);
    let handle = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(Outcome::Published))
        .map_err(|(_, e)| e)?;
    assert_eq!(account.usage().work - before, 52);
    let foreign = Handle {
        owner: handle.owner + 1,
        ..handle
    };
    assert!(matches!(
        sink.acknowledge(foreign, &mut account),
        Err(Error::Native(crate::LoanError::ForeignOwner))
    ));
    sink.acknowledge(handle, &mut account)?;
    assert!(matches!(
        sink.acknowledge(handle, &mut account),
        Err(Error::Native(crate::LoanError::DuplicateReturn))
    ));
    let next = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(Outcome::Absent))
        .map_err(|(_, e)| e)?;
    assert_eq!(next.generation, handle.generation + 1);
    assert!(matches!(
        sink.acknowledge(handle, &mut account),
        Err(Error::Native(crate::LoanError::StaleOccupation))
    ));
    sink.acknowledge(next, &mut account)?;
    sink.backing.slots[0].generation.set(u64::MAX);
    assert!(matches!(
        sink.prepare(0, Encoding::Outcome, &mut account),
        Err(Error::Full)
    ));
    assert_eq!(sink.backing.slots[0].state.get(), 4);
    Ok(())
}
/// Unknown bounds and retaining-node policies refuse before any sink storage escapes.
fn refusals() -> TestResult {
    for route in [
        Route::RetainUnsupportedScale,
        Route::RetainConflictingKind,
        Route::RetainSeriesConstant,
        Route::Unproved,
    ] {
        // Mapped transport, construction, replay and checked-omega phase positions
        // all receive the same complete reachable-field admission check.
        for phase in 0..4 {
            let mut account = account(Limits::default())?;
            let node = Node {
                route,
                position: Position { phase, ..POSITION },
                ..NODES[0]
            };
            assert!(matches!(
                Sink::admit(&[node], CAPACITY, &[], &mut account),
                Err(Error::Unbounded)
            ));
            assert_eq!(live_bytes(&account)?, 0);
        }
    }
    let mut account = account(Limits::default())?;
    let capacity = Capacities {
        records: 2,
        ..CAPACITY
    };
    assert!(matches!(
        Sink::admit(&NODES, capacity, &[], &mut account),
        Err(Error::Full)
    ));
    let mut sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    std::rc::Rc::get_mut(&mut sink.backing)
        .ok_or("exclusive fixture")?
        .slots[0]
        .bound = 40;
    assert!(matches!(
        sink.prepare(0, Encoding::Outcome, &mut account),
        Err(Error::Overlength)
    ));
    assert_eq!(sink.status(&mut account)?, Status::Overlength);
    assert_eq!(sink.backing.slots[0].state.get(), 1);
    Ok(())
}

/// Spare slots, spare evidence words and zero capacities are inventoried as actual allocations.
fn capacities() -> TestResult {
    for capacity in [
        CAPACITY,
        Capacities {
            records: 5,
            words: 23,
            witnesses: 4,
        },
    ] {
        let mut account = account(Limits::default())?;
        let sink = Sink::admit(&NODES, capacity, &[], &mut account)?;
        assert_eq!(sink.backing.slots.capacity(), capacity.records);
        assert_eq!(sink.backing.words.borrow().capacity(), capacity.words);
        assert_eq!(sink.backing.witnesses.capacity(), capacity.witnesses);
        assert_eq!(
            live_bytes(&account)?,
            24 + 144 + 88 * capacity.records + 8 * capacity.words + 8 * capacity.witnesses
        );
    }
    let mut account = account(Limits::default())?;
    let empty = Sink::admit(
        &[],
        Capacities {
            records: 0,
            words: 0,
            witnesses: 0,
        },
        &[],
        &mut account,
    )?;
    assert_eq!(live_bytes(&account)?, 168);
    assert_eq!(empty.status(&mut account)?, Status::Complete);
    drop(empty);
    assert_eq!(live_bytes(&account)?, 24);
    Ok(())
}

/// An immutable pull must end before acknowledgement; refusal leaves the record outstanding.
fn borrowed_record() -> TestResult {
    let mut account = account(Limits::default())?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let handle = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(Outcome::Published))
        .map_err(|(_, e)| e)?;
    let pull = sink.pull(handle, &mut account)?;
    assert!(matches!(
        sink.acknowledge(handle, &mut account),
        Err(Error::Borrowed)
    ));
    assert_eq!(pull.words(), [42, 7, 19, 3, 0, 1]);
    assert_eq!(sink.backing.slots[0].state.get(), 2);
    drop(pull);
    sink.acknowledge(handle, &mut account)?;
    Ok(())
}
