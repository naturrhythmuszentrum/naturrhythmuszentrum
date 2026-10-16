// SPDX-License-Identifier: AGPL-3.0-only
//! Every deterministic kernel refuses a carried statistical relation before arithmetic.

use crate::kernel_testing::{TestResult, axis, owner, shape};
use nrz_matrix::{
    Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, Extent, Heap, HeapError,
    HeapLimits, JointLimits, JointModel, OperationDraft, OperationError, ShapeLimits, Source,
    SourceFactor, TargetMap,
};
use nrz_units::Descriptor;

/// Statistical coverage is never silently replaced by the represented-only center problem.
/// ´test:integration:deterministic-sum-refuses-carried-statistics´
#[test]
fn deterministic_sum_refuses_carried_statistics() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let quantity = Descriptor::ratio();
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_represented(Extent::new(1, 1)?, &quantity, &[2.0_f64])?;
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
    let operation = OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?
        .check(ExecutionLimits::default())?;
    assert!(matches!(
        Binding::new(
            operation,
            vec![input.value(), input.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Heap(HeapError::MissingCoverage))
    ));
    assert!(heap.statistical(companion).is_ok());
    Ok(())
}
