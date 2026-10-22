// SPDX-License-Identifier: AGPL-3.0-only
//! Family margin, actual binding, and deterministic coverage refusals.

use crate::kernel_testing::output;
use crate::solve_testing::{TestResult, draft};
use nrz_bindings::Descriptor;
use nrz_matrix::{Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError, SolveError};

/// Family uncertainty cannot be accepted when the common operator may be singular.
/// ´test:integration:solve-perturbation-margin-refuses-at-and-above-one´
#[test]
fn solve_perturbation_margin_refuses_at_and_above_one() -> TestResult {
    for radius in [0.5_f64, 1.0, 2.0] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let operator = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[1.0],
            &[radius],
        )?;
        let rhs =
            heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0], &[0.0])?;
        let limits = ExecutionLimits::default();
        let bound = Binding::new(
            draft(1, 1)?.check(limits)?,
            vec![operator.value(), rhs.value()],
            limits,
        )
        .validate(&mut heap)?;
        let evaluated = bound.evaluate(bound.normalized_inputs()?);
        if radius < 1.0 {
            let outcome = bound.publish(evaluated?)?;
            let result = output(&outcome)?;
            crate::solve_public::check(&heap, result, &[1.0], &[1.0], 1)?;
            let (center, total, _) = crate::kernel_testing::entry(&heap, result, 0)?;
            assert!(crate::kernel_oracle::Rational::new(2, 1)?.covered(center, total)?);
        } else {
            let failure = evaluated.err().ok_or("singular family admitted")?;
            assert!(matches!(
                failure.cause(),
                OperationError::Solve(SolveError::PerturbationMargin)
            ));
            let Some(nrz_matrix::RecipeEvidence::Solve(partial)) =
                failure.record().historical_evidence().first()
            else {
                return Err("missing represented-center evidence".into());
            };
            assert!(!partial.is_complete());
            assert_eq!(partial.total_radius(), None);
            assert_eq!(partial.numerical_radius().to_bits(), 0.0_f64.to_bits());
        }
    }
    Ok(())
}

/// Missing physical coverage never becomes exact physical input by implicit mixing.
/// ´test:integration:solve-mixed-coverage-and-concrete-shape-mismatches-refuse´
#[test]
fn solve_mixed_coverage_and_concrete_shape_mismatches_refuse() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let first = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let second =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64], &[0.0])?;
    let limits = ExecutionLimits::default();
    assert!(matches!(
        Binding::new(
            draft(1, 1)?.check(limits)?,
            vec![first.value(), second.value()],
            limits
        )
        .validate(&mut heap),
        Err(OperationError::Solve(SolveError::CoverageMismatch))
    ));
    let wrong = heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[2.0_f64; 2])?;
    assert!(matches!(
        Binding::new(
            draft(1, 1)?.check(limits)?,
            vec![first.value(), wrong.value()],
            limits
        )
        .validate(&mut heap),
        Err(OperationError::InputMismatch { slot: 1 })
    ));
    Ok(())
}

/// A nearly singular represented center may fail the independent witness despite finite LU output.
/// ´test:integration:near-singular-solve-refuses-an-unresolved-inverse-defect´
#[test]
fn near_singular_solve_refuses_an_unresolved_inverse_defect() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let first = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[1.0_f64, 1.0, 1.0, f64::from_bits(1.0_f64.to_bits() + 1)],
    )?;
    let second =
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0_f64; 2])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(2, 1)?.check(limits)?,
        vec![first.value(), second.value()],
        limits,
    )
    .validate(&mut heap)?;
    let failure = bound
        .evaluate(bound.normalized_inputs()?)
        .err()
        .ok_or("unresolved inverse accepted")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Solve(SolveError::CannotCertifyInverse)
    ));
    Ok(())
}
