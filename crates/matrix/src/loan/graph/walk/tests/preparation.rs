// SPDX-License-Identifier: AGPL-3.0-only
//! A busy evidence destination prevents entry and preserves the original attempt envelope.

use super::{TestResult, account, graph};
use crate::LoanError;
use crate::loan::graph::{
    sink::{
        Sink,
        admission::{Capacities, Node, Route},
        codec::{Encoding, Evidence, Position},
        delivery::Prepared,
    },
    walk::{
        Error, Handle,
        attempt::Attempt,
        control::Demand,
        drive::{Capacity, Visit},
        waiting::ScanReceipt,
    },
};
use nrz_bindings::Budget;

/// A real delivery owner is prepared on the walk's original account ledger.
struct Destination<'a> {
    /// Complete fixed sink storage, independent of the graph backing.
    sink: &'a Sink,
    /// Original authenticated execution identity.
    account: u64,
    /// No allocation is required for the entered probe's destination.
    prepared: Option<Prepared<'a>>,
    /// Observation of actual entered probes, independent of charged graph Work.
    probes: usize,
}
impl Attempt for Destination<'_> {
    fn prepare(&mut self, _: Handle, exhausted: bool, budget: &mut Budget) -> Result<(), Error> {
        let (slot, encoding) = if exhausted {
            (2, Encoding::Exhaustion)
        } else {
            (1, Encoding::Probe)
        };
        self.prepared = Some(
            self.sink
                .prepare_on(slot, encoding, self.account, budget)
                .map_err(Error::Delivery)?,
        );
        Ok(())
    }
    fn probe(&mut self, _: usize, _: &mut Budget) -> Result<Capacity, LoanError> {
        self.probes += 1;
        Ok(Capacity::Fits)
    }
    fn receipt(&mut self, receipt: ScanReceipt) {
        let prepared = self
            .prepared
            .take()
            .expect("prepared before entered attempt");
        let evidence = receipt
            .capacity
            .map_or(Evidence::Exhaustion, Evidence::Probe);
        prepared.commit(evidence).expect("matching prepaid codec");
    }
}

/// Literal one-node storage is one terminal, one probe and one exhaustion record.
fn sink(account: &mut crate::ExecutionAccount) -> TestResult<Sink> {
    Ok(Sink::admit(
        &[Node {
            position: Position {
                owner: 1,
                node: 0,
                generation: 1,
                phase: 0,
            },
            attempts: 1,
            route: Route::ConsumingReturn,
        }],
        Capacities {
            records: 3,
            words: 19,
            witnesses: 0,
        },
        &[],
        account,
    )?)
}

/// A live immutable pull blocks preparation before either cumulative counter can move.
#[test]
fn graph_probe_destination_precedes_attempt() -> TestResult {
    for attempts in [0, 1] {
        let mut account = account()?;
        let mut core = graph(1, &[attempts], &[], &mut account)?;
        let sink = sink(&mut account)?;
        let delivered = sink
            .prepare(0, Encoding::Outcome, &mut account)?
            .commit(Evidence::Outcome(
                crate::loan::graph::sink::codec::Outcome::Published,
            ))
            .map_err(|(_, e)| e)?;
        let pull = sink.pull(delivered, &mut account)?;
        let mut destination = Destination {
            sink: &sink,
            account: account.identity,
            prepared: None,
            probes: 0,
        };
        let drive = core.reservation.backing.drive;
        let before = account.usage().work;
        let result = core.drive_prepared(
            1,
            &mut account,
            &mut destination,
            |_, _| Ok(Demand::Satisfied),
            |_| {},
        );
        assert!(matches!(
            result,
            Err(Error::Delivery(crate::loan::graph::sink::Error::Borrowed))
        ));
        assert_eq!(destination.probes, 0);
        assert_eq!(core.reservation.backing.nodes[0].attempts, attempts);
        assert_eq!(core.reservation.backing.drive, drive);
        // H entry + M search read + N phase + N attempt + H cursor + M path;
        // sink H entry + M status + six fixed destination fields.
        assert_eq!(account.usage().work - before, 14);
        drop(pull);
        let result = core.drive_prepared(
            1,
            &mut account,
            &mut destination,
            |_, _| Ok(Demand::Satisfied),
            |_| {},
        )?;
        assert!(matches!(
            (attempts, result),
            (0, Visit::AttemptsExhausted(_)) | (1, Visit::Started(_))
        ));
        assert_eq!(destination.probes, usize::from(attempts != 0));
        assert_eq!(core.reservation.backing.drive, drive - 1);
    }
    Ok(())
}

/// A blocked retry leaves the wait-scan cursor and its attempt epoch intact.
#[test]
fn graph_wait_destination_precedes_attempt() -> TestResult {
    let mut account = account()?;
    let mut core = graph(1, &[2], &[], &mut account)?;
    core.drive_one(
        &mut account,
        |_, _| {
            Ok(Capacity::Wait {
                required: 2,
                available: 1,
            })
        },
        |_, _| Ok(Demand::Satisfied),
        |_| {},
        |_| {},
    )?;
    core.begin_capacity_epoch(&mut account)?;
    let sink = sink(&mut account)?;
    let delivered = sink
        .prepare(0, Encoding::Outcome, &mut account)?
        .commit(Evidence::Outcome(
            crate::loan::graph::sink::codec::Outcome::Published,
        ))
        .map_err(|(_, e)| e)?;
    let pull = sink.pull(delivered, &mut account)?;
    let mut destination = Destination {
        sink: &sink,
        account: account.identity,
        prepared: None,
        probes: 0,
    };
    let drive = core.reservation.backing.drive;
    let epoch = core.reservation.backing.nodes[0].epochs[2];
    let cursor = core.reservation.backing.control.waiting[2];
    let result = core.scan_prepared(1, &mut account, &mut destination);
    assert!(matches!(
        result,
        Err(Error::Delivery(crate::loan::graph::sink::Error::Borrowed))
    ));
    assert_eq!(destination.probes, 0);
    assert_eq!(core.reservation.backing.nodes[0].attempts, 1);
    assert_eq!(core.reservation.backing.nodes[0].epochs[2], epoch);
    assert_eq!(core.reservation.backing.drive, drive);
    assert_eq!(core.reservation.backing.control.waiting[2], cursor);
    drop(pull);
    core.scan_prepared(1, &mut account, &mut destination)?;
    assert_eq!(destination.probes, 1);
    assert_eq!(core.reservation.backing.nodes[0].attempts, 0);
    Ok(())
}
