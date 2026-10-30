// SPDX-License-Identifier: AGPL-3.0-only
//! Literal preparation and typed decision boundaries preserve the archived graph byte for byte.

use super::{
    decision_fixture::ledger,
    lattice_revalidation_live::{enclose, fixture, retired},
};
use crate::binding_testing::TestResult;
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{
        FaultKind, Location,
        lattice::{CheckedLatticeAttempt, LatticeSelectionOutcome, LatticeUndecidedReason},
        limits::BudgetKind,
    },
};

/// Caller Work spent before the operation remains on the same monotone ledger.
fn prior(work: u64) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(17, Location::new("prior live lattice revalidation"))?;
    Ok(budget)
}

/// Warm and post-rollback preparation have literal boundaries with identical archived graph bytes.
/// ´test:integration:lattice-live-attempt-preparation-boundaries´
/// ´claim:matrix:lattice-live-attempt-preparation-boundaries´
#[test]
fn lattice_live_attempt_preparation_boundaries() -> TestResult {
    for (retry, limits) in [(false, [129_235, 129_234]), (true, [146_664, 146_663])] {
        for (position, work) in limits.into_iter().enumerate() {
            preparation(retry, work, position == 0)?;
        }
    }
    Ok(())
}

/// Each twin owns a fresh store; the retry subject explicitly includes one earlier rollback.
fn preparation(retry: bool, work: u64, accepted: bool) -> TestResult {
    let fixture = fixture(1, true)?;
    let enclosed = enclose(&fixture, &mut ledger()?)?;
    let mut cas = fixture.archived.cas()?;
    if retry {
        let attempt = cas.prepare_lattice_selection(
            &fixture.cover,
            &fixture.expected,
            &fixture.evidence,
            &mut ledger()?,
        )?;
        let LatticeSelectionOutcome::Undecided(outcome) =
            fixture
                .heap
                .select_lattice_attempt(&enclosed, &mut cas, &attempt, &mut ledger()?)?
        else {
            panic!("the earlier widened attempt must be undecided");
        };
        assert_eq!(outcome.reason, LatticeUndecidedReason::BoundaryIntersection);
    }
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = prior(work)?;
    let limits = budget.limits().clone();
    let result = cas.prepare_lattice_selection(
        &fixture.cover,
        &fixture.expected,
        &fixture.evidence,
        &mut budget,
    );
    if accepted {
        let attempt = result?;
        assert_eq!(attempt.spent_work(), work);
        assert_eq!(budget.usage().work, work);
    } else {
        let Err(fault) = result else {
            panic!("one-below preparation Work must refuse the complete receipt");
        };
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert!(budget.usage().work >= 17);
        assert!(budget.usage().work <= work);
    }
    assert_eq!(budget.limits(), &limits);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    drop(cas);
    retired(&fixture)?;
    Ok(())
}

/// The last owner reached by one complete caller ledger.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Boundary {
    /// The complete fresh native capture and original-box containment.
    Containment,
    /// Complete valid mathematical question and enclosure preparation.
    Preparation,
    /// Typed CAS cut outcome after live question, box and lease checking.
    Decision,
}

/// Every cumulative owner has one literal success and one-below twin in a fresh store.
/// ´test:integration:lattice-live-attempt-work-boundaries´
/// ´claim:matrix:lattice-live-attempt-work-boundaries´
#[test]
fn lattice_live_attempt_work_boundaries() -> TestResult {
    for (owner, limits) in [
        (Boundary::Containment, [27_652, 27_651]),
        (Boundary::Preparation, [156_870, 156_869]),
        (Boundary::Decision, [328_431, 328_430]),
    ] {
        for (position, work) in limits.into_iter().enumerate() {
            boundary(owner, work, position == 0)?;
        }
    }
    Ok(())
}

/// Stop only at the named owner's boundary and compare the whole original graph on either result.
fn boundary(owner: Boundary, work: u64, accepted: bool) -> TestResult {
    let fixture = fixture(1, true)?;
    let before = fixture.archived.cas()?.canonical_snapshot(&mut ledger()?)?;
    let heap_records = fixture.heap.provenance_usage();
    let mut budget = prior(work)?;
    let limits = budget.limits().clone();
    let result = enclose(&fixture, &mut budget);
    if owner == Boundary::Containment {
        if accepted {
            drop(result?);
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(fault) = result else {
                panic!("one-below containment Work must refuse the complete native enclosure");
            };
            let fault = fault.downcast::<nrz_matrix::LiveDecisionError>()?;
            let nrz_matrix::LiveDecisionError::Algebra(fault) = *fault else {
                panic!("short containment Work must refuse at the enclosure checker");
            };
            exhausted(&fault.kind);
        }
    } else {
        let enclosed = result?;
        let mut cas = fixture.archived.cas()?;
        let result = cas.prepare_lattice_selection(
            &fixture.cover,
            &fixture.expected,
            &fixture.evidence,
            &mut budget,
        );
        if owner == Boundary::Preparation {
            if accepted {
                assert_eq!(result?.spent_work(), work);
            } else {
                let Err(fault) = result else {
                    panic!(
                        "one-below cumulative preparation Work must refuse the complete receipt"
                    );
                };
                exhausted(&fault.kind);
            }
        } else {
            let attempt = result?;
            let outcome =
                fixture
                    .heap
                    .select_lattice_attempt(&enclosed, &mut cas, &attempt, &mut budget)?;
            check_outcome(&outcome, &attempt, &budget, accepted);
        }
        drop(cas);
    }
    assert!(budget.usage().work >= 17);
    assert!(budget.usage().work <= work);
    assert_eq!(budget.limits(), &limits);
    assert_eq!(
        fixture.archived.cas()?.canonical_snapshot(&mut ledger()?)?,
        before
    );
    assert_eq!(fixture.heap.provenance_usage(), heap_records);
    retired(&fixture)?;
    Ok(())
}

/// Only caller Work exhaustion satisfies these one-below refusal tests.
fn exhausted(kind: &FaultKind) {
    assert!(matches!(
        kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
}

/// Budget undecided borrows the identical complete receipt without allocating selected state.
fn check_outcome(
    value: &LatticeSelectionOutcome<'_>,
    attempt: &CheckedLatticeAttempt,
    budget: &Budget,
    accepted: bool,
) {
    let LatticeSelectionOutcome::Undecided(outcome) = value else {
        panic!("neither boundary outcome may select a widened tuple");
    };
    assert!(std::ptr::eq(outcome.evidence, attempt));
    assert_eq!(outcome.spent_work, budget.usage().work);
    if accepted {
        assert_eq!(outcome.reason, LatticeUndecidedReason::BoundaryIntersection);
        assert_eq!(budget.usage().work, budget.limits().work);
    } else {
        assert_eq!(
            outcome.reason,
            LatticeUndecidedReason::DecisionBudgetExhausted
        );
        exhausted(&outcome.fault.kind);
    }
}
