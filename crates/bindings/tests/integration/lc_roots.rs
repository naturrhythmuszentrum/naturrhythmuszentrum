// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC coverage is checked against an independently rebuilt bindings request.

use crate::testing::lc_roots::{TestResult, ledger, original, proof, proposal};
use nrz_bindings::readings::algebra::{
    FaultKind, Location,
    domain::HypothesisRef,
    limits::{Budget, BudgetKind, Limits},
};
use nrz_bindings::readings::{ReadingError, RootCoverage, RootProblem};

/// Original L/C guards, four divisor occurrences and source receipts survive complete replay.
/// ´test:integration:lc-original-root-coverage-replays´
#[test]
fn lc_original_root_coverage_replays() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    let mut construction = ledger()?;
    let (mut fixture, request) = original([1, 1], &mut construction)?;
    let root = request.inputs.first().ok_or("LC input")?.root.clone();
    let set = proposal(&mut fixture, &theorem)?;
    let mut admission = ledger()?;
    let problem = RootProblem::check(&mut fixture.cas, request, &mut admission)?;
    let mut budget = ledger()?;
    let coverage = RootCoverage::replay(&mut fixture.cas, &problem, set, bytes, &mut budget)?;
    let usage = coverage.usage();
    assert_eq!(usage.replayed.work - usage.expected.work, 1_464_494);
    assert_eq!(usage.before.work, 0);
    assert_eq!(usage.expected.work, 1);
    assert_eq!(
        coverage.original().request().inputs[0].receipt.root(),
        &root
    );
    assert!(coverage.witness().is_discharged());
    assert_eq!(coverage.witness().hypotheses(), [] as [HypothesisRef; 0]);
    assert_eq!(coverage.export(&fixture.cas, &mut budget)?, *bytes);
    assert_eq!(budget.usage().work, 1_470_635);
    let repeated = RootCoverage::replay(
        &mut fixture.cas,
        &problem,
        coverage.set().clone(),
        bytes,
        &mut budget,
    )?;
    assert_eq!(
        repeated.usage().replayed.work - repeated.usage().expected.work,
        1_462_481
    );
    assert_eq!(
        (
            construction.usage().work,
            admission.usage().work,
            coverage.retained_payload_bytes()
        ),
        (855_980, 4_468, 52_448)
    );
    Ok(())
}

/// An explicit cumulative ceiling refuses a second request and retry after fresh replay succeeds.
/// ´test:integration:lc-root-replay-explicit-cumulative-budget´
#[test]
fn lc_root_replay_explicit_cumulative_budget() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    let (mut fixture, request) = original([1, 1], &mut ledger()?)?;
    let set = proposal(&mut fixture, &theorem)?;
    let problem = RootProblem::check(&mut fixture.cas, request, &mut ledger()?)?;
    let mut budget = Budget::new(Limits {
        work: 2_000_000,
        ..Limits::default()
    })?;
    let first = RootCoverage::replay(&mut fixture.cas, &problem, set.clone(), bytes, &mut budget)?;
    assert_eq!(first.usage().before.work, 0);
    assert_eq!(first.usage().replayed, budget.usage());
    for _ in 0..2 {
        let spent = budget.usage();
        let before = fixture.cas.counts();
        let refused = budget.with_retained_payload(
            first.retained_payload_bytes(),
            Location::new("retained earlier coverage"),
            |budget| RootCoverage::replay(&mut fixture.cas, &problem, set.clone(), bytes, budget),
        );
        let Err(ReadingError::Algebra(fault)) = refused else {
            return Err("expected cumulative replay refusal".into());
        };
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert_eq!(fixture.cas.counts(), before);
        assert!(budget.usage().work >= spent.work);
    }
    assert_eq!(budget.limits().work, 2_000_000);
    Ok(())
}
