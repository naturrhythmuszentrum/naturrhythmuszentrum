// SPDX-License-Identifier: AGPL-3.0-only
//! Missing checked source rows and joint blocks cannot turn into independence assumptions.

use crate::kernel_testing::{TestResult, axis, owner, shape};
use crate::statistics_testing::scalar;
use nrz_matrix::{
    Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, Extent, Heap, HeapLimits,
    JointError, JointLimits, JointModel, OperationDraft, OperationError, ShapeLimits, Source,
    SourceFactor, StatisticsError, TargetMap,
};
use nrz_units::Descriptor;

/// A raw equal-byte copy cannot supply the checked K rows required for a cross-lineage request.
/// ´test:integration:cross-covariance-refuses-a-value-without-source-rows´
#[test]
fn cross_covariance_refuses_a_value_without_source_rows() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = scalar::<f64>(&mut heap)?;
    let copied = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0])?;
    assert!(matches!(
        heap.materialize_cross_covariance(source.value(), copied.value(), JointLimits::default()),
        Err(OperationError::Statistics(StatisticsError::MissingSource {
            slot: 1,
            entry: 0
        }))
    ));
    Ok(())
}

/// Two separately admitted models do not imply a zero cross block or independent operands.
/// ´test:integration:statistical-sum-refuses-missing-joint-model´
#[test]
fn statistical_sum_refuses_missing_joint_model() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let input = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let first = scalar::<f64>(&mut heap)?;
    let second = scalar::<f64>(&mut heap)?;
    let operation = OperationDraft::add(input.clone(), input, ShapeLimits::default())?
        .check(ExecutionLimits::default())?;
    assert!(matches!(
        Binding::new(
            operation,
            vec![first.value(), second.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Statistics(
            StatisticsError::MissingJointModel
        ))
    ));
    Ok(())
}

/// A PSD completion does not authorize a statistically unknown participating cross block.
/// ´test:integration:statistical-affine-map-refuses-unknown-cross-block´
#[test]
fn statistical_affine_map_refuses_unknown_cross_block() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let column = axis(1)?;
    let input = shape(&declarations, &column, &coordinates)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[2.0_f64, 3.0])?;
    let limits = JointLimits::default();
    let sources = [
        Source::new(&Descriptor::ratio(), 1)?,
        Source::new(&Descriptor::ratio(), 1)?,
    ];
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(2, &[1.0, 1.0], &[0.25], limits)?,
        &[
            CovarianceBlock::Known,
            CovarianceBlock::Unknown,
            CovarianceBlock::Unknown,
            CovarianceBlock::Known,
        ],
        limits,
    )?;
    let target = TargetMap::capture(&heap, source.value())?;
    heap.publish_joint(
        CompanionSetDraft::new(model, [target.clone(), target], [vec![0, 1], vec![0, 1]]),
        limits,
    )?;
    let operation = OperationDraft::copy(vec![input]).check(ExecutionLimits::default())?;
    assert!(matches!(
        Binding::new(operation, vec![source.value()], ExecutionLimits::default())
            .validate(&mut heap),
        Err(OperationError::Statistics(StatisticsError::Joint(
            JointError::MissingCrossBlock
        )))
    ));
    Ok(())
}

/// An already propagated source reaches a solve with its original common-source cancellation intact.
/// ´test:integration:solve-propagates-existing-statistical-lineage´
#[test]
fn solve_propagates_existing_statistical_lineage() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let input = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = scalar::<f64>(&mut heap)?;
    let copied = crate::kernel_testing::execute(
        &mut heap,
        OperationDraft::copy(vec![input.clone()]),
        vec![source.value()],
    )?;
    let value = crate::kernel_testing::output(&copied)?;
    let outcome = crate::kernel_testing::execute(
        &mut heap,
        OperationDraft::solve(input.clone(), input.clone(), input, ShapeLimits::default())?,
        vec![value.value(); 2],
    )?;
    let result = crate::kernel_testing::output(&outcome)?;
    crate::solve_public::check(&heap, result, &[2.0], &[2.0], 1)?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    let zero = [crate::kernel_oracle::Rational::new(0, 1)?];
    crate::statistics_testing::check(moments.covariance(), &zero)?;
    crate::statistics_testing::check(moments.sensitivity().ok_or("source sensitivity")?, &zero)?;
    assert_eq!(
        moments.assurance(),
        nrz_matrix::MomentAssurance::LinearizedMoments
    );
    assert!(heap.moments(heap.moment_companion(value.value())?).is_ok());
    Ok(())
}
