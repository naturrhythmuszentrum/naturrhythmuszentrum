// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical coverage is explicitly unavailable to deterministic square solves.

use crate::solve_testing::{TestResult, draft};
use nrz_matrix::{
    Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, Extent, Heap, HeapLimits,
    JointLimits, JointModel, OperationError, SolveError, Source, SourceFactor, TargetMap,
};
use nrz_units::Descriptor;

/// The typed statistical refusal names the physical operand before faer entry.
/// ´test:integration:square-solve-refuses-statistical-input-explicitly´
#[test]
fn square_solve_refuses_statistical_input_explicitly() -> TestResult {
    let quantity = Descriptor::ratio();
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_represented(Extent::new(1, 1)?, &quantity, &[2.0_f64])?;
    let target = TargetMap::capture(&heap, input.value())?;
    let source = Source::new(&quantity, 1)?;
    let limits = JointLimits::default();
    let model = JointModel::from_factor(
        std::slice::from_ref(&source),
        SourceFactor::new(1, &[1.0], &[0.25], limits)?,
        &[CovarianceBlock::Known],
        limits,
    )?;
    let companion = heap.publish_joint(
        CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]]),
        limits,
    )?;
    let execution = ExecutionLimits::default();
    assert!(matches!(
        Binding::new(
            draft(1, 1)?.check(execution)?,
            vec![input.value(); 2],
            execution
        )
        .validate(&mut heap),
        Err(OperationError::Solve(SolveError::StatisticalInput {
            slot: 0
        }))
    ));
    assert!(heap.statistical(companion).is_ok());
    Ok(())
}
