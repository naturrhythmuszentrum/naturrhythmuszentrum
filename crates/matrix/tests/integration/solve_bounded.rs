// SPDX-License-Identifier: AGPL-3.0-only
//! Exact worked solve families and common right-hand-side perturbations in both formats.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{entry, execute, output};
use crate::solve_testing::{TestResult, draft};
use nrz_matrix::{Coverage, Extent, Heap, HeapLimits, RecipeEvidence};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// The diagonal worked example encloses its exact 22/19 perturbed solution.
/// ´test:integration:bounded-solve-encloses-the-worked-diagonal-example´
#[test]
fn bounded_solve_encloses_the_worked_diagonal_example() -> TestResult {
    worked::<f32>()?;
    worked::<f64>()
}

/// Supplied radii enclose the displayed exact 1/10 and 1/5 perturbations in both formats.
fn worked<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let tenth = P::ONE / P::from_u8(10);
    let first = heap.import_declared_bounds(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[P::from_u8(2), P::ZERO, P::ZERO, P::from_u8(4)],
        &[tenth, P::ZERO, P::ZERO, P::ZERO],
    )?;
    let second = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(2), P::from_u8(8)],
        &[P::ONE / P::from_u8(5), P::ZERO],
    )?;
    let outcome = execute(&mut heap, draft(2, 1)?, vec![first.value(), second.value()])?;
    let result = output(&outcome)?;
    crate::solve_public::check(&heap, result, &[2.0, 0.0, 0.0, 4.0], &[2.0, 8.0], 2)?;
    for (index, expected) in [Rational::new(22, 19)?, Rational::new(2, 1)?]
        .into_iter()
        .enumerate()
    {
        let (center, total, _) = entry(&heap, result, index)?;
        assert!(expected.covered(center, total)?);
    }
    assert_eq!(
        heap.value(result.value())?.coverage(),
        Coverage::DeclaredBounds
    );
    let record = heap.operation_record(outcome.record())?;
    let RecipeEvidence::Solve(proof) = record.evidence().first().ok_or("proof")? else {
        return Err("solve proof".into());
    };
    assert!(Rational::new(1, 10)?.at_most(Rational::binary(
        proof.operator_radius().ok_or("complete solve proof")?
    )?)?);
    assert!(Rational::new(1, 5)?.at_most(Rational::binary(
        proof.rhs_radius().ok_or("complete solve proof")?
    )?)?);
    assert!(proof.numerical_radius() <= proof.total_radius().ok_or("complete solve proof")?);
    assert_eq!(
        proof.budget().ok_or("budget assessment")?.decision(),
        nrz_accuracy::BudgetDecision::NotRequested
    );
    Ok(())
}

/// A single uncertain operator perturbs all RHS columns in one Frobenius ball.
/// ´test:integration:bounded-solve-retains-one-operator-across-common-columns´
#[test]
fn bounded_solve_retains_one_operator_across_common_columns() -> TestResult {
    common::<f32>()?;
    common::<f64>()
}

/// Exact common endpoints saturate the scalar-operator theorem up to outward evaluation.
fn common<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let first = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(2)],
        &[P::ONE / P::from_u8(8)],
    )?;
    let second = heap.import_declared_bounds(
        Extent::new(1, 2)?,
        &Descriptor::ratio(),
        &[P::from_u8(2), P::from_u8(4)],
        &[P::ZERO; 2],
    )?;
    let outcome = execute(&mut heap, draft(1, 2)?, vec![first.value(), second.value()])?;
    let result = output(&outcome)?;
    crate::solve_public::check(&heap, result, &[2.0], &[2.0, 4.0], 1)?;
    for (index, expected) in [Rational::new(16, 15)?, Rational::new(32, 15)?]
        .into_iter()
        .enumerate()
    {
        let (center, total, _) = entry(&heap, result, index)?;
        assert!(expected.covered(center, total)?);
    }
    let record = heap.operation_record(outcome.record())?;
    let RecipeEvidence::Solve(proof) = record.evidence().first().ok_or("proof")? else {
        return Err("solve proof".into());
    };
    let radius = Rational::binary(proof.total_radius().ok_or("complete solve proof")?)?;
    assert!(Rational::new(1, 45)?.at_most(radius.multiply(radius)?)?);
    assert!(proof.total_radius().ok_or("complete solve proof")? < 0.16);
    assert!(proof.perturbation_margin().ok_or("complete solve proof")? > 0.0);
    Ok(())
}
