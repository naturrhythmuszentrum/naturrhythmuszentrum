// SPDX-License-Identifier: AGPL-3.0-only
//! Controls for the shared standalone measurement implementation.
use super::*;

/// Independent five-plus-five tariff separates children from the remainder.
/// Seven prior Work is outside observation; eleven trailing Work is unclassified.
/// # Errors
/// Returns budget or lock failures.
/// # Panics
/// Panics if a child is counted twice or unclassified Work disappears.
/// ´claim:matrix:prepared-exponential-measurement-attribution´
/// ´test:integration:prepared-exponential-measurement-attribution´
#[test]
fn prepared_exponential_measurement_attribution() -> TestResult {
    use nrz_bindings::readings::algebra::Location;
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    let mut budget = ledger()?;
    let at = Location::new("measurement control");
    budget.work(7, at)?;
    let (result, report) = trace::observed(4, || {
        budget.with_work_phase("control", "outer", |budget| {
            budget.work(3, at)?;
            budget.with_work_phase("control", "inner", |budget| budget.work(5, at))?;
            budget.work(2, at)
        })?;
        budget.work(11, at)?;
        drop(budget);
        Ok::<_, nrz_bindings::readings::algebra::Fault>(())
    });
    result?;
    assert!(report.complete);
    assert_eq!(report.receipts.len(), 2);
    for operation in ["outer", "inner"] {
        assert_eq!(
            report
                .clocks
                .get(&(String::new(), operation.into(), "control".into()))
                .map(|clock| clock.1),
            Some(1)
        );
    }
    for (key, receipt) in &report.receipts {
        assert_eq!(receipt.exclusive, 5);
        assert_eq!(receipt.inclusive, if key.3 == "outer" { 10 } else { 5 });
    }
    assert_eq!(report.ledgers.len(), 1);
    for ledger in report.ledgers.values() {
        assert_eq!(
            (
                ledger.first,
                ledger.last,
                ledger.classified,
                ledger.remainder
            ),
            (7, 28, 10, 11)
        );
    }
    Ok(())
}

/// Enabled receipts preserve ordered complete result bits and typed refusals.
/// Deterministic charges agree, including every observed native ledger increment.
/// # Errors
/// Returns fixture, preparation or receipt failures.
/// # Panics
/// Panics if observation changes a result, loses coverage or silently overflows.
/// ´claim:matrix:prepared-exponential-measurement-equivalence´
/// ´test:integration:prepared-exponential-measurement-equivalence´
#[test]
fn prepared_exponential_measurement_equivalence() -> TestResult {
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    let (mut enabled, receipt) = run(4, true)?;
    let (mut disabled, _) = run(4, false)?;
    for row in enabled.iter_mut().chain(&mut disabled) {
        row.seconds = 0.0;
    }
    assert_eq!(enabled, disabled);
    for row in &enabled {
        if row.phase.starts_with("native-") {
            assert_eq!(row.work, Some(0));
        }
    }
    let receipt = receipt.ok_or("enabled receipt")?;
    assert!(receipt.complete);
    assert!(receipt.observer.peak_spans < 4096);
    assert!(receipt.clocks.contains_key(&(
        "native-exp-reuse".into(),
        "nrz.validation".into(),
        "prepared-exp".into()
    )));
    assert!(receipt.clocks.contains_key(&(
        "admit-expm1".into(),
        "nrz.import".into(),
        "prepared-expm1-coefficients".into()
    )));
    assert_eq!(
        receipt
            .counters
            .get(&("prepare-warm".into(), "cache hit".into())),
        Some(&1)
    );
    assert_eq!(
        receipt
            .counters
            .get(&("prepare-cold".into(), "cache miss".into())),
        Some(&1)
    );
    for boundary in ["native-exp-reuse", "native-expm1-reuse"] {
        assert!(!receipt.counters.keys().any(|key| key.0 == boundary));
        assert!(
            receipt
                .boundary_work
                .iter()
                .all(|(key, work)| key.0 != boundary || *work == 0)
        );
    }
    for ledger in receipt.ledgers.values() {
        assert_eq!(
            ledger.last - ledger.first,
            ledger.classified + ledger.remainder
        );
    }
    let (outcome, lost) = trace::observed(0, || {
        let _span =
            tracing::trace_span!("nrz.measure", family = "capacity", revision = 1_u64).entered();
        7
    });
    assert_eq!(outcome, 7);
    assert!(!lost.complete);
    Ok(())
}

