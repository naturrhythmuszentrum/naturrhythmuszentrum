// SPDX-License-Identifier: AGPL-3.0-only
//! Original-request and root-coverage payloads compose with caller-retained storage.

use crate::testing::lc_roots::{TestResult, Theorem, ledger, original, proof, proposal};
use nrz_bindings::readings::algebra::{
    Location,
    limits::{Budget, BudgetKind, Limits},
};
use nrz_bindings::readings::{RootCoverage, RootProblem};

/// Admission consumes the same caller ledger as subsequent replay and export, at exact Work boundaries.
/// ´test:integration:lc-root-original-admission-cumulative-boundary´
#[test]
fn lc_root_original_admission_cumulative_boundary() -> TestResult {
    let theorem = proof()?;
    let bytes = &theorem.bytes;
    let (mut fixture, request) = original([1, 1], &mut ledger()?)?;
    let _set = proposal(&mut fixture, &theorem)?;
    let mut measured = ledger()?;
    RootProblem::check(&mut fixture.cas, request.clone(), &mut measured)?;
    let admission = measured.usage().work;
    for ceiling in [admission, admission - 1] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        let before = fixture.cas.counts();
        let result = RootProblem::check(&mut fixture.cas, request.clone(), &mut budget);
        assert_eq!(result.is_ok(), ceiling == admission, "{result:?}");
        if result.is_err() {
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    let required = 1_391_795;
    for ceiling in [required, required - 1] {
        let (mut fixture, request) = original([1, 1], &mut ledger()?)?;
        let set = proposal(&mut fixture, &theorem)?;
        let mut budget = Budget::new(Limits {
            work: ceiling,
            replay_work: 1_381_167,
            ..Limits::default()
        })?;
        budget.work(19, Location::new("prior caller work"))?;
        let problem = RootProblem::check(&mut fixture.cas, request, &mut budget)?;
        let evidence = RootCoverage::replay(&mut fixture.cas, &problem, set, bytes, &mut budget)?;
        assert_eq!(evidence.usage().before.work, 19 + admission);
        let exported = evidence.export(&fixture.cas, &mut budget);
        assert_eq!(exported.is_ok(), ceiling == required);
        if let Ok(exported) = exported {
            assert_eq!(&exported, bytes);
        }
    }
    Ok(())
}

/// Live caller payload reduces the available staged allowance without changing any Work counter.
/// ´test:integration:lc-root-coverage-payload-boundaries´
#[test]
fn lc_root_coverage_payload_boundaries() -> TestResult {
    let theorem = proof()?;
    let mut costs = Vec::new();
    for kind in [BudgetKind::StagedPayloadBytes, BudgetKind::OutputBytes] {
        let attempt = |limit, prior| replay_limit(&theorem, kind, limit, prior);
        let mut lower = 0;
        let mut upper = 4_194_304;
        assert!(attempt(upper, 0)?);
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if attempt(middle, 0)? {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(attempt(upper, 0)?);
        assert!(!attempt(upper - 1, 0)?);
        if kind == BudgetKind::StagedPayloadBytes {
            assert!(attempt(upper + 23, 23)?);
            assert!(!attempt(upper + 22, 23)?);
        }
        costs.push(upper);
    }
    assert_eq!(costs, vec![352_789, 52_448]);
    Ok(())
}

fn replay_limit(theorem: &Theorem, kind: BudgetKind, limit: u64, prior: u64) -> TestResult<bool> {
    let (mut fixture, request) = original([1, 1], &mut ledger()?)?;
    let set = proposal(&mut fixture, theorem)?;
    let problem = RootProblem::check(&mut fixture.cas, request, &mut ledger()?)?;
    let mut limits = Limits {
        work: 1_381_168,
        replay_work: 1_381_167,
        ..Limits::default()
    };
    match kind {
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        _ => limits.output_bytes = limit,
    }
    let mut budget = Budget::new(limits)?;
    let result =
        budget.with_retained_payload(prior, Location::new("prior retained evidence"), |budget| {
            RootCoverage::replay(&mut fixture.cas, &problem, set, &theorem.bytes, budget)
        });
    if let Err(nrz_bindings::readings::ReadingError::Algebra(fault)) = &result {
        assert!(
            matches!(fault.kind, nrz_bindings::readings::algebra::FaultKind::Exhausted { budget, .. } if budget == kind)
        );
    } else if result.is_err() {
        return Err(format!("unexpected payload refusal: {result:?}").into());
    }
    Ok(result.is_ok())
}
