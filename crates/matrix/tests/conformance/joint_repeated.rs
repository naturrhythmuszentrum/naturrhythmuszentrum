// SPDX-License-Identifier: AGPL-3.0-only
//! Repeated physical selections retain one source coordinate from initial admission.

use crate::testing::TestResult;
use nrz_bindings::Descriptor;
use nrz_matrix::{
    CompanionSetDraft, CovarianceBlock, Extent, Heap, HeapLimits, JointError, JointLimits,
    JointModel, Layout, Source, SourceFactor, TargetMap,
};

/// A repeated entry cannot acquire two source coordinates, even with identical schemas.
/// ´test:integration:repeated-target-cannot-invent-sources´
#[test]
fn repeated_target_cannot_invent_sources() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let quantity = Descriptor::ratio();
    let value = heap.import_represented(Extent::new(1, 1)?, &quantity, &[2.0_f64])?;
    let repeated = heap.alias(
        value.value(),
        &Layout::gather(Extent::new(2, 1)?, 1, &[0, 0], 2)?,
    )?;
    let target = TargetMap::capture(&heap, repeated)?;
    let limits = JointLimits::default();
    let sources = [Source::new(&quantity, 1)?, Source::new(&quantity, 1)?];
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(2, &[1.0, 2.0], &[1.0], limits)?,
        &vec![CovarianceBlock::Known; 4],
        limits,
    )?;
    let invalid = CompanionSetDraft::new(
        model.clone(),
        [target.clone(), target.clone()],
        [vec![0, 1], vec![0, 1]],
    );
    assert!(matches!(
        heap.publish_joint(invalid, limits),
        Err(JointError::SourceConflict)
    ));
    let valid = CompanionSetDraft::new(model, [target.clone(), target], [vec![0, 0], vec![0, 0]]);
    let companion = heap.publish_joint(valid, limits)?;
    assert_eq!(
        heap.statistical(companion)?.source_maps(),
        &[vec![0, 0], vec![0, 0]]
    );
    Ok(())
}
