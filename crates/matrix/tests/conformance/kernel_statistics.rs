// SPDX-License-Identifier: AGPL-3.0-only
//! Supported kernels propagate statistics; unfinished modes retain explicit refusals.

use crate::kernel_testing::{TestResult, axis, owner, shape};
use nrz_matrix::{
    Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, Extent, Heap, HeapError,
    HeapLimits, JointLimits, JointModel, OperationDraft, OperationError, ShapeLimits, Source,
    SourceFactor, TargetMap,
};
use nrz_units::Descriptor;

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

/// A product cannot drop statistics while propagating only its deterministic bounds.
/// ´test:integration:deterministic-product-refuses-carried-statistics´
#[test]
fn deterministic_product_refuses_carried_statistics() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    refuse_statistics(
        OperationDraft::matmul(contract.clone(), contract, ShapeLimits::default())?,
        2.0,
        2,
    )
}

/// Attach one current statistical relation, then demand the same typed refusal from a recipe.
fn refuse_statistics(draft: OperationDraft, center: f64, operands: usize) -> TestResult {
    let quantity = Descriptor::ratio();
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_represented(Extent::new(1, 1)?, &quantity, &[center])?;
    let target = TargetMap::capture(&heap, input.value())?;
    let source = Source::new(&quantity, 1)?;
    let joint_limits = JointLimits::default();
    let model = JointModel::from_factor(
        std::slice::from_ref(&source),
        SourceFactor::new(1, &[1.0], &[0.25], joint_limits)?,
        &[CovarianceBlock::Known],
        joint_limits,
    )?;
    let companion = heap.publish_joint(
        CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]]),
        joint_limits,
    )?;
    let operation = draft.check(ExecutionLimits::default())?;
    assert!(matches!(
        Binding::new(
            operation,
            vec![input.value(); operands],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Heap(HeapError::MissingCoverage))
    ));
    assert!(heap.statistical(companion).is_ok());
    Ok(())
}

/// Nonlinear operations, including square root at zero, never discard a statistical operand.
/// ´test:integration:native-nonlinear-kernels-refuse-carried-statistics´
#[test]
fn native_nonlinear_kernels_refuse_carried_statistics() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    for (function, center) in [
        (nrz_matrix::Elementwise::Square, 2.0),
        (nrz_matrix::Elementwise::Reciprocal, 2.0),
        (nrz_matrix::Elementwise::SquareRoot, 0.0),
    ] {
        refuse_statistics(
            OperationDraft::map_elementwise(contract.clone(), contract.clone(), function)?,
            center,
            1,
        )?;
    }
    Ok(())
}
