// SPDX-License-Identifier: AGPL-3.0-only
//! Paired fixed destinations preserve both entered-probe and intact-refusal obligations.

use super::{CAPACITY, Encoding, Error, Evidence, NODES, Sink, Status, TestResult, account};
use crate::loan::graph::sink::codec::{ProbeFailure, Refusal};
use nrz_bindings::Limits;

/// Two independent fixed transactions cost Probe30 + Refusal30, without another allocation.
pub(super) fn run() -> TestResult {
    let mut account = account(Limits::default())?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let before = account.usage().work;
    let (probe, refusal) =
        sink.prepare_pair((1, Encoding::Probe), (0, Encoding::Refusal), &mut account)?;
    assert_eq!(account.usage().work - before, 60);
    let probe = probe
        .at_native_state(9)
        .commit(Evidence::ProbeFailure(ProbeFailure::Repair))
        .map_err(|(_, error)| error)?;
    assert!(matches!(
        sink.pull(probe, &mut account),
        Err(Error::Borrowed)
    ));
    let refusal = refusal
        .at_native_state(9)
        .commit(Evidence::Refusal(Refusal::Loan))
        .map_err(|(_, error)| error)?;
    {
        let record = sink.pull(probe, &mut account)?;
        assert_eq!(record.words(), [42, 7, 19, 9, 3, 3, 0, 3]);
        assert_eq!(record.bytes(), 57);
    }
    {
        let record = sink.pull(refusal, &mut account)?;
        assert_eq!(record.words(), [42, 7, 19, 9, 3, 2]);
        assert_eq!(record.bytes(), 41);
    }
    sink.acknowledge(probe, &mut account)?;
    assert_eq!(sink.status(&mut account)?, Status::Incomplete);
    sink.acknowledge(refusal, &mut account)?;
    assert_eq!(sink.status(&mut account)?, Status::Complete);
    interrupted()
}

/// A one-below paired prepayment leaves both destinations and all evidence words unchanged.
fn interrupted() -> TestResult {
    let mut account = account(Limits {
        work: 92 + 59,
        ..Limits::default()
    })?;
    let sink = Sink::admit(&NODES, CAPACITY, &[], &mut account)?;
    let before = account.usage().work;
    assert!(
        sink.prepare_pair((1, Encoding::Probe), (0, Encoding::Refusal), &mut account)
            .is_err()
    );
    // Two H1+M1+M6 inspections, then the probe's T1+M8+I8+M5;
    // the refusal's indivisible T1+M4+M6+I6+M5 prepayment cannot fit.
    assert_eq!(account.usage().work - before, 38);
    assert_eq!(sink.backing.slots[0].state.get(), 1);
    assert_eq!(sink.backing.slots[1].state.get(), 0);
    assert!(sink.backing.words.borrow().iter().all(|word| *word == 0));
    Ok(())
}
