// SPDX-License-Identifier: AGPL-3.0-only
//! Complete vector covariance preserves the existing explicit cumulative caller policy.

use super::fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{Budget, Limits};
use nrz_scalar::ArithmeticContext;

type Profile = ([u64; 4], u64, u64, Vec<[u64; 3]>, u64);

/// Construction and all three complete covariance maps retain measured phases on one caller ledger.
/// ´test:integration:lc-acceptance-vector-covariance-costs-are-pinned´
#[test]
fn lc_acceptance_vector_covariance_costs_are_pinned() -> TestResult {
    assert_eq!(
        complete(crate::phasor_solve_testing::policy(), 0)?,
        (
            [441_084, 1_677_092, 2_657_028, 2_702_485],
            17_092,
            2727,
            vec![[396, 49, 199_286], [408, 49, 355_537], [396, 49, 199_316]],
            2_723_651
        )
    );
    for (work, prior, success) in [
        (2_723_651, 0, true),
        (2_723_650, 0, false),
        (2_723_651, 1, false),
    ] {
        let limits = Limits {
            work,
            ..crate::phasor_solve_testing::policy()
        };
        assert_eq!(complete(limits, prior).is_ok(), success);
    }
    Ok(())
}

fn complete(limits: Limits, prior: u64) -> TestResult<Profile> {
    let mut budget = Budget::new(limits)?;
    budget.work(
        prior,
        nrz_bindings::readings::algebra::Location::new("prior LC covariance work"),
    )?;
    let mut case = fixture::vector_case_with_budget::<f64>(&mut budget)?;
    let construction = budget.usage().work;
    let means = case.means(&mut budget)?;
    let mean_work = budget.usage().work - construction;
    let before = budget.usage().work;
    let executions = case.executions(&means, &mut budget)?;
    let planning = budget.usage().work - before;
    let context = ArithmeticContext::new()?;
    let mut phases = Vec::new();
    for execution in &executions {
        let result = case
            .heap
            .evaluate_companion(execution, &context, &mut budget)?;
        let usage = result.usage();
        phases.push([
            usage.before_evaluation.work - usage.before_lowering.work,
            usage.after.work - usage.before_evaluation.work,
            execution.retained_payload_bytes(),
        ]);
        assert_eq!(
            (result.values(&case.heap, &context)?.len()
                + result.errors(&case.heap, &context)?.len())
                * size_of::<f64>(),
            64
        );
    }
    Ok((
        case.construction,
        mean_work,
        planning,
        phases,
        budget.usage().work,
    ))
}

/// A refused covariance allocation preserves both published mean families and earlier caller charges.
/// ´test:integration:lc-acceptance-vector-covariance-payload-boundaries´
#[test]
fn lc_acceptance_vector_covariance_payload_boundaries() -> TestResult {
    let mut case = fixture::vector_case::<f64>()?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    for (execution, profile) in executions.iter().zip([
        [445, 199_782, 199_606],
        [457, 356_033, 355_857],
        [445, 199_812, 199_636],
    ]) {
        for (kind, needed) in profile.into_iter().enumerate() {
            for (limit, prior, success) in [
                (needed, 0, true),
                (needed - 1, 0, false),
                (needed, u64::from(kind == 0), kind != 0),
            ] {
                let mut limits = Limits::default();
                match kind {
                    0 => limits.work = limit,
                    1 => limits.staged_payload_bytes = limit,
                    _ => limits.output_bytes = limit,
                }
                let mut budget = Budget::new(limits)?;
                budget.work(
                    prior,
                    nrz_bindings::readings::algebra::Location::new("prior vector covariance work"),
                )?;
                assert_eq!(
                    case.heap
                        .evaluate_companion(execution, &context, &mut budget)
                        .is_ok(),
                    success
                );
                for mean in &means {
                    assert!(mean.evaluation().centers(&case.heap, &context).is_ok());
                }
            }
        }
    }
    Ok(())
}