/// Default allowances admit the maximum through printed rows; the next size cannot allocate.
/// Permuted inputs are distinct and nonmonotone throughout the tested populations.
/// # Errors
/// Returns an invalid population error.
/// # Panics
/// Panics if an over-limit population or duplicate point is admitted.
/// ´claim:matrix:prepared-exponential-measurement-inputs´
/// ´test:integration:prepared-exponential-measurement-inputs´
#[test]
fn prepared_exponential_measurement_inputs() -> TestResult {
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    for count in [0, 3, 5, 4097, 8192, u32::MAX] {
        assert!(inputs(count).is_err());
    }
    for count in [4097, 8192] {
        assert!(admitted_inputs(count, |_| panic!("over-limit population allocated")).is_err());
    }
    for count in [4, 16, 256, 4096] {
        let values = inputs(count)?;
        let distinct: std::collections::BTreeSet<_> = values.iter().map(|v| v.to_bits()).collect();
        assert_eq!(distinct.len(), count as usize);
        assert!(values.windows(2).any(|v| v[0] < v[1]));
        assert!(values.windows(2).any(|v| v[0] > v[1]));
    }
    let mut printed = Vec::new();
    write_run(4096, false, &mut printed)?;
    let printed = String::from_utf8(printed)?;
    assert!(printed.starts_with("owner=prepared-exponentials cells=4096 "));
    assert_eq!(
        printed
            .lines()
            .filter(|line| line.starts_with("phase="))
            .count(),
        19
    );
    assert!(printed.contains("phase=batch-exp work=Some(4097)"));
    assert!(printed.contains("phase=uncertified-expm1 "));
    let mut refused = Vec::new();
    assert!(write_run(4097, false, &mut refused).is_err());
    assert_eq!(refused, []);
    Ok(())
}

/// Native boundary deltas include dropped private ledgers and exclude prior consumption.
/// # Errors
/// Returns fixture, budget or lock failures.
/// # Panics
/// Panics if private lifecycle Work disappears or a prior prefix is attributed to the boundary.
/// ´claim:matrix:prepared-exponential-measurement-native-work´
/// ´test:integration:prepared-exponential-measurement-native-work´
#[test]
fn prepared_exponential_measurement_native_work() -> TestResult {
    use nrz_bindings::readings::algebra::Location;
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    let mut budget = ledger()?;
    let at = Location::new("native Work control");
    budget.work(7, at)?;
    let (outcome, receipt) = trace::observed(32, || -> TestResult {
        // Establish the excluded prefix before native entry without adding Work.
        budget.with_work_phase("control", "prefix anchor", |_| {
            Ok::<_, nrz_bindings::readings::algebra::Fault>(())
        })?;
        {
            let _span =
                tracing::trace_span!("nrz.measure", family = "native-control", revision = 1_u64)
                    .entered();
            budget.with_work_phase("control", "native charge", |budget| budget.work(2, at))?;
            let mut private = ledger()?;
            private.work(1, at)?;
            drop(private);
        }
        drop(budget);
        Ok(())
    });
    outcome?;
    assert!(receipt.complete);
    let mut amounts: Vec<_> = receipt
        .boundary_work
        .iter()
        .map(|(key, work)| {
            assert_eq!(key.0, "native-control");
            *work
        })
        .collect();
    amounts.sort_unstable();
    assert_eq!(amounts, [1, 2]);
    Ok(())
}

