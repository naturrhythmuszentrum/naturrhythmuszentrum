// SPDX-License-Identifier: AGPL-3.0-only
//! Independent projected-forest, lifecycle, refusal and disabled-observer controls.

use crate::budget::{Budget, Limits};
use crate::error::{Fault, Location};
use crate::testing::common::TestResult;
use crate::testing::work_trace::{Report, observed};
use crate::work_observation::{Phase, Scope, finish, observe};

/// Lookup by operation after checking the report is complete.
fn receipt(report: &Report, operation: &str) -> (u64, u64, u64) {
    assert!(report.complete);
    let mut total = (0_u64, 0_u64, 0_u64);
    for ((_, _, _, name), value) in &report.receipts {
        if name == operation {
            total.0 += value.inclusive;
            total.1 += value.exclusive;
            total.2 += value.intervals;
        }
    }
    total
}

/// Intervening foreign-ledger spans do not conceal projected same-ledger children.
/// ´test:crate:work-tracing-projected-ledgers´
#[test]
fn work_tracing_projected_ledgers() -> TestResult {
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut left = Budget::new(Limits::default())?;
        let mut middle = Budget::new(Limits::default())?;
        observe!(left, Location::new("A"), {
            left.work(2, Location::new("A"))?;
            observe!(middle, Location::new("B"), {
                middle.work(7, Location::new("B"))?;
                observe!(left, Location::new("C"), {
                    left.work(5, Location::new("C"))
                })
            })?;
            left.work(3, Location::new("A"))
        })
    });
    result?;
    assert_eq!(receipt(&report, "A"), (10, 5, 1));
    assert_eq!(receipt(&report, "B"), (7, 7, 1));
    assert_eq!(receipt(&report, "C"), (5, 5, 1));
    let mut totals: Vec<_> = report
        .ledgers
        .values()
        .map(|ledger| {
            (
                ledger.last - ledger.first,
                ledger.classified,
                ledger.remainder,
            )
        })
        .collect();
    totals.sort_unstable();
    assert_eq!(totals, [(7, 7, 0), (10, 10, 0)]);
    Ok(())
}

/// Ledgerless wrappers, deep descendants and disjoint siblings leave explicit root remainder.
/// ´test:crate:work-tracing-ledgerless-remainder´
#[test]
fn work_tracing_ledgerless_remainder() -> TestResult {
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        observe!(ledger, Location::new(""), {
            ledger.work(4, Location::new("unclassified"))?;
            observe!(ledger, Location::new("A"), {
                ledger.work(2, Location::new("A"))?;
                {
                    let _wrapper = Scope::phase("wrapper");
                    observe!(ledger, Location::new("C"), {
                        ledger.work(3, Location::new("C"))?;
                        observe!(ledger, Location::new("D"), {
                            ledger.work(2, Location::new("D"))
                        })
                    })?;
                }
                observe!(ledger, Location::new("E"), {
                    ledger.work(1, Location::new("E"))
                })
            })
        })
    });
    result?;
    assert_eq!(receipt(&report, "A"), (8, 2, 1));
    assert_eq!(receipt(&report, "C"), (5, 3, 1));
    assert_eq!(receipt(&report, "D"), (2, 2, 1));
    assert_eq!(receipt(&report, "E"), (1, 1, 1));
    let ledger = report.ledgers.values().next().ok_or("ledger")?;
    assert_eq!(
        (ledger.last, ledger.classified, ledger.remainder),
        (12, 8, 4)
    );
    assert_eq!(report.rows.iter().map(|row| row.work).sum::<u64>(), 8);
    Ok(())
}

/// A purpose-bearing interval is classified even without a helper location.
#[test]
fn work_tracing_operationless_phase() -> TestResult {
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        ledger.work(1, Location::new("root"))?;
        let _rule = Scope::rule(|| "phase.rule");
        let _purpose = Scope::phase("proof");
        observe!(ledger, Location::new(""), {
            ledger.work(3, Location::new("phase work"))?;
            observe!(ledger, Location::new("child"), {
                ledger.work(2, Location::new("child"))
            })
        })
    });
    result?;
    let ledger = report.ledgers.values().next().ok_or("ledger")?;
    assert_eq!((ledger.classified, ledger.remainder), (5, 1));
    assert_eq!(
        report
            .rows
            .iter()
            .find(|row| row.rule == "phase.rule" && row.location == "phase boundary")
            .map(|row| row.work),
        Some(3)
    );
    Ok(())
}

