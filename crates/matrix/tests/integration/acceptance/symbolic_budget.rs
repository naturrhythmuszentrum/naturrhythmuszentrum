// SPDX-License-Identifier: AGPL-3.0-only
//! The unreduced ladder exhausts witness replay under the integrated default symbolic policy.

#[path = "symbolic_budget/limits.rs"]
mod limits;

use super::{oracle, support::ResultOf};
use nrz_bindings::{Descriptor, SensitivityError};
use nrz_matrix::{
    Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, Extent, Handle, Heap, HeapLimits,
    JointLimits, JointModel, OperationError, Source, SourceFactor, StatisticsError, TargetMap,
    Value,
};

/// The full five-by-five system passes draft validation but exhausts default witness replay.
/// ´test:integration:integrated-full-ladder-retains-symbolic-work-refusal´
#[test]
fn integrated_full_ladder_retains_symbolic_work_refusal() -> ResultOf {
    let operator = [
        1.0, -1.0, 0.0, 0.0, 1.0, -1.0, 3.0, -1.0, 0.0, 0.0, 0.0, -1.0, 3.0, -1.0, 0.0, 0.0, 0.0,
        -1.0, 2.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
    ];
    let rhs = [0.0, 0.0, 0.0, 0.0, 1.0];
    let exact = crate::solve_public::exact(&operator, &rhs, 5)?;
    let expected = oracle::fractions(&[(1, 1), (5, 13), (2, 13), (1, 13), (-8, 13)])?;
    for (actual, expected) in exact.iter().zip(&expected) {
        assert_eq!(actual.subtract(*expected)?.numerator, 0);
    }
    let mut heap = Heap::new(HeapLimits::default())?;
    let matrix = heap.import_represented(Extent::new(5, 5)?, &Descriptor::ratio(), &operator)?;
    let load = heap.import_represented(Extent::new(5, 1)?, &Descriptor::ratio(), &rhs)?;
    attach(&mut heap, [matrix.value(), load.value()])?;
    let limits = ExecutionLimits::default();
    let operation = crate::solve_testing::draft(5, 1)?.check(limits)?;
    let cause = Binding::new(operation, vec![matrix.value(), load.value()], limits)
        .validate(&mut heap)
        .err()
        .ok_or("full sensitivity draft unexpectedly admitted")?;
    let OperationError::Statistics(StatisticsError::Sensitivity(cause)) = cause else {
        return Err("expected public sensitivity refusal".into());
    };
    let SensitivityError::Evidence(outcome) = cause.as_ref() else {
        return Err("expected symbolic witness replay refusal".into());
    };
    let nrz_bindings::readings::algebra::request::Outcome::Exhausted { fault, .. } =
        outcome.as_ref()
    else {
        return Err("expected exhausted symbolic replay".into());
    };
    assert_eq!(fault.location.operation, "typed witness DAG");
    assert_eq!(
        format!("{:?}", fault.kind),
        "Exhausted { reason: Limit, budget: ReplayWork, limit: 2000000, needed: 2000017 }"
    );
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}

/// One exact shared source suffices to trigger the same descriptor-only dense derivative schedule.
fn attach(heap: &mut Heap, inputs: [Handle<Value, f64>; 2]) -> ResultOf {
    let limits = JointLimits::default();
    let sources = (0..30)
        .map(|_| Source::new(&Descriptor::ratio(), 1))
        .collect::<Result<Vec<_>, _>>()?;
    let mut coefficients = vec![0.0; 30];
    *coefficients.last_mut().ok_or("source")? = 1.0;
    let blocks = vec![CovarianceBlock::Known; 900];
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(30, &coefficients, &[0.0625], limits)?,
        &blocks,
        limits,
    )?;
    let targets = [
        TargetMap::capture(heap, inputs[0])?,
        TargetMap::capture(heap, inputs[1])?,
    ];
    heap.publish_joint(
        CompanionSetDraft::new(model, targets, [(0..25).collect(), (25..30).collect()]),
        limits,
    )?;
    Ok(())
}