/// Positive Work across an unsnapshotted measurement edge is ambiguous, even outside on both sides.
/// Idle ledgers crossing the same edges remain complete; raw Work still reconciles on refusal.
/// # Errors
/// Returns fixture, budget or lock failures.
/// # Panics
/// Panics if crossing consumption is accepted or idle residence is rejected.
/// ´claim:matrix:prepared-exponential-measurement-crossing-work´
/// ´test:integration:prepared-exponential-measurement-crossing-work´
#[test]
fn prepared_exponential_measurement_crossing_work() -> TestResult {
    use nrz_bindings::readings::algebra::Location;
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    for charge in [1, 0] {
        let (outcome, receipt) = trace::observed(32, || -> TestResult {
            let mut private = ledger()?;
            timed("native-expm1-reuse", || {
                private.work(charge, Location::new("crossing native Work"))?;
                Ok(())
            })?;
            drop(private);
            Ok(())
        });
        outcome?;
        assert_eq!(receipt.complete, charge == 0);
        assert_eq!(receipt.ledgers.len(), 1);
        for ledger in receipt.ledgers.values() {
            assert_eq!(
                (ledger.first, ledger.last, ledger.remainder),
                (0, charge, charge)
            );
            assert!(ledger.created && ledger.closed);
        }
    }
    Ok(())
}

/// Every admitted population completes with equal complete results in both observation modes.
/// # Errors
/// Returns fixture, admission or incomplete-receipt failures.
/// # Panics
/// Panics if observation changes a result or honest ledger lifetimes lose coverage.
/// ´claim:matrix:prepared-exponential-measurement-populations´
/// ´test:integration:prepared-exponential-measurement-populations´
#[test]
fn prepared_exponential_measurement_populations() -> TestResult {
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    for count in [256, 4, 4096, 16, 1024, 8, 512, 64, 2048, 32, 128] {
        let (mut enabled, receipt) = run(count, true)?;
        let (mut disabled, _) = run(count, false)?;
        assert!(receipt.ok_or("enabled receipt")?.complete);
        assert_eq!(enabled.len(), 19);
        for row in enabled.iter_mut().chain(&mut disabled) {
            row.seconds = 0.0;
        }
        assert_eq!(enabled, disabled);
    }
    Ok(())
}

/// A nonzero first snapshot cannot exclude an unknown prefix after native entry.
/// Inside and later closes both refuse; zero first values remain valid anchors.
/// # Errors
/// Returns fixture, budget or lock failures.
/// # Panics
/// Panics if unanchored Work is accepted or an idle first snapshot is rejected.
/// ´claim:matrix:prepared-exponential-measurement-anchoring´
/// ´test:integration:prepared-exponential-measurement-anchoring´
#[test]
fn prepared_exponential_measurement_anchoring() -> TestResult {
    use nrz_bindings::readings::algebra::Location;
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    for close_inside in [false, true] {
        for charge in [1, 0] {
            let private = ledger()?;
            let (outcome, receipt) = trace::observed(32, || -> TestResult {
                let (remaining, _) = timed("native-expm1-reuse", || {
                    let mut private = private;
                    private.work(charge, Location::new("unanchored native Work"))?;
                    if close_inside {
                        drop(private);
                        Ok(None)
                    } else {
                        Ok(Some(private))
                    }
                })?;
                drop(remaining);
                Ok(())
            });
            outcome?;
            assert_eq!(receipt.complete, charge == 0);
            assert_eq!(receipt.ledgers.len(), 1);
            for ledger in receipt.ledgers.values() {
                assert_eq!((ledger.first, ledger.last), (charge, charge));
                assert!(!ledger.created && ledger.closed);
            }
        }
    }
    Ok(())
}

