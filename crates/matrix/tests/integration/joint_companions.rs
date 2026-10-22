// SPDX-License-Identifier: AGPL-3.0-only
//! Source-preserving selections, concrete two-sided targets and stale evidence.

use crate::testing::TestResult;
use nrz_bindings::Descriptor;
use nrz_matrix::{
    CompanionSetDraft, CovarianceBlock, Extent, Heap, HeapError, HeapLimits, JointError,
    JointLimits, JointModel, Layout, Source, SourceFactor, TargetMap,
};

/// Common sources retain dependence across distinct center regions and compaction.
/// ´test:integration:joint-targets-survive-compaction-and-refuse-staleness´
#[test]
fn joint_targets_survive_compaction_and_refuse_staleness() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let quantity = Descriptor::ratio();
    let first = heap.import_represented(Extent::new(2, 1)?, &quantity, &[2.0_f64, 3.0])?;
    let bytes = heap.value(first.value())?.region().export()?;
    let copied = heap.import_represented(Extent::new(2, 1)?, &quantity, &bytes)?;
    let targets = [
        TargetMap::capture(&heap, first.value())?,
        TargetMap::capture(&heap, copied.value())?,
    ];
    let sources = [Source::new(&quantity, 1)?, Source::new(&quantity, 1)?];
    let limits = JointLimits::default();
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(2, &[1.0, 2.0], &[0.25], limits)?,
        &vec![CovarianceBlock::Known; 4],
        limits,
    )?;
    let companion = heap.publish_joint(
        CompanionSetDraft::new(model, targets, [vec![0, 1], vec![0, 1]]),
        limits,
    )?;
    assert_eq!(
        heap.statistical(companion)?.source_maps(),
        &[vec![0, 1], vec![0, 1]]
    );
    assert_eq!(heap.joint_companions(first.value())?.len(), 1);
    let binding = nrz_matrix::Binding::new(
        crate::binding_testing::operation(2, 1)?,
        vec![first.value()],
        nrz_matrix::ExecutionLimits::default(),
    );
    let bound = binding.validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let result = outcome.output().first().ok_or("copied moments")?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    crate::statistics_testing::check(
        moments.covariance(),
        &[
            crate::kernel_oracle::Rational::new(1, 4)?,
            crate::kernel_oracle::Rational::new(1, 2)?,
            crate::kernel_oracle::Rational::new(1, 2)?,
            crate::kernel_oracle::Rational::new(1, 1)?,
        ],
    )?;
    assert_eq!(moments.source_indices(), [0, 1]);
    heap.compact()?;
    assert_eq!(
        heap.statistical(companion)?
            .factor_region()
            .ok_or("missing factor")?
            .export()?,
        [1.0, 2.0]
    );
    let version = heap.value(first.value())?.stamp().version();
    heap.replace_represented(first.value(), version, &[5.0, 6.0])?;
    assert!(matches!(
        heap.statistical(companion),
        Err(JointError::Heap(HeapError::StaleEvidence))
    ));
    assert!(matches!(
        heap.value(copied.value()),
        Err(HeapError::StaleEvidence)
    ));
    Ok(())
}

/// Equal extents and units cannot authorize a different target order.
/// ´test:integration:joint-target-order-is-exact´
#[test]
fn joint_target_order_is_exact() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let value =
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0_f32, 2.0])?;
    let map = TargetMap::capture(&heap, value.value())?;
    let reversed = heap.alias(
        value.value(),
        &Layout::gather(Extent::new(2, 1)?, 2, &[1, 0], 2)?,
    )?;
    assert!(matches!(
        map.validate(&heap, reversed),
        Err(JointError::WrongTarget)
    ));
    map.validate(&heap, value.value())?;
    Ok(())
}
