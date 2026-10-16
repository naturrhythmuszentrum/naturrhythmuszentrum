// SPDX-License-Identifier: AGPL-3.0-only
//! Affine solution restoration preserves origins, physical radii and output budgets.

use crate::conversion_testing::{contract, measure, reference_owner};
use crate::kernel_oracle::Rational;
use crate::kernel_testing::{axis, entry, execute, output, shape};
use crate::solve_testing::TestResult;
use faer::prelude::*;
use nrz_accuracy::{Budget, BudgetDecision};
use nrz_matrix::{
    Extent, Heap, HeapLimits, OperationDraft, RecipeEvidence, ShapeLimits, SolveBudget, SolvePolicy,
};
use nrz_scalar::Scalar;
use nrz_units::{BaseDimension, Descriptor, Dimension, Kind, Role};

/// Restoration adds a point origin only to the center and applies the difference scale to both bounds.
/// ´test:integration:solve-restoration-encloses-exact-point-and-difference-results´
#[test]
fn solve_restoration_encloses_exact_point_and_difference_results() -> TestResult {
    restored::<f32>()?;
    restored::<f64>()
}

/// A normalized exact solution of two restores to 293.15 K with a bounded physical family.
fn restored<P: Scalar + 'static>() -> TestResult {
    let (proposal, point) = restoration_operation()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(2)],
        &[P::ZERO],
    )?;
    let rhs = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(4)],
        &[P::ONE / P::from_u8(8)],
    )?;
    let outcome = execute(&mut heap, proposal, vec![operator.value(), rhs.value()])?;
    let result = output(&outcome)?;
    let (center, total, error) = entry(&heap, result, 0)?;
    assert!(Rational::new(5863, 20)?.covered(center, error)?);
    assert!(Rational::new(11701, 40)?.covered(center, total)?);
    assert!(Rational::new(11751, 40)?.covered(center, total)?);
    assert!(total < 0.7);
    let backend = faer::Mat::<f64>::from_fn(1, 1, |_, _| 2.0)
        .full_piv_lu()
        .solve(&faer::Mat::<f64>::from_fn(1, 1, |_, _| 4.0));
    let exact_backend = Rational::binary(backend[(0, 0)])?
        .multiply(Rational::new(10, 1)?)?
        .add(Rational::new(5463, 20)?)?;
    assert!(exact_backend.covered(center, error)?);
    let record = heap.operation_record(outcome.record())?;
    let RecipeEvidence::Solve(proof) = record.evidence().first().ok_or("proof")? else {
        return Err("solve proof".into());
    };
    assert!(proof.restoration().is_some());
    assert_eq!(
        proof.budget().ok_or("budget assessment")?.decision(),
        BudgetDecision::Met
    );
    assert_eq!(
        heap.value(result.value())?.region().descriptor(),
        &point.coherent()
    );
    Ok(())
}

/// Build only exact descriptor definitions; no numerical fixture value enters units algebra.
fn restoration_operation() -> TestResult<(OperationDraft, Descriptor)> {
    let point = Descriptor::new(
        Kind::Named(
            "temperature".into(),
            Box::new(Dimension::base(BaseDimension::Temperature)),
        ),
        measure(1, 1, 27315, 100)?,
        Role::Point,
    )?;
    let declarations = reference_owner(&point)?;
    let coordinate = axis(1)?;
    let normalized = shape(&declarations, &coordinate, &coordinate)?;
    let physical = crate::conversion_testing::shape(&declarations, &point, 1)?;
    let map = declarations
        .normalization_map(&point.coherent(), "reference", Some("origin"))?
        .inverse()?;
    let conversion = contract(normalized.clone(), physical, map)?;
    let policy = SolvePolicy::with_budget(SolveBudget::frobenius(
        point.difference().coherent(),
        Budget::new(1.0, 0.0, 1.0)?,
    )?);
    let operation = OperationDraft::solve(
        normalized.clone(),
        normalized.clone(),
        normalized,
        ShapeLimits::default(),
    )?
    .restore_solution(conversion)?
    .with_solve_policy(policy)?;
    Ok((operation, point))
}
