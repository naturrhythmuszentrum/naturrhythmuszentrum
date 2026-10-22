// SPDX-License-Identifier: AGPL-3.0-only
//! Public square solve premises and independent exact rational positive twins.

use crate::kernel_testing::{execute, output};
use crate::solve_testing::{TestResult, draft, shapes};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, OperationError,
    ShapeLimits, SolveError,
};

/// Both formats certify a pivoted system and all physical RHS columns together.
/// ´test:integration:represented-square-solves-enclose-exact-rational-solutions´
#[test]
fn represented_square_solves_enclose_exact_rational_solutions() -> TestResult {
    represented::<f32>()?;
    represented::<f64>()
}

/// Integer fixtures admit exact independent elimination and a distinct faer factorization.
fn represented<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = [P::ZERO, P::ONE, P::ONE, P::ONE + P::ONE];
    let rhs = [P::ONE, P::ONE + P::ONE + P::ONE, P::ONE + P::ONE, P::ONE];
    let first = heap.import_represented(Extent::new(2, 2)?, &Descriptor::ratio(), &operator)?;
    let second = heap.import_represented(Extent::new(2, 2)?, &Descriptor::ratio(), &rhs)?;
    let outcome = execute(&mut heap, draft(2, 2)?, vec![first.value(), second.value()])?;
    crate::solve_public::check(
        &heap,
        output(&outcome)?,
        &[0.0, 1.0, 1.0, 2.0],
        &[1.0, 3.0, 2.0, 1.0],
        2,
    )?;
    let record = heap.operation_record(outcome.record())?;
    let nrz_matrix::RecipeEvidence::Solve(claim) =
        record.evidence().first().ok_or("solve evidence")?
    else {
        return Err("wrong recipe".into());
    };
    assert!(claim.inverse_defect() < 1.0);
    assert!(claim.inverse_margin() > 0.0);
    assert_eq!(record.inputs().len(), 2);
    Ok(())
}

/// Shape checks reject zero order, no RHS columns and incompatible ordered equation axes.
/// ´test:integration:square-solve-shape-refusals-precede-backend-entry´
#[test]
fn square_solve_shape_refusals_precede_backend_entry() -> TestResult {
    for (order, columns) in [(0, 1), (1, 0)] {
        let [operator, rhs, solution] = shapes(order, columns)?;
        assert!(matches!(
            OperationDraft::solve(operator, rhs, solution, ShapeLimits::default()),
            Err(OperationError::Shape(nrz_matrix::ShapeError::SolveExtent))
        ));
    }
    let [operator, _, solution] = shapes(2, 1)?;
    let [_, foreign, _] = shapes(2, 1)?;
    assert!(matches!(
        OperationDraft::solve(operator, foreign, solution, ShapeLimits::default()),
        Err(OperationError::Shape(_))
    ));
    Ok(())
}

/// A singular backend candidate never issues a published value or a false inverse proof.
/// ´test:integration:singular-square-solve-refuses-without-publication´
#[test]
fn singular_square_solve_refuses_without_publication() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator =
        heap.import_represented(Extent::new(2, 2)?, &Descriptor::ratio(), &[1.0_f64; 4])?;
    let rhs = heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0_f64; 2])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(2, 1)?.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let failure = bound
        .evaluate(bound.normalized_inputs()?)
        .err()
        .ok_or("singular accepted")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Solve(SolveError::CandidateFailure | SolveError::CannotCertifyInverse)
    ));
    drop(bound);
    assert_eq!(
        heap.value(operator.value())?.region().export()?,
        vec![1.0; 4]
    );
    Ok(())
}

/// Binding limits independently recheck the complete solve requirement.
/// ´test:integration:square-solve-work-limits-test-below-at-and-above´
#[test]
fn square_solve_work_limits_test_below_at_and_above() -> TestResult {
    let proposal = draft(1, 1)?;
    let checked = proposal.clone().check(ExecutionLimits::default())?;
    let required = checked.solve_work().ok_or("work")?;
    for capacity in [required - 1, required, required + 1] {
        let limits = ExecutionLimits::new(2, capacity, capacity);
        if capacity < required {
            assert!(matches!(
                proposal.clone().check(limits),
                Err(OperationError::ResourceLimit)
            ));
        }
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let result =
            Binding::new(checked.clone(), vec![source.value(); 2], limits).validate(&mut heap);
        if capacity < required {
            assert!(matches!(result, Err(OperationError::ResourceLimit)));
        } else {
            let bound = result?;
            let candidate = bound.evaluate(bound.normalized_inputs()?)?;
            let outcome = bound.publish(candidate)?;
            crate::solve_public::check(&heap, output(&outcome)?, &[2.0], &[2.0], 1)?;
        }
    }
    Ok(())
}