/// Suspension closes one interval and resume projects a new parent without absorbing the gap.
/// ´test:crate:work-tracing-suspension´
#[test]
fn work_tracing_suspension() -> TestResult {
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        let phase = Phase::new(&ledger, || "resumed");
        let entered = phase.enter(&ledger);
        let result = ledger.work(2, Location::new("first"));
        finish(entered, &ledger, &result);
        result?;
        observe!(ledger, Location::new("gap"), {
            ledger.work(7, Location::new("gap"))
        })?;
        let ledger = &mut ledger;
        observe!(ledger, Location::new("new parent"), {
            let entered = phase.enter(ledger);
            let result = ledger.work(3, Location::new("second"));
            finish(entered, ledger, &result);
            result
        })
    });
    result?;
    assert_eq!(receipt(&report, "resumed"), (5, 5, 2));
    assert_eq!(receipt(&report, "gap"), (7, 7, 1));
    assert_eq!(receipt(&report, "new parent"), (3, 0, 1));
    let ledger = report.ledgers.values().next().ok_or("ledger")?;
    assert_eq!(
        (ledger.last, ledger.classified, ledger.remainder),
        (12, 12, 0)
    );
    Ok(())
}

/// The same ordinary helper returns exactly the same fault and counters under either dispatch.
fn attempt() -> Result<(Result<(), Fault>, crate::budget::BudgetUsage), Fault> {
    let mut ledger = Budget::new(Limits {
        work: 5,
        ..Limits::default()
    })?;
    ledger.work(3, Location::new("prior"))?;
    let result = observe!(ledger, Location::new("refused"), {
        ledger.work(2, Location::new("prefix"))?;
        ledger.work(1, Location::new("rejected"))
    });
    Ok((result, ledger.usage()))
}

/// Rejected charges remain absent, while refused attempts retain their charged prefix.
/// ´test:crate:work-tracing-refusal-equivalence´
#[test]
fn work_tracing_refusal_equivalence() -> TestResult {
    let disabled = crate::testing::work_trace::without_observation(attempt)?;
    let (enabled, report) = observed(64, attempt);
    assert_eq!(enabled?, disabled);
    assert!(disabled.0.is_err());
    assert_eq!(disabled.1.work, 5);
    assert_eq!(receipt(&report, "refused"), (2, 2, 1));
    assert_eq!(
        report
            .receipts
            .values()
            .map(|value| value.refused)
            .sum::<u64>(),
        1
    );
    let ledger = report.ledgers.values().next().ok_or("ledger")?;
    assert_eq!((ledger.classified, ledger.remainder), (2, 3));
    Ok(())
}

/// Private, copied-identity and recreated ledgers never alias equal counters or reused storage.
/// ´test:crate:work-tracing-ledger-incarnations´
#[test]
fn work_tracing_ledger_incarnations() -> TestResult {
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        let copy = ledger.observation.clone();
        assert_ne!(ledger.observation.get(), copy.get());
        ledger.work(3, Location::new("private"))?;
        let original = ledger.observation.get();
        ledger = Budget::new(ledger.limits().clone())?;
        assert_ne!(original, ledger.observation.get());
        ledger.work(3, Location::new("replacement"))
    });
    result?;
    assert!(report.complete);
    assert_eq!(report.ledgers.len(), 2);
    for ledger in report.ledgers.values() {
        assert_eq!(
            (ledger.last, ledger.classified, ledger.remainder),
            (3, 0, 3)
        );
    }
    Ok(())
}

/// A transfer relates two reconciled ledgers and never manufactures a second parent debit.
/// ´test:crate:work-tracing-transfer-reconciliation´
#[test]
fn work_tracing_transfer_reconciliation() -> TestResult {
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut parent = Budget::new(Limits::default())?;
        let mut child = Budget::new(Limits::default())?;
        observe!(parent, Location::new("parent"), {
            parent.work(3, Location::new("parent"))?;
            observe!(child, Location::new("child"), {
                child.work(7, Location::new("child"))
            })?;
            let before = parent.usage().work;
            parent.work(child.usage().work, Location::new("existing debit"))?;
            tracing::event!(name: "nrz.work.transfer", target: "nrz::work", tracing::Level::TRACE,
                owner = "nrz-cas", child = child.observation.get(), parent = parent.observation.get(),
                before, after = parent.usage().work, work = child.usage().work);
            Ok(())
        })
    });
    result?;
    assert_eq!(receipt(&report, "parent"), (10, 10, 1));
    assert_eq!(receipt(&report, "child"), (7, 7, 1));
    assert_eq!(report.transfers.len(), 1);
    assert_eq!(report.transfers.first().ok_or("transfer")?.work, 7);
    assert_eq!(
        report
            .ledgers
            .values()
            .map(|value| value.remainder)
            .sum::<u64>(),
        0
    );
    Ok(())
}

