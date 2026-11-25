// SPDX-License-Identifier: AGPL-3.0-only
//! Exact graph ownership boundaries preserve all registrations and caller prefixes.

use super::{BudgetKind, Fault, FaultKind, Fixture, Limits, TestResult, ledger, retained};
use crate::budget::Budget;
use crate::{Exhaustion, Location};

/// A fresh ledger includes every graph judgment without a registration discount.
/// ´test:unit:retained-graph-fresh´
#[test]
fn retained_graph_fresh() -> TestResult {
    boundary(false, false, 0)
}

/// An existing registration supplies judgments once and retains its metadata.
/// ´test:unit:retained-graph-registered´
#[test]
fn retained_graph_registered() -> TestResult {
    boundary(true, false, 0)
}

/// An unrelated owner cannot discount this graph or lose its own reservation.
/// ´test:unit:retained-graph-unrelated´
#[test]
fn retained_graph_unrelated() -> TestResult {
    boundary(false, true, 0)?;
    boundary(true, true, 0)
}

/// Prior caller bytes compose and both successful and error callbacks restore limits.
/// ´test:unit:retained-graph-caller´
#[test]
fn retained_graph_caller() -> TestResult {
    boundary(false, false, 31)?;
    boundary(true, true, 31)
}

/// The fixture has J=4,600 judgment bytes per graph and 64 per registration.
/// G is the complete graph inventory before the wrapper runs; the exact live
/// peak is G + 64 when registered, plus 4,664 for an unrelated owner, plus P.
/// The callback's zero scratch fits; one scratch byte must fail at that peak.
/// These expectations never use the wrapper's computed additional reservation.
/// # Panics
/// Panics if an independently derived boundary or callback lifetime is violated.
fn boundary(registered: bool, unrelated: bool, prefix: u64) -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.retain_graph(&mut ledger()?)?;
    let mut other = Fixture::new()?;
    other.retain_graph(&mut ledger()?)?;
    assert_eq!(fixture.cas.retained_judgment_bytes(), 4_600);
    let graph = fixture.cas.counts().payload_bytes;
    let registrations = u64::from(registered) * 4_664 + u64::from(unrelated) * 4_664;
    let peak = graph + u64::from(registered) * 64 + u64::from(unrelated) * 4_664;
    for short in [0, 1] {
        for callback_error in [false, true] {
            let limit = peak + prefix - short;
            let mut budget = Budget::new(Limits {
                staged_payload_bytes: limit,
                work: 100,
                ..Limits::default()
            })?;
            if unrelated {
                retained(&other.cas, &mut budget, |_| Ok(()))?;
            }
            if registered {
                retained(&fixture.cas, &mut budget, |_| Ok(()))?;
            }
            budget.work(7, super::super::AT)?;
            let mut entered = false;
            let result: Result<(), Fault> =
                budget.with_retained_payload(prefix, super::super::AT, |budget| {
                    let result = fixture.cas.with_retained_graph(budget, |budget| {
                        entered = true;
                        budget.work(3, super::super::AT)?;
                        let fault = budget
                            .check(BudgetKind::StagedPayloadBytes, 1, super::super::AT)
                            .expect_err("one byte exceeds the independently derived peak");
                        assert_eq!(
                            fault.kind,
                            FaultKind::Exhausted {
                                reason: Exhaustion::Limit,
                                budget: BudgetKind::StagedPayloadBytes,
                                limit: registrations,
                                needed: registrations + 1,
                            }
                        );
                        if callback_error {
                            return budget.work(100, super::super::AT);
                        }
                        Ok(())
                    });
                    assert_eq!(budget.limits().staged_payload_bytes, limit - prefix);
                    result
                });
            let visits = u64::from(unrelated) + u64::from(registered);
            if short == 1 {
                assert!(!entered);
                let fault = result.expect_err("one-below graph footprint");
                assert_eq!(fault.location, Location::new("retained graph"));
                assert_eq!(
                    fault.kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::StagedPayloadBytes,
                        limit: peak - 1,
                        needed: peak,
                    }
                );
            } else if callback_error {
                assert_eq!(
                    result.expect_err("callback Work refusal").kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::Work,
                        limit: 100,
                        needed: 110 + visits,
                    }
                );
            } else {
                result?;
            }
            assert_eq!(entered, short == 0);
            assert_eq!(budget.usage().work, 7 + visits + u64::from(entered) * 3);
            assert_eq!(budget.checked_records.bytes, registrations);
            assert_eq!(budget.limits().staged_payload_bytes, limit);
        }
    }
    Ok(())
}
