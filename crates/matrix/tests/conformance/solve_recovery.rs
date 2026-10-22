// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded recovery refuses exhausted or stagnant certificates without publishing last iterates.

use crate::kernel_testing::output;
use crate::recovery_testing::{policy, proof};
use crate::solve_testing::{TestResult, draft};
use nrz_accuracy::{Limits, Promotion};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError, SolveError, SolveStop,
};
use nrz_scalar::{Precision, Scalar};

/// Zero corrections and stagnation retain complete attempts but never issue a success candidate.
/// ´test:integration:solve-refinement-stops-with-an-honest-refusal´
#[test]
fn solve_refinement_stops_with_an_honest_refusal() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f32])?;
    let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f32])?;
    let limits = ExecutionLimits::default();
    for (refinements, stop, count) in [
        (0, SolveStop::RefinementLimit, 1),
        (8, SolveStop::Stagnation, 2),
    ] {
        let operation = draft(1, 1)?
            .with_solve_policy(policy(
                0.0,
                refinements,
                Promotion::Never,
                Limits::new(16, 1_000_000),
            )?)?
            .check(limits)?;
        let bound = Binding::new(operation, vec![operator.value(), rhs.value()], limits)
            .validate(&mut heap)?;
        let failure = bound
            .evaluate(bound.normalized_inputs()?)
            .err()
            .ok_or("last iterate published")?;
        let recovery = proof(failure.record().historical_evidence())?
            .recovery()
            .ok_or("recovery evidence")?;
        assert_eq!(recovery.stop(), stop);
        assert_eq!(recovery.attempts().len(), count);
        if count == 2 {
            assert_eq!(
                recovery
                    .attempts()
                    .get(1)
                    .ok_or("correction")?
                    .residual_precision(),
                Precision::F64
            );
        }
        for attempt in recovery.attempts() {
            assert!(attempt.radius() > 0.0);
        }
        drop(bound);
        assert_eq!(heap.provenance_usage().records(), 0);
    }
    Ok(())
}
/// The cumulative solve-work limit refuses immediately below its admitted cost and accepts equality.
/// ´test:integration:solve-recovery-work-below-at-and-above´
#[test]
fn solve_recovery_work_below_at_and_above() -> TestResult {
    resource::<f32>()?;
    resource::<f64>()
}
/// Both precision paths use the same explicit admission count and retain exact rational truth.
fn resource<P: Scalar + 'static>() -> TestResult {
    let required = u64::try_from(
        draft(1, 1)?
            .check(ExecutionLimits::default())?
            .solve_work()
            .ok_or("work")?,
    )?;
    for allowance in [required - 1, required, required + 1] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let input =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[P::from_u8(2)])?;
        let limits = ExecutionLimits::default();
        let operation = draft(1, 1)?
            .with_solve_policy(policy(1.0, 0, Promotion::Never, Limits::new(1, allowance))?)?
            .check(limits)?;
        let bound = Binding::new(operation, vec![input.value(); 2], limits).validate(&mut heap)?;
        let result = bound.evaluate(bound.normalized_inputs()?);
        if allowance < required {
            assert!(matches!(
                result.err().ok_or("work exceeded")?.cause(),
                OperationError::Solve(SolveError::ResourceLimit)
            ));
        } else {
            let outcome = bound.publish(result?)?;
            crate::solve_public::check(&heap, output(&outcome)?, &[2.0], &[2.0], 1)?;
            let recovery = proof(heap.operation_record(outcome.record())?.evidence())?
                .recovery()
                .ok_or("recovery")?;
            assert_eq!(recovery.work(), required);
            assert_eq!(recovery.stop(), SolveStop::BudgetMet);
        }
    }
    Ok(())
}

/// An operator outside the admitted inverse-defect range refuses even with finite LU entries.
/// ´test:integration:recovery-refuses-unadmitted-ill-conditioned-operators´
#[test]
fn recovery_refuses_unadmitted_ill_conditioned_operators() -> TestResult {
    ill_conditioned::<f32>()?;
    ill_conditioned::<f64>()
}
/// Adjacent representable rows force an unresolved independent inverse certificate in each format.
fn ill_conditioned<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[P::ONE, P::ONE, P::ONE, P::ONE.next_up()],
    )?;
    let rhs = heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[P::ONE; 2])?;
    let limits = ExecutionLimits::default();
    let operation = draft(2, 1)?
        .with_solve_policy(policy(1.0, 2, Promotion::Never, Limits::new(3, 100_000))?)?
        .check(limits)?;
    let bound =
        Binding::new(operation, vec![operator.value(), rhs.value()], limits).validate(&mut heap)?;
    let failure = bound
        .evaluate(bound.normalized_inputs()?)
        .err()
        .ok_or("unadmitted operator certified")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Solve(SolveError::CannotCertifyInverse)
    ));
    drop(bound);
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}
