// SPDX-License-Identifier: AGPL-3.0-only
//! Homogeneous physical solves retain the solution kind and reject incorrectly typed budgets.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{entry, execute, output};
use crate::shape_testing::{axis, dense, owner, quantity};
use crate::solve_testing::TestResult;
use nrz_accuracy::Budget;
use nrz_bindings::{Descriptor, ExponentPolicy, Registry};
use nrz_matrix::{
    EntryDraft, Extent, Heap, HeapLimits, OperationDraft, OperationError, ShapeLimits, SolveBudget,
    SolveError, SolvePolicy,
};
use nrz_scalar::Scalar;

/// The solution and its tolerance retain voltage kinds in both execution formats.
/// ´test:integration:physical-square-solve-retains-voltage-and-refuses-ratio-budget´
#[test]
fn physical_square_solve_retains_voltage_and_refuses_ratio_budget() -> TestResult {
    physical::<f32>()?;
    physical::<f64>()
}

/// An exact ratio operator divides a bounded voltage family without changing its physical kind.
fn physical<P: Scalar + 'static>() -> TestResult {
    let voltage = quantity("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let proposal = physical_operation(&voltage)?;
    let coefficients = Budget::new(0.25, 0.0, 1.0)?;
    let wrong =
        SolvePolicy::with_budget(SolveBudget::frobenius(Descriptor::ratio(), coefficients)?);
    assert!(matches!(
        proposal.clone().with_solve_policy(wrong),
        Err(OperationError::Solve(SolveError::BudgetContract))
    ));
    let policy = SolvePolicy::with_budget(SolveBudget::frobenius(voltage.clone(), coefficients)?);
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(2)],
        &[P::ZERO],
    )?;
    let rhs = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &voltage,
        &[P::from_u8(8)],
        &[P::ONE / P::from_u8(4)],
    )?;
    let result = execute(
        &mut heap,
        proposal.with_solve_policy(policy)?,
        vec![operator.value(), rhs.value()],
    )?;
    let bundle = output(&result)?;
    crate::solve_public::check(&heap, bundle, &[2.0], &[8.0], 1)?;
    assert_eq!(
        heap.symbol_binding(bundle.value())?.symbol().descriptor(),
        &voltage
    );
    let (center, radius, _) = entry(&heap, bundle, 0)?;
    assert!(Rational::new(31, 8)?.covered(center, radius)?);
    assert!(Rational::new(33, 8)?.covered(center, radius)?);
    Ok(())
}

/// Check physical axis pairings against one immutable declaration owner.
fn physical_operation(voltage: &Descriptor) -> TestResult<OperationDraft> {
    let declarations = owner(Registry::new(ExponentPolicy::Rational))?;
    let variables = axis(&["voltage-unknown"], voltage)?;
    let equations = axis(&["voltage-equation"], voltage)?;
    let columns = axis(&["source-column"], &Descriptor::ratio())?;
    let operator = dense(
        &declarations,
        &variables,
        &equations,
        vec![EntryDraft::Declared(Descriptor::ratio())],
    )?;
    let rhs = dense(
        &declarations,
        &columns,
        &equations,
        vec![EntryDraft::Declared(voltage.clone())],
    )?;
    let solution = dense(
        &declarations,
        &columns,
        &variables,
        vec![EntryDraft::Declared(voltage.clone())],
    )?;
    Ok(OperationDraft::solve(
        operator,
        rhs,
        solution,
        ShapeLimits::default(),
    )?)
}
