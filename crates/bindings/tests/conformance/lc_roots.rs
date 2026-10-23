// SPDX-License-Identifier: AGPL-3.0-only
//! Original source identities and cumulative replay ceilings are mandatory engine evidence.

use crate::testing::lc_roots::{TestResult, changed, ledger, mutations, original, proof, proposal};
use nrz_bindings::readings::algebra::{
    FaultKind, Location,
    limits::{Budget, BudgetKind, Limits},
};
use nrz_bindings::readings::{ReadingError, RootCoverage, RootProblem};

/// Missing either sign and either sibling stratum cannot inherit positive-family coverage.
/// ´test:integration:lc-root-coverage-refuses-changed-signs´
#[test]
fn lc_root_coverage_refuses_changed_signs() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    for signs in [[0, 1], [1, 0], [-1, 1], [1, -1]] {
        let (mut fixture, request) = original(signs, &mut ledger()?)?;
        let set = proposal(&mut fixture, &theorem)?;
        let set = mutations::rebind(&mut fixture.cas, &request, &set, false, &mut ledger()?)?;
        let problem = RootProblem::check(&mut fixture.cas, request, &mut ledger()?)?;
        let before = fixture.cas.counts();
        let refused = RootCoverage::replay(&mut fixture.cas, &problem, set, bytes, &mut ledger()?);
        assert!(matches!(refused, Err(ReadingError::Algebra(_))));
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Deleting, duplicating or permuting original divisor occurrences refuses during request admission.
/// ´test:integration:lc-root-request-retains-every-divisor-occurrence´
#[test]
fn lc_root_request_retains_every_divisor_occurrence() -> TestResult {
    for mutation in 0..3 {
        let (mut fixture, mut request) = original([1, 1], &mut ledger()?)?;
        let mut data = mutations::problem_data(&fixture.cas, &request)?;
        assert_eq!(data.denominators.len(), 4);
        match mutation {
            0 => {
                data.denominators.pop();
            }
            1 => {
                data.denominators.push(data.denominators[0].clone());
            }
            _ => data.denominators.swap(0, 1),
        }
        mutations::replace_problem(&mut fixture.cas, &mut request, data, &mut ledger()?)?;
        let before = fixture.cas.counts();
        let refused = RootProblem::check(&mut fixture.cas, request, &mut ledger()?);
        assert!(matches!(refused, Err(ReadingError::Algebra(_))));
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// An omitted root changes the expected set, even when the original equation and signs are unchanged.
/// ´test:integration:lc-root-coverage-refuses-incomplete-set´
#[test]
fn lc_root_coverage_refuses_incomplete_set() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    let (mut fixture, request) = original([1, 1], &mut ledger()?)?;
    let set = proposal(&mut fixture, &theorem)?;
    let set = mutations::rebind(&mut fixture.cas, &request, &set, true, &mut ledger()?)?;
    let problem = RootProblem::check(&mut fixture.cas, request, &mut ledger()?)?;
    let before = fixture.cas.counts();
    assert!(RootCoverage::replay(&mut fixture.cas, &problem, set, bytes, &mut ledger()?).is_err());
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// One explicit replay allowance and one cumulative caller ceiling succeed exactly at their costs.
/// ´test:integration:lc-root-replay-exact-work-boundaries´
#[test]
fn lc_root_replay_exact_work_boundaries() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    for (work, replay, accepted) in [
        (1_433_784, 1_433_783, true),
        (1_433_783, 1_433_783, false),
        (1_433_784, 1_433_782, false),
    ] {
        let (mut fixture, request) = original([1, 1], &mut ledger()?)?;
        let set = proposal(&mut fixture, &theorem)?;
        let problem = RootProblem::check(&mut fixture.cas, request, &mut ledger()?)?;
        let mut budget = Budget::new(Limits {
            work,
            replay_work: replay,
            ..Limits::default()
        })?;
        let before = fixture.cas.counts();
        let result = RootCoverage::replay(&mut fixture.cas, &problem, set, bytes, &mut budget);
        assert_eq!(result.is_ok(), accepted, "{result:?}");
        if !accepted {
            exhausted(&result, &[BudgetKind::Work, BudgetKind::ReplayWork])?;
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}

/// Requests and failed retries share every prior caller charge; a retry never opens a new ledger.
/// ´test:integration:lc-root-replay-carries-requests-and-retries´
#[test]
fn lc_root_replay_carries_requests_and_retries() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    for ceiling in [2_865_572, 2_865_571] {
        let (mut fixture, request) = original([1, 1], &mut ledger()?)?;
        let set = proposal(&mut fixture, &theorem)?;
        let problem = RootProblem::check(&mut fixture.cas, request, &mut ledger()?)?;
        let mut budget = Budget::new(Limits {
            work: ceiling,
            replay_work: 1_433_783,
            ..Limits::default()
        })?;
        budget.work(17, Location::new("earlier symbolic work"))?;
        let first =
            RootCoverage::replay(&mut fixture.cas, &problem, set.clone(), bytes, &mut budget)?;
        assert_eq!(first.usage().before.work, 17);
        let second = budget.with_retained_payload(
            first.retained_payload_bytes(),
            Location::new("retained earlier coverage"),
            |budget| RootCoverage::replay(&mut fixture.cas, &problem, set.clone(), bytes, budget),
        );
        assert_eq!(second.is_ok(), ceiling == 2_865_572);
        if second.is_err() {
            exhausted(&second, &[BudgetKind::Work])?;
            let spent = budget.usage();
            let retry =
                RootCoverage::replay(&mut fixture.cas, &problem, set.clone(), bytes, &mut budget);
            exhausted(&retry, &[BudgetKind::Work])?;
            assert!(budget.usage().work >= spent.work);
        }
    }
    Ok(())
}

fn exhausted(result: &Result<RootCoverage, ReadingError>, kinds: &[BudgetKind]) -> TestResult {
    let Err(ReadingError::Algebra(fault)) = result else {
        return Err("expected exact resource refusal".into());
    };
    let FaultKind::Exhausted { budget, .. } = fault.kind else {
        return Err(format!("{fault:?}").into());
    };
    assert!(kinds.contains(&budget));
    Ok(())
}

/// A foreign receipt cannot attest to a local occurrence even when both source texts agree.
/// ´test:integration:lc-root-request-refuses-substituted-receipt´
#[test]
fn lc_root_request_refuses_substituted_receipt() -> TestResult {
    let (mut fixture, mut request) = original([1, 1], &mut ledger()?)?;
    let (_, foreign) = original([1, 1], &mut ledger()?)?;
    request.inputs[0].receipt = foreign.inputs[0].receipt.clone();
    let before = fixture.cas.counts();
    assert!(matches!(
        RootProblem::check(&mut fixture.cas, request, &mut ledger()?),
        Err(ReadingError::Definition)
    ));
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Changing either original physical coefficient or hiding an undefined child beneath zero rejects coverage.
/// ´test:integration:lc-root-coverage-retains-original-coefficients-and-zero-children´
#[test]
fn lc_root_coverage_retains_original_coefficients_and_zero_children() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    for (scales, invalid) in [([2, 1], false), ([1, 2], false), ([1, 1], true)] {
        let (mut fixture, request) = changed([1, 1], scales, invalid, &mut ledger()?)?;
        let set = proposal(&mut fixture, &theorem)?;
        let set = mutations::rebind(&mut fixture.cas, &request, &set, false, &mut ledger()?)?;
        let problem = RootProblem::check(&mut fixture.cas, request, &mut ledger()?)?;
        let before = fixture.cas.counts();
        assert!(
            RootCoverage::replay(&mut fixture.cas, &problem, set, bytes, &mut ledger()?).is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// The positive guard tree cannot stand in for its omitted original family projection.
/// ´test:integration:lc-root-coverage-refuses-missing-placement´
#[test]
fn lc_root_coverage_refuses_missing_placement() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    let (mut fixture, mut request) = original([1, 1], &mut ledger()?)?;
    mutations::omit_placement(&mut fixture.cas, &mut request, &mut ledger()?)?;
    let set = proposal(&mut fixture, &theorem)?;
    let problem = RootProblem::check(&mut fixture.cas, request, &mut ledger()?)?;
    let before = fixture.cas.counts();
    assert!(RootCoverage::replay(&mut fixture.cas, &problem, set, bytes, &mut ledger()?).is_err());
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
