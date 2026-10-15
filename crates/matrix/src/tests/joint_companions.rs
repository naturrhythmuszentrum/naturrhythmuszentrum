// SPDX-License-Identifier: AGPL-3.0-only
//! Private category and storage integration for statistical companion records.

use crate::{
    CompanionSetDraft, CovarianceBlock, Extent, Heap, HeapError, HeapLimits, JointLimits,
    JointModel, Source, SourceFactor, TargetMap,
};
use nrz_units::Descriptor;

/// Statistical factor occupations cannot be accessed as deterministic bounds or values.
/// ´test:crate:joint-regions-retain-category-separation´
#[test]
fn joint_regions_retain_category_separation() -> Result<(), Box<dyn std::error::Error>> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let value = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f64])?;
    let target = TargetMap::capture(&heap, value.value())?;
    let limits = JointLimits::default();
    let source = Source::new(&Descriptor::ratio(), 1)?;
    let model = JointModel::from_factor(
        &[source],
        SourceFactor::new(1, &[1.0], &[1.0], limits)?,
        &[CovarianceBlock::Known],
        limits,
    )?;
    let companion = heap.publish_joint(
        CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]]),
        limits,
    )?;
    let key = heap.joint.records[companion.index].regions[0];
    let bound = crate::Handle::<crate::Companion, f64>::new(heap.scope, key);
    let forged_value = crate::Handle::<crate::Value, f64>::new(heap.scope, key);
    assert!(matches!(
        heap.companion(bound),
        Err(HeapError::CategoryMismatch)
    ));
    assert!(matches!(
        heap.value(forged_value),
        Err(HeapError::CategoryMismatch)
    ));
    Ok(())
}
