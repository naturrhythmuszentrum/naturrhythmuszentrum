// SPDX-License-Identifier: AGPL-3.0-only
//! Supported kernels propagate statistics; unfinished modes retain explicit refusals.

use crate::kernel_testing::{TestResult, axis, owner, shape};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, OperationError, ShapeLimits,
};

/// A repeated operand propagates its complete dependence rather than discarding statistics.
/// ´test:integration:deterministic-sum-propagates-carried-statistics´
#[test]
fn deterministic_sum_propagates_carried_statistics() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = crate::statistics_testing::scalar::<f64>(&mut heap)?;
    let result = crate::kernel_testing::execute(
        &mut heap,
        OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?,
        vec![source.value(); 2],
    )?;
    let output = crate::kernel_testing::output(&result)?;
    let view = heap.moments(heap.moment_companion(output.value())?)?;
    crate::statistics_testing::check(
        view.covariance(),
        &[crate::kernel_oracle::Rational::new(1, 1)?],
    )?;
    assert_eq!(view.targets(), [heap.value(output.value())?.stamp(); 2]);
    assert_eq!(view.source_indices(), [0]);
    Ok(())
}

/// A statistical operand cannot make an operand with no declared source mathematically fixed.
/// ´test:integration:statistical-sum-refuses-missing-source´
#[test]
fn statistical_sum_refuses_missing_source() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = crate::statistics_testing::scalar::<f64>(&mut heap)?;
    let unmodeled = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0])?;
    let operation = OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?
        .check(ExecutionLimits::default())?;
    assert!(matches!(
        Binding::new(
            operation,
            vec![source.value(), unmodeled.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Statistics(
            nrz_matrix::StatisticsError::MissingSource { slot: 1, entry: 0 }
        ))
    ));
    Ok(())
}

/// Both variable product operands retain their common source in the local model.
/// ´test:integration:deterministic-product-propagates-carried-statistics´
#[test]
fn deterministic_product_propagates_carried_statistics() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = crate::statistics_testing::scalar::<f64>(&mut heap)?;
    let result = crate::kernel_testing::execute(
        &mut heap,
        OperationDraft::matmul(contract.clone(), contract, ShapeLimits::default())?,
        vec![source.value(); 2],
    )?;
    let output = crate::kernel_testing::output(&result)?;
    let view = heap.moments(heap.moment_companion(output.value())?)?;
    assert_eq!(
        view.assurance(),
        nrz_matrix::MomentAssurance::LinearizedMoments
    );
    crate::statistics_testing::check(
        view.covariance(),
        &[crate::kernel_oracle::Rational::new(4, 1)?],
    )
}

/// A represented factor without source coverage cannot silently become fixed.
/// ´test:integration:statistical-product-refuses-missing-source´
#[test]
fn statistical_product_refuses_missing_source() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = crate::statistics_testing::scalar::<f64>(&mut heap)?;
    let unmodeled = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0])?;
    let operation = OperationDraft::matmul(contract.clone(), contract, ShapeLimits::default())?
        .check(ExecutionLimits::default())?;
    assert!(matches!(
        Binding::new(
            operation,
            vec![source.value(), unmodeled.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Statistics(
            nrz_matrix::StatisticsError::MissingSource { slot: 1, entry: 0 }
        ))
    ));
    Ok(())
}

/// Smooth native recipes retain separate derivative evidence and local covariance.
/// ´test:integration:native-nonlinear-kernels-propagate-carried-statistics´
#[test]
fn native_nonlinear_kernels_propagate_carried_statistics() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    for (function, numerator, denominator) in [
        (nrz_matrix::Elementwise::Square, 4, 1),
        (nrz_matrix::Elementwise::Reciprocal, 1, 64),
    ] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = crate::statistics_testing::scalar::<f64>(&mut heap)?;
        let result = crate::kernel_testing::execute(
            &mut heap,
            OperationDraft::map_elementwise(contract.clone(), contract.clone(), function)?,
            vec![source.value()],
        )?;
        let output = crate::kernel_testing::output(&result)?;
        let view = heap.moments(heap.moment_companion(output.value())?)?;
        assert_eq!(
            view.assurance(),
            nrz_matrix::MomentAssurance::LinearizedMoments
        );
        crate::statistics_testing::check(
            view.covariance(),
            &[crate::kernel_oracle::Rational::new(numerator, denominator)?],
        )?;
    }
    Ok(())
}

/// A deterministic interval containing zero cannot discharge reciprocal smoothness.
/// ´test:integration:native-statistical-reciprocal-refuses-zero-neighborhood´
#[test]
fn native_statistical_reciprocal_refuses_zero_neighborhood() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64], &[2.0])?;
    crate::statistics_testing::attach(&mut heap, source.value(), &[0.25], false)?;
    let operation = OperationDraft::map_elementwise(
        contract.clone(),
        contract,
        nrz_matrix::Elementwise::Reciprocal,
    )?
    .check(ExecutionLimits::default())?;
    let bound = Binding::new(operation, vec![source.value()], ExecutionLimits::default())
        .validate(&mut heap)?;
    assert!(bound.evaluate(bound.normalized_inputs()?).is_err());
    drop(bound);
    assert_eq!(heap.provenance_usage().records(), 0);
    assert!(heap.value(source.value()).is_ok());
    Ok(())
}