/// A measurement inside a native boundary is unsupported even with zero Work.
/// # Errors
/// Returns fixture, budget or lock failures.
/// # Panics
/// Panics if a nested measurement can hide consumption from the native boundary.
/// ´claim:matrix:prepared-exponential-measurement-nesting´
/// ´test:integration:prepared-exponential-measurement-nesting´
#[test]
fn prepared_exponential_measurement_nesting() -> TestResult {
    use nrz_bindings::readings::algebra::Location;
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    for charge in [0, 1] {
        let (outcome, receipt) = trace::observed(32, || -> TestResult {
            timed("native-expm1-reuse", || {
                timed("nested-control", || {
                    let mut private = ledger()?;
                    private.work(charge, Location::new("nested native Work"))?;
                    drop(private);
                    Ok(())
                })?;
                Ok(())
            })?;
            Ok(())
        });
        outcome?;
        assert!(!receipt.complete);
        assert_eq!(receipt.ledgers.len(), 1);
        for ledger in receipt.ledgers.values() {
            assert_eq!((ledger.first, ledger.last), (0, charge));
            assert!(ledger.created && ledger.closed);
        }
    }
    Ok(())
}

/// A surviving ledger needs a terminal snapshot, not merely an observed prefix.
/// An idle terminal anchor completes coverage; charged anchors expose epoch-crossing Work.
/// # Errors
/// Returns fixture, budget or lock failures.
/// # Panics
/// Panics if hidden surviving Work or missing terminal coverage is accepted.
/// ´claim:matrix:prepared-exponential-measurement-terminal´
/// ´test:integration:prepared-exponential-measurement-terminal´
#[test]
fn prepared_exponential_measurement_terminal() -> TestResult {
    use nrz_bindings::readings::algebra::{Fault, Location};
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    for terminal in [false, true] {
        for charge in [1, 0] {
            let mut private = ledger()?;
            let (outcome, receipt) =
                trace::observed(32, || -> TestResult {
                    private.with_work_phase("control", "prefix anchor", |_| Ok::<_, Fault>(()))?;
                    timed("native-exp-reuse", || {
                        private.work(charge, Location::new("late native Work"))?;
                        Ok(())
                    })?;
                    if terminal {
                        private.with_work_phase("control", "terminal anchor", |_| {
                            Ok::<_, Fault>(())
                        })?;
                    }
                    Ok(())
                });
            outcome?;
            assert_eq!(receipt.complete, terminal && charge == 0);
            assert_eq!(receipt.ledgers.len(), 1);
            for ledger in receipt.ledgers.values() {
                assert!(!ledger.created && !ledger.closed);
                assert_eq!(ledger.first, 0);
                assert_eq!(ledger.last, if terminal { charge } else { 0 });
            }
            drop(private);
        }
    }
    Ok(())
}

/// Terminal coverage is measured against the last native exit, including repeated names.
/// # Errors
/// Returns fixture, budget or lock failures.
/// # Panics
/// Panics if an anchor between two native boundaries certifies the later boundary.
/// ´claim:matrix:prepared-exponential-measurement-last-exit´
/// ´test:integration:prepared-exponential-measurement-last-exit´
#[test]
fn prepared_exponential_measurement_last_exit() -> TestResult {
    use nrz_bindings::readings::algebra::Fault;
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    for terminal in [true, false] {
        let mut private = ledger()?;
        let (outcome, receipt) = trace::observed(32, || -> TestResult {
            private.with_work_phase("control", "prefix anchor", |_| Ok::<_, Fault>(()))?;
            timed("native-exp-reuse", || Ok(()))?;
            private.with_work_phase("control", "between native exits", |_| Ok::<_, Fault>(()))?;
            timed("native-exp-reuse", || Ok(()))?;
            if terminal {
                private.with_work_phase("control", "terminal anchor", |_| Ok::<_, Fault>(()))?;
            }
            Ok(())
        });
        outcome?;
        assert_eq!(receipt.complete, terminal);
        assert_eq!(receipt.ledgers.len(), 1);
        for ledger in receipt.ledgers.values() {
            assert_eq!((ledger.first, ledger.last), (0, 0));
            assert!(!ledger.created && !ledger.closed);
        }
        drop(private);
    }
    Ok(())
}
