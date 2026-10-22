// SPDX-License-Identifier: AGPL-3.0-only
//! Changed right-hand sides reuse only operator resources and get fresh solve records.

use crate::kernel_testing::output;
use crate::solve_testing::{TestResult, draft};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, SolvePolicy, SolveRecordKeys, SolveStage,
};
use nrz_scalar::Scalar;

/// RHS replacement, unrelated mutation and compaction preserve factor eligibility in both formats.
/// ´test:integration:factor-reuse-is-specific-to-unchanged-operator-dependencies´
#[test]
fn factor_reuse_is_specific_to_unchanged_operator_dependencies() -> TestResult {
    reuse::<f32>()?;
    reuse::<f64>()
}
/// Exact rational oracles certify both RHS problems independently of the reused LU resource.
fn reuse<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[P::from_u8(3), P::ONE, P::ONE, P::from_u8(2)],
    )?;
    let rhs = heap.import_represented(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(5), P::from_u8(5)],
    )?;
    let unrelated = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[P::ONE])?;
    let proposal = draft(2, 1)?;
    let limits = ExecutionLimits::default();
    let operation = proposal.clone().check(limits)?;
    let bound =
        Binding::new(operation, vec![operator.value(), rhs.value()], limits).validate(&mut heap)?;
    let factor = bound.factor()?;
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let original = bound.publish(candidate)?;
    crate::solve_public::check(
        &heap,
        output(&original)?,
        &[3.0, 1.0, 1.0, 2.0],
        &[5.0, 5.0],
        2,
    )?;
    let unused_version = heap.value(unrelated.value())?.stamp().version();
    heap.replace_represented(unrelated.value(), unused_version, &[P::from_u8(9)])?;
    heap.compact()?;
    let previous = heap.value(rhs.value())?.stamp();
    heap.replace_represented(
        rhs.value(),
        previous.version(),
        &[P::from_u8(7), P::from_u8(4)],
    )?;
    let keys = SolveRecordKeys::new(
        Some("plan".into()),
        None,
        Some("second RHS".into()),
        Some("solve".into()),
    )?;
    let operation = proposal
        .with_solve_policy(SolvePolicy::default().with_record_keys(keys))?
        .check(limits)?;
    let result = heap.solve_factored_recorded(
        Binding::new(operation, vec![operator.value(), rhs.value()], limits),
        &factor,
    )?;
    crate::solve_public::check(
        &heap,
        output(&result)?,
        &[3.0, 1.0, 1.0, 2.0],
        &[7.0, 4.0],
        2,
    )?;
    let record = heap.operation_record(result.record())?;
    assert_eq!(record.solve_stage(), Some(SolveStage::Published));
    let usage = record
        .solve_evidence()
        .ok_or("solve")?
        .factor_use()
        .ok_or("factor")?;
    assert!(usage.reused());
    assert_eq!(usage.origin(), factor.execution());
    assert_eq!(usage.execution(), record.execution());
    assert_ne!(usage.origin(), usage.execution());
    assert_eq!(usage.dependency(), factor.dependency());
    assert_ne!(record.inputs().get(1).ok_or("RHS")?.stamp(), previous);
    assert_eq!(
        record
            .operation()
            .solve_record_keys()
            .ok_or("keys")?
            .point(),
        Some("second RHS")
    );
    assert!(heap.operation_record(original.record()).is_err());
    assert!(
        heap.historical_record(original.record())?
            .solve_evidence()
            .is_some()
    );
    Ok(())
}
