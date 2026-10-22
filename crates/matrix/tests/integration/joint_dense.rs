// SPDX-License-Identifier: AGPL-3.0-only
//! Dense parameters, numerical error targets and cross-value lineage lifecycles.

use crate::testing::TestResult;
use nrz_bindings::{Descriptor, PairingKind, UncertaintyMode};
use nrz_matrix::{
    CompanionSetDraft, CovarianceBlock, DenseCovariance, Extent, Heap, HeapError, HeapLimits,
    JointError, JointLimits, JointModel, Source, TargetMap,
};
use nrz_scalar::{ArithmeticContext, Enclosure};

/// A covariance error region targets product parameters and cannot resolve as a physical bound.
/// ´test:integration:covariance-errors-target-parameters´
#[test]
fn covariance_errors_target_parameters() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let quantity = crate::testing::voltage()?;
    let value = heap.import_represented(Extent::new(1, 1)?, &quantity, &[3.0_f64])?;
    let limits = JointLimits::default();
    let context = ArithmeticContext::new()?;
    let source = Source::new(&quantity, 1)?;
    let dense = DenseCovariance::with_enclosures(
        1,
        &[1.0],
        &[Enclosure::new(0.875, 1.125)?],
        &context,
        limits,
    )?;
    let model = JointModel::from_dense(&[source], dense, &[CovarianceBlock::Known], limits)?;
    let target = TargetMap::capture(&heap, value.value())?;
    let draft = CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]])
        .with_means(
            &heap,
            [
                vec![Enclosure::new(2.75, 2.875)?],
                vec![Enclosure::new(2.75, 2.875)?],
            ],
            &context,
            limits,
        )?;
    let companion = heap.publish_joint(draft, limits)?;
    let view = heap.statistical(companion)?;
    assert!(view.factor_region().is_none());
    assert_eq!(
        view.covariance_region()
            .ok_or("missing covariance")?
            .get(0, 0)?
            .to_bits(),
        1.0_f64.to_bits()
    );
    let error = view.parameter_errors().ok_or("missing error")?;
    assert!(error.get(0, 0)? >= 0.125);
    assert_eq!(
        error.descriptors()[0].target().kind(),
        PairingKind::Uncertainty(UncertaintyMode::Covariance)
    );
    assert_ne!(error.descriptors()[0].dimension(), quantity.dimension());
    assert!(view.means().is_some());
    assert!(view.mean_error_regions()[0].get(0, 0)? >= 0.25);
    assert!(heap.value(value.value())?.bound().is_none());
    Ok(())
}

/// Two separately retained outputs select the same declared source and retain cross dependence.
/// ´test:integration:cross-value-lineage-retains-common-source´
#[test]
fn cross_value_lineage_retains_common_source() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let quantity = Descriptor::ratio();
    let first = heap.import_represented(Extent::new(1, 1)?, &quantity, &[2.0_f32])?;
    let second = heap.import_represented(Extent::new(1, 1)?, &quantity, &[4.0_f32])?;
    let limits = JointLimits::default();
    let source = Source::new(&quantity, 3)?;
    let dense = DenseCovariance::new(1, &[0.25_f32], &ArithmeticContext::new()?, limits)?;
    let model = JointModel::from_dense(&[source], dense, &[CovarianceBlock::Known], limits)?;
    let first_target = TargetMap::capture(&heap, first.value())?;
    let left = heap.publish_joint(
        CompanionSetDraft::new(
            model.clone(),
            [first_target.clone(), first_target],
            [vec![0], vec![0]],
        ),
        limits,
    )?;
    let second_target = TargetMap::capture(&heap, second.value())?;
    let right = heap.publish_joint(
        CompanionSetDraft::new(
            model,
            [second_target.clone(), second_target],
            [vec![0], vec![0]],
        ),
        limits,
    )?;
    let lineage = heap.cross_lineage(left, right, limits)?;
    assert_eq!(lineage.left_sources(), [0]);
    assert_eq!(lineage.right_sources(), [0]);
    assert_ne!(lineage.targets()[0], lineage.targets()[1]);
    assert!(heap.statistical(left)?.means().is_none());
    heap.compact()?;
    heap.cross_lineage(left, right, limits)?;
    let version = heap.value(first.value())?.stamp().version();
    heap.replace_represented(first.value(), version, &[2.0])?;
    assert!(matches!(
        heap.cross_lineage(left, right, limits),
        Err(JointError::Heap(HeapError::StaleEvidence))
    ));
    Ok(())
}