/// Missing exit snapshots and bounded observer loss reject evidence while arithmetic succeeds.
/// ´test:crate:work-tracing-observer-loss´
#[test]
fn work_tracing_observer_loss() -> TestResult {
    let (result, report) = observed(0, attempt);
    assert_eq!(result?.1.work, 5);
    assert!(!report.complete);
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        let phase = Phase::new(&ledger, || "missing exit");
        let _entered = phase.enter(&ledger);
        ledger.work(5, Location::new("ordinary work"))
    });
    result?;
    assert!(!report.complete);
    Ok(())
}

/// Replacing the foreign-ledger middle span with a ledgerless wrapper keeps the projection.
/// ´test:crate:work-tracing-ledgerless-projection´
#[test]
fn work_tracing_ledgerless_projection() -> TestResult {
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        observe!(ledger, Location::new("A"), {
            ledger.work(2, Location::new("A"))?;
            {
                let _wrapper = Scope::phase("wrapper");
                observe!(ledger, Location::new("C"), {
                    ledger.work(5, Location::new("C"))
                })?;
            }
            ledger.work(3, Location::new("A"))
        })
    });
    result?;
    assert_eq!(receipt(&report, "A"), (10, 5, 1));
    assert_eq!(receipt(&report, "C"), (5, 5, 1));
    let ledger = report.ledgers.values().next().ok_or("ledger")?;
    assert_eq!(
        (ledger.last, ledger.classified, ledger.remainder),
        (10, 10, 0)
    );
    Ok(())
}

/// Impossible subtraction and reused incarnation identities are incomplete evidence.
/// ´test:crate:work-tracing-invalid-boundaries´
#[test]
fn work_tracing_invalid_boundaries() -> TestResult {
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        let phase = Phase::new(&ledger, || "invalid parent");
        let entered = phase.enter(&ledger);
        observe!(ledger, Location::new("child"), {
            ledger.work(5, Location::new("child"))
        })?;
        entered.record("work", 0_u64);
        entered.record("outcome", "completed");
        Ok(())
    });
    result?;
    assert!(!report.complete);
    let (result, report) = observed(64, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        crate::work_observation::ledger(&ledger, "created");
        ledger.work(5, Location::new("ordinary work"))
    });
    result?;
    assert!(!report.complete);
    Ok(())
}

/// Longer runs retain bounded aggregates; overlong fields lose evidence, never arithmetic.
/// ´test:crate:work-tracing-bounded-collection´
#[test]
fn work_tracing_bounded_collection() -> TestResult {
    use std::io::Write;

    for count in [8, 128] {
        let (result, report) = observed(8, || -> Result<(), Fault> {
            let mut ledger = Budget::new(Limits::default())?;
            for _ in 0..count {
                observe!(ledger, Location::new("repeated"), {
                    ledger.work(1, Location::new("repeated"))
                })?;
            }
            Ok(())
        });
        result?;
        assert_eq!(receipt(&report, "repeated"), (count, count, count));
        assert_eq!(report.receipts.len(), 1);
        assert_eq!(report.ledgers.len(), 1);
        assert_eq!(report.observer.capacity, 8);
        assert_eq!(report.observer.peak_spans, 1);
        assert_eq!(report.observer.peak_depth, 1);
        assert!(report.observer.callbacks > count);
        writeln!(
            std::io::stdout().lock(),
            "observer-cost count={count} {:?}",
            report.observer
        )?;
    }
    let (result, report) = observed(8, || -> Result<(), Fault> {
        let mut ledger = Budget::new(Limits::default())?;
        let operation = "x".repeat(513);
        let span = tracing::trace_span!(target: "nrz::work", "nrz.work.phase",
            owner = "nrz-cas", operation = operation.as_str(),
            ledger = ledger.observation.get(), work = ledger.usage().work,
            outcome = tracing::field::Empty);
        let entered = span.entered();
        let result = ledger.work(1, Location::new("ordinary work"));
        finish(entered, &ledger, &result);
        result
    });
    result?;
    assert!(!report.complete);
    Ok(())
}
