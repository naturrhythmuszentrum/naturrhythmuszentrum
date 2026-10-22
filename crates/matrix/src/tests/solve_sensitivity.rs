// SPDX-License-Identifier: AGPL-3.0-only
//! Certified enclosed RHS parameters feed differential solves through the original factors.

use crate::kernel_oracle::Rational;
use crate::solve_testing::{TestResult, draft};
use crate::statistics::{Matrix, enclosed};
use crate::{Binding, ExecutionLimits, Extent, Heap, HeapLimits};
use nrz_bindings::Descriptor;
use nrz_scalar::ArithmeticContext;

/// The differential parameter enclosure is retained even when its represented candidate has zero residual.
/// ´test:crate:solve-differential-includes-rhs-parameter-error´
#[test]
fn solve_differential_includes_rhs_parameter_error() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(1, 1)?.check(limits)?,
        vec![operator.value(); 2],
        limits,
    )
    .validate(&mut heap)?;
    let factor = bound.factor()?;
    let views = bound.normalized_inputs()?;
    let context = ArithmeticContext::new()?;
    let mut rhs = Matrix::zeros(1, 1)?;
    rhs.set(0, 0, 2.0, enclosed(2.0, 0.25, &context)?)?;
    let (result, proof) = crate::solve::sensitivity::enclosed_solve(
        views.views().first().ok_or("operator")?,
        &rhs,
        &factor,
        &mut crate::statistics::contraction::Work::new(1),
        &context,
    )?;
    let center = *result.centers.first().ok_or("center")?;
    assert!(Rational::new(7, 8)?.covered(center, proof.radius())?);
    assert!(Rational::new(9, 8)?.covered(center, proof.radius())?);
    Ok(())
}

/// The propagated differential retains the original model allocation and separate parameter targets.
/// ´test:crate:solve-differential-keeps-original-model-and-parameter-targets´
#[test]
fn solve_differential_keeps_original_model_and_parameter_targets() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] =
        crate::solve_statistics_testing::common(&mut heap, &[1.0_f64, 3.0, 1.0], true)?;
    let original = std::rc::Rc::clone(&heap.joint.records.first().ok_or("joint record")?.model);
    let outcome = crate::kernel_testing::execute(
        &mut heap,
        draft(1, 2)?,
        vec![operator.value(), rhs.value()],
    )?;
    let result = crate::kernel_testing::output(&outcome)?;
    let reference = heap.moment_companion(result.value())?;
    let record = heap
        .statistics
        .records
        .get(reference.index)
        .ok_or("moment record")?;
    assert!(std::rc::Rc::ptr_eq(&original, &record.lineage.model.data));
    let jacobian = record.jacobian.as_ref().ok_or("Jacobian")?;
    assert_ne!(jacobian.regions[1], record.covariance.regions[1]);
    assert_ne!(jacobian.regions[1], result.value().key);
    let moments = heap.moments(reference)?;
    assert!(Rational::new(1, 16)?.covered(
        moments.covariance().get(0, 0)?,
        moments.covariance().error(0, 0)?
    )?);
    Ok(())
}
