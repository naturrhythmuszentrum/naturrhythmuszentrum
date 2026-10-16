// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh f64 recovery preserves f32 input information and every source relationship.

use crate::kernel_testing::output;
use crate::recovery_testing::{policy, proof};
use crate::solve_testing::{TestResult, draft};
use nrz_accuracy::{Limits, Promotion};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError, SolveError, SolveStop,
};
use nrz_scalar::Precision;
use nrz_units::Descriptor;

/// A tighter requested certificate succeeds only after explicit widening and a fresh f64 execution.
/// ´test:integration:solve-f64-retry-has-distinct-conversion-and-certificate´
#[test]
fn solve_f64_retry_has_distinct_conversion_and_certificate() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f32])?;
    let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f32])?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)?
        .with_solve_policy(policy(
            1.0e-12,
            0,
            Promotion::F32ToF64,
            Limits::new(2, 1_000_000),
        )?)?
        .check(limits)?;
    let result = heap.solve_recovering(Binding::new(
        operation,
        vec![operator.value(), rhs.value()],
        limits,
    ))?;
    assert_eq!(result.precision(), Precision::F64);
    assert!(result.single().is_none());
    let original = result.initial_failure().ok_or("initial failure")?;
    assert_eq!(
        proof(original.record().historical_evidence())?
            .recovery()
            .ok_or("initial attempts")?
            .stop(),
        SolveStop::RefinementLimit
    );
    let outcome = result.double().ok_or("promoted result")?;
    crate::solve_public::check(&heap, output(outcome)?, &[3.0], &[1.0], 1)?;
    let record = heap.operation_record(outcome.record())?;
    let conversion = heap.operation_record(result.conversion_record().ok_or("conversion")?)?;
    assert_eq!(
        conversion.inputs().first().ok_or("source")?.precision(),
        Precision::F32
    );
    assert_eq!(
        record.inputs().first().ok_or("input")?.precision(),
        Precision::F64
    );
    assert_ne!(
        record.inputs().first().ok_or("input")?.stamp(),
        heap.value(operator.value())?.stamp()
    );
    assert_eq!(
        proof(record.evidence())?
            .recovery()
            .ok_or("attempts")?
            .stop(),
        SolveStop::BudgetMet
    );
    Ok(())
}
/// Widening retains the original declared physical error floor, so more precision cannot fake success.
/// ´test:integration:solve-retry-retains-original-input-bounds´
#[test]
fn solve_retry_retains_original_input_bounds() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f32], &[0.0])?;
    let rhs = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[1.0_f32],
        &[0.125],
    )?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)?
        .with_solve_policy(policy(
            1.0e-12,
            0,
            Promotion::F32ToF64,
            Limits::new(2, 1_000_000),
        )?)?
        .check(limits)?;
    let failure = heap
        .solve_recovering(Binding::new(
            operation,
            vec![operator.value(), rhs.value()],
            limits,
        ))
        .err()
        .ok_or("physical error lost")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Solve(SolveError::RefinementLimit)
    ));
    let promoted = proof(
        failure
            .promoted_failure()
            .ok_or("promoted refusal")?
            .record()
            .historical_evidence(),
    )?;
    assert!(promoted.rhs_radius().ok_or("radius")? >= 0.125);
    assert!(failure.conversion_record().is_some());
    Ok(())
}
/// The retry shares the finite cumulative allowance, including its separate widening phase.
/// ´test:integration:solve-retry-cumulative-work-below-at-and-above´
#[test]
fn solve_retry_cumulative_work_below_at_and_above() -> TestResult {
    let required = u64::try_from(
        draft(1, 1)?
            .check(ExecutionLimits::default())?
            .solve_work()
            .ok_or("work")?,
    )? * 2
        + 32;
    for allowance in [required - 1, required, required + 1] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let operator =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f32])?;
        let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f32])?;
        let limits = ExecutionLimits::default();
        let operation = draft(1, 1)?
            .with_solve_policy(policy(
                1.0e-12,
                0,
                Promotion::F32ToF64,
                Limits::new(2, allowance),
            )?)?
            .check(limits)?;
        let result = heap.solve_recovering(Binding::new(
            operation,
            vec![operator.value(), rhs.value()],
            limits,
        ));
        if allowance < required {
            assert!(matches!(
                result.err().ok_or("cumulative work exceeded")?.cause(),
                OperationError::Solve(SolveError::ResourceLimit)
            ));
        } else {
            let result = result?;
            crate::solve_public::check(
                &heap,
                output(result.double().ok_or("double")?)?,
                &[3.0],
                &[1.0],
                1,
            )?;
        }
    }
    Ok(())
}
/// Complete source lineage survives the same precision retry as the deterministic inputs.
/// ´test:integration:solve-retry-preserves-common-source-covariance´
#[test]
fn solve_retry_preserves_common_source_covariance() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] =
        crate::solve_statistics_testing::common(&mut heap, &[1.0_f32, 3.0, 1.0], true)?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 2)?
        .with_solve_policy(policy(
            1.0e-12,
            0,
            Promotion::F32ToF64,
            Limits::new(2, 1_000_000),
        )?)?
        .check(limits)?;
    let result = heap.solve_recovering(Binding::new(
        operation,
        vec![operator.value(), rhs.value()],
        limits,
    ))?;
    let bundle = output(result.double().ok_or("double")?)?;
    let expected = [(1, 16), (-1, 32), (-1, 32), (1, 64)]
        .into_iter()
        .map(|(n, d)| crate::kernel_oracle::Rational::new(n, d))
        .collect::<TestResult<Vec<_>>>()?;
    crate::statistics_testing::check(
        heap.moments(heap.moment_companion(bundle.value())?)?
            .covariance(),
        &expected,
    )?;
    Ok(())
}
