// SPDX-License-Identifier: AGPL-3.0-only
//! Exact proof, export and fresh replay boundaries retain one cumulative caller ledger.

use crate::testing::common::TestResult;
use crate::testing::readings::{ComplexChange, complex_case, ledger, state_case};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::readings::AffineMap;
use nrz_cas::{Fault, FaultKind};

/// The checked coefficient rule has exact Work and retained graph-payload boundaries.
/// ´test:integration:reading-proof-work-and-payload-boundaries´
#[test]
fn reading_proof_work_and_payload_boundaries() -> TestResult {
    let mut measured_case = complex_case(ComplexChange::None)?;
    let mut measured = ledger()?;
    measured_case.prove(&mut measured)?;
    for (kind, needed) in [
        (BudgetKind::Work, measured.usage().work),
        (
            BudgetKind::PayloadBytes,
            measured_case.fixture.cas.counts().payload_bytes,
        ),
    ] {
        for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
            let mut case = complex_case(ComplexChange::None)?;
            let before = case.fixture.cas.counts();
            let limits = if kind == BudgetKind::Work {
                Limits {
                    work: limit,
                    ..Limits::default()
                }
            } else {
                Limits {
                    payload_bytes: limit,
                    ..Limits::default()
                }
            };
            let mut budget = Budget::new(limits)?;
            let result = case.prove(&mut budget);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert!(
                    matches!(result, Err(Fault { kind: FaultKind::Exhausted { budget: actual, .. }, .. }) if actual == kind)
                );
                assert_eq!(case.fixture.cas.counts(), before);
                assert!(budget.usage().work > 0);
            }
        }
    }
    Ok(())
}

/// Export retains exact operator payloads at its Work and witness-byte boundaries.
/// ´test:integration:reading-export-work-and-byte-boundaries´
#[test]
fn reading_export_work_and_byte_boundaries() -> TestResult {
    let mut case = state_case(1)?;
    let witness = case.prove(&mut ledger()?)?;
    let mut measured = ledger()?;
    let bytes = case.fixture.cas.export_witness(&witness, &mut measured)?;
    for (kind, needed) in [
        (BudgetKind::Work, measured.usage().work),
        (BudgetKind::WitnessBytes, u64::try_from(bytes.len())?),
    ] {
        for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
            let limits = if kind == BudgetKind::Work {
                Limits {
                    work: limit,
                    ..Limits::default()
                }
            } else {
                Limits {
                    witness_bytes: limit,
                    ..Limits::default()
                }
            };
            let result = case
                .fixture
                .cas
                .export_witness(&witness, &mut Budget::new(limits)?);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert!(
                    matches!(result, Err(Fault { kind: FaultKind::Exhausted { budget: actual, .. }, .. }) if actual == kind)
                );
            }
        }
    }
    Ok(())
}

/// A fresh receiving ledger covers full decoding, all map compositions and expected-claim checks.
/// ´test:integration:reading-replay-work-boundaries´
#[test]
fn reading_replay_work_boundaries() -> TestResult {
    let mut source = state_case(0)?;
    let witness = source.prove(&mut ledger()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let mut measured_case = state_case(0)?;
    let expected = measured_case.expected()?;
    let mut measured = ledger()?;
    measured_case
        .fixture
        .cas
        .replay(&bytes, &expected, &mut measured)?;
    let needed = measured.usage().work;
    for replay_limit in [false, true] {
        for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
            let mut receiver = state_case(0)?;
            let expected = receiver.expected()?;
            let before = receiver.fixture.cas.counts();
            let limits = if replay_limit {
                Limits {
                    replay_work: limit,
                    ..Limits::default()
                }
            } else {
                Limits {
                    work: limit,
                    ..Limits::default()
                }
            };
            let result = receiver
                .fixture
                .cas
                .replay(&bytes, &expected, &mut Budget::new(limits)?);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                let kind = if replay_limit {
                    BudgetKind::ReplayWork
                } else {
                    BudgetKind::Work
                };
                assert!(
                    matches!(result, Err(Fault { kind: FaultKind::Exhausted { budget: actual, .. }, .. }) if actual == kind)
                );
                assert_eq!(receiver.fixture.cas.counts(), before);
            }
        }
    }
    Ok(())
}

/// Previously spent work remains charged when entering the public reading producer.
/// ´test:integration:reading-proof-cumulative-ledger´
#[test]
fn reading_proof_cumulative_ledger() -> TestResult {
    let mut measured_case = state_case(0)?;
    let mut measured = ledger()?;
    measured_case.prove(&mut measured)?;
    let needed = measured.usage().work;
    let mut case = state_case(0)?;
    let before = case.fixture.cas.counts();
    let mut budget = Budget::new(Limits {
        work: needed,
        ..Limits::default()
    })?;
    AffineMap::identity(1, &mut budget)?;
    let spent = budget.usage().work;
    assert!(matches!(
        case.prove(&mut budget),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    assert!(budget.usage().work >= spent);
    assert_eq!(case.fixture.cas.counts(), before);
    Ok(())
}
