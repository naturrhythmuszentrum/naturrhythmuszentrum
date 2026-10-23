// SPDX-License-Identifier: AGPL-3.0-only
//! Caller-owned live payload shares staged ceilings without resetting cumulative Work.

use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::{Fault, FaultKind, Location};

const AT: Location = Location::new("retained payload contract");

#[derive(Debug, Eq, PartialEq)]
enum ConsumerError {
    Resource(Fault),
    Reading,
}

impl From<Fault> for ConsumerError {
    fn from(fault: Fault) -> Self {
        Self::Resource(fault)
    }
}

fn ledger(staged: u64, work: u64) -> Result<Budget, Fault> {
    Budget::new(Limits {
        staged_payload_bytes: staged,
        work,
        ..Limits::default()
    })
}

/// N retained bytes fit at N staged allowance and refuse at N minus one before the callback.
/// ´test:integration:retained-payload-exact-staged-boundary´
#[test]
fn retained_payload_exact_staged_boundary() -> Result<(), Fault> {
    let mut budget = ledger(32, 10)?;
    let before = budget.limits().clone();
    budget.with_retained_payload(32, AT, |inner| {
        assert_eq!(inner.limits().staged_payload_bytes, 0);
        inner.check(BudgetKind::StagedPayloadBytes, 0, AT)
    })?;
    assert_eq!(budget.limits(), &before);
    let mut budget = ledger(31, 10)?;
    let before = budget.limits().clone();
    let usage = budget.usage();
    let mut called = false;
    let result = budget.with_retained_payload(32, AT, |_| {
        called = true;
        Ok::<(), Fault>(())
    });
    assert!(matches!(
        result,
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::StagedPayloadBytes,
                limit: 31,
                needed: 32,
                ..
            },
            ..
        })
    ));
    assert!(!called);
    assert_eq!(budget.limits(), &before);
    assert_eq!(budget.usage(), usage);
    Ok(())
}

/// Both result paths restore every limit while retaining callback Work and consumer error identity.
/// ´test:integration:retained-payload-restores-limits-and-keeps-work´
#[test]
fn retained_payload_restores_limits_and_keeps_work() -> Result<(), Fault> {
    let mut budget = ledger(64, 20)?;
    let before = budget.limits().clone();
    let result = budget.with_retained_payload(13, AT, |inner| {
        assert_eq!(inner.limits().staged_payload_bytes, 51);
        inner.work(5, AT)?;
        Ok::<_, ConsumerError>(7)
    });
    assert_eq!(result, Ok(7));
    assert_eq!(budget.limits(), &before);
    assert_eq!(budget.usage().work, 5);
    let result = budget.with_retained_payload(17, AT, |inner| {
        assert_eq!(inner.limits().staged_payload_bytes, 47);
        inner.work(7, AT)?;
        Err::<(), _>(ConsumerError::Reading)
    });
    assert_eq!(result, Err(ConsumerError::Reading));
    assert_eq!(budget.limits(), &before);
    assert_eq!(budget.usage().work, 12);
    Ok(())
}

/// Nested reservations only shrink the remaining allowance and restore each enclosing scope.
/// ´test:integration:retained-payload-nested-reservations-compose´
#[test]
fn retained_payload_nested_reservations_compose() -> Result<(), Fault> {
    let mut budget = ledger(100, 20)?;
    budget.with_retained_payload(40, AT, |outer| {
        assert_eq!(outer.limits().staged_payload_bytes, 60);
        outer.with_retained_payload(50, AT, |inner| {
            assert_eq!(inner.limits().staged_payload_bytes, 10);
            let before = inner.limits().clone();
            let usage = inner.usage();
            let result = inner.with_retained_payload(11, AT, |_| Ok::<(), Fault>(()));
            assert!(matches!(
                result,
                Err(Fault {
                    kind: FaultKind::Exhausted {
                        budget: BudgetKind::StagedPayloadBytes,
                        limit: 10,
                        needed: 11,
                        ..
                    },
                    ..
                })
            ));
            assert_eq!(inner.limits(), &before);
            assert_eq!(inner.usage(), usage);
            inner.work(3, AT)
        })?;
        assert_eq!(outer.limits().staged_payload_bytes, 60);
        outer.work(2, AT)
    })?;
    assert_eq!(budget.limits().staged_payload_bytes, 100);
    assert_eq!(budget.usage().work, 5);
    Ok(())
}

/// Callback Work has its exact N boundary and a prior debit cannot be reset by reservation.
/// ´test:integration:retained-payload-cumulative-work-boundary´
#[test]
fn retained_payload_cumulative_work_boundary() -> Result<(), Fault> {
    let mut budget = ledger(64, 9)?;
    budget.with_retained_payload(16, AT, |inner| inner.work(9, AT))?;
    assert_eq!(budget.usage().work, 9);
    let mut budget = ledger(64, 8)?;
    assert!(
        budget
            .with_retained_payload(16, AT, |inner| inner.work(9, AT))
            .is_err()
    );
    assert_eq!(budget.limits().staged_payload_bytes, 64);
    let mut budget = ledger(64, 9)?;
    budget.work(1, AT)?;
    assert!(
        budget
            .with_retained_payload(16, AT, |inner| inner.work(9, AT))
            .is_err()
    );
    assert_eq!(budget.usage().work, 1);
    assert_eq!(budget.limits().staged_payload_bytes, 64);
    Ok(())
}
