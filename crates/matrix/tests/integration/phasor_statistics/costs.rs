// SPDX-License-Identifier: AGPL-3.0-only
//! Full-family evaluation retains all plans while lowering and evaluating each signed map.

use super::fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{Budget, Limits};
use nrz_matrix::CompanionExecution;
use nrz_scalar::ArithmeticContext;

/// Refusal at either payload boundary returns no statistical family and leaves the means available.
/// ´test:integration:shared-voltage-phasor-statistics-have-payload-boundaries´
#[test]
fn shared_voltage_phasor_statistics_have_payload_boundaries() -> TestResult {
    let mut case = fixture::case::<f64>()?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    for (staged, needed) in [true, false].into_iter().zip(required(&case, &executions)?) {
        for (limit, success) in [(needed, true), (needed - 1, false)] {
            assert_eq!(
                case.heap
                    .evaluate_phasor_statistics(&executions, &context, &mut policy(staged, limit)?)
                    .is_ok(),
                success
            );
            assert!(means[0].evaluation().centers(&case.heap, &context).is_ok());
            assert!(means[1].evaluation().centers(&case.heap, &context).is_ok());
        }
    }
    Ok(())
}

/// Measured construction, mean publication and covariance phases pin cumulative Work and payload.
/// ´test:integration:shared-voltage-phasor-costs-are-pinned´
#[test]
fn shared_voltage_phasor_costs_are_pinned() -> TestResult {
    let mut budget = crate::phasor_solve_testing::budget()?;
    let mut case = fixture::case_with_budget::<f64>(&mut budget)?;
    let prepared = budget.usage().work;
    let means = case.means(&mut budget)?;
    let mean_work = budget.usage().work - prepared;
    let before = budget.usage().work;
    let executions = case.executions(&means, &mut budget)?;
    let planning = budget.usage().work - before;
    let result = case.heap.evaluate_phasor_statistics(
        &executions,
        &ArithmeticContext::new()?,
        &mut budget,
    )?;
    let phases = result.evaluations().each_ref().map(|evaluation| {
        let usage = evaluation.usage();
        [
            usage.before_evaluation.work - usage.before_lowering.work,
            usage.after.work - usage.before_evaluation.work,
        ]
    });
    let bytes = executions
        .each_ref()
        .map(CompanionExecution::retained_payload_bytes);
    assert_eq!(
        (
            case.construction,
            mean_work,
            planning,
            phases,
            bytes,
            result.retained_payload_bytes(),
            budget.usage().work,
            required(&case, &executions)?
        ),
        (
            [441_084, 1_561_619, 2_541_555, 2_587_012],
            17_092,
            2655,
            [[144, 13], [156, 13], [144, 13]],
            [192_952, 349_203, 192_982],
            426_761,
            2_607_296,
            [776_292, 426_761]
        )
    );
    Ok(())
}

/// Read-only evaluation permits a precise payload search against the unchanged default policy.
fn required(
    case: &fixture::Case<f64>,
    executions: &[CompanionExecution<f64>; 3],
) -> TestResult<[u64; 2]> {
    let context = ArithmeticContext::new()?;
    let mut result = [0; 2];
    for (index, staged) in [true, false].into_iter().enumerate() {
        let mut lower = 0;
        let mut upper = if staged {
            Limits::default().staged_payload_bytes
        } else {
            Limits::default().output_bytes
        };
        case.heap
            .evaluate_phasor_statistics(executions, &context, &mut policy(staged, upper)?)?;
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            if case
                .heap
                .evaluate_phasor_statistics(executions, &context, &mut policy(staged, middle)?)
                .is_ok()
            {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        result[index] = upper;
    }
    Ok(result)
}

/// A payload experiment never changes Work or another limit to obtain success.
fn policy(staged: bool, limit: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    if staged {
        limits.staged_payload_bytes = limit;
    } else {
        limits.output_bytes = limit;
    }
    Ok(Budget::new(limits)?)
}

/// Repeated references retain one definition allocation while distinct equations remain separately charged.
/// ´test:integration:shared-voltage-phasor-definitions-are-charged-once´
#[test]
fn shared_voltage_phasor_definitions_are_charged_once() -> TestResult {
    let mut case = fixture::case::<f64>()?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let left = executions[0]
        .exact()
        .target()
        .phasor_law()
        .ok_or("left law")?;
    let cross = executions[1]
        .exact()
        .target()
        .phasor_law()
        .ok_or("cross law")?;
    let right = executions[2]
        .exact()
        .target()
        .phasor_law()
        .ok_or("right law")?;
    assert!(left.equations()[0].shares_allocation(&left.equations()[1]));
    assert!(left.equations()[0].shares_allocation(&cross.equations()[0]));
    assert!(cross.equations()[1].shares_allocation(&right.equations()[0]));
    assert!(right.equations()[0].shares_allocation(&right.equations()[1]));
    assert!(!cross.equations()[0].shares_allocation(&cross.equations()[1]));
    let bytes = executions
        .iter()
        .map(CompanionExecution::retained_payload_bytes)
        .sum::<u64>()
        + 4096
        - left.equations()[0].retained_payload_bytes()
        - right.equations()[0].retained_payload_bytes();
    let result = case.heap.evaluate_phasor_statistics(
        &executions,
        &ArithmeticContext::new()?,
        &mut ledger()?,
    )?;
    assert_eq!(result.retained_payload_bytes(), bytes);
    assert!(bytes <= Limits::default().output_bytes);
    Ok(())
}
