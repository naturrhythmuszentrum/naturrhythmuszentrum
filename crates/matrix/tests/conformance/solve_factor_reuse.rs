// SPDX-License-Identifier: AGPL-3.0-only
//! Equal center bits cannot authorize reuse across changed operator keys.

use crate::solve_testing::{TestResult, draft};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, Layout, OperationError, SolveError,
    SolveStage,
};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// Operator version, selected region and original checked recipe are separate mandatory key components.
/// ´test:integration:factor-reuse-refuses-changed-version-region-and-recipe´
#[test]
fn factor_reuse_refuses_changed_version_region_and_recipe() -> TestResult {
    refusals::<f32>()?;
    refusals::<f64>()
}
/// Same-valued aliases share a base identity but cannot exchange their captured selections.
fn refusals<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let base = heap.import_represented(
        Extent::new(1, 2)?,
        &Descriptor::ratio(),
        &[P::from_u8(2); 2],
    )?;
    let first = heap.alias(
        base.value(),
        &Layout::gather(Extent::new(1, 1)?, 2, &[0], 1)?,
    )?;
    let second = heap.alias(
        base.value(),
        &Layout::gather(Extent::new(1, 1)?, 2, &[1], 1)?,
    )?;
    let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[P::ONE])?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)?.check(limits)?;
    let bound =
        Binding::new(operation.clone(), vec![first, rhs.value()], limits).validate(&mut heap)?;
    let factor = bound.factor()?;
    drop(bound);
    let failure = heap
        .solve_factored_recorded(
            Binding::new(operation.clone(), vec![second, rhs.value()], limits),
            &factor,
        )
        .err()
        .ok_or("different region reused")?;
    assert_eq!(failure.record().stage(), SolveStage::Factorization);
    assert!(matches!(
        failure.cause(),
        OperationError::Solve(SolveError::ForeignFactor)
    ));
    let failure = heap
        .solve_factored_recorded(
            Binding::new(
                draft(1, 1)?.check(limits)?,
                vec![first, rhs.value()],
                limits,
            ),
            &factor,
        )
        .err()
        .ok_or("different checked revision reused")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Solve(SolveError::ForeignFactor)
    ));
    let version = heap.value(base.value())?.stamp().version();
    heap.replace_represented(base.value(), version, &[P::from_u8(2); 2])?;
    let failure = heap
        .solve_factored_recorded(
            Binding::new(operation, vec![first, rhs.value()], limits),
            &factor,
        )
        .err()
        .ok_or("changed operator version reused")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Solve(SolveError::ForeignFactor)
    ));
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}
