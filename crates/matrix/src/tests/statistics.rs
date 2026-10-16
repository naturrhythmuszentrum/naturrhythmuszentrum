// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical append integrates with the existing descriptor and typed storage reservations.

use crate::kernel_testing::{TestResult, axis, execute, output, owner, shape};
use crate::{
    CompanionSetDraft, CovarianceBlock, Extent, Heap, HeapLimits, JointLimits, JointModel,
    OperationDraft, ShapeLimits, Source, SourceFactor, TargetMap,
};
use nrz_units::Descriptor;

/// Appended covariance descriptors cannot replace the reserved value/total/evaluation prefix.
/// ´test:crate:statistical-reservations-preserve-the-value-prefix´
#[test]
fn statistical_reservations_preserve_the_value_prefix() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f32])?;
    let target = TargetMap::capture(&heap, input.value())?;
    let limits = JointLimits::default();
    let source = Source::new(&Descriptor::ratio(), 1)?;
    let model = JointModel::from_factor(
        &[source],
        SourceFactor::new(1, &[1.0], &[0.25], limits)?,
        &[CovarianceBlock::Known],
        limits,
    )?;
    heap.publish_joint(
        CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]]),
        limits,
    )?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?,
        vec![input.value(); 2],
    )?)?;
    let reference = heap.moment_companion(result.value())?;
    let record = heap
        .statistics
        .records
        .get(reference.index)
        .ok_or("missing internal record")?;
    assert_eq!(record.targets[0].key, result.value().key);
    assert_eq!(
        record.targets[0].stamp(),
        heap.value(result.value())?.stamp()
    );
    for key in record.covariance.regions {
        assert!(matches!(
            heap.descriptors.get(key)?.meaning,
            crate::heap::records::Meaning::Statistical
        ));
        assert_ne!(key, result.value().key);
        assert_ne!(Some(key), result.bound().map(|handle| handle.key));
    }
    let view = heap.moments(reference)?;
    assert!(crate::kernel_oracle::Rational::new(1, 1)?.covered(
        f64::from(view.covariance().get(0, 0)?),
        f64::from(view.covariance().error(0, 0)?)
    )?);
    Ok(())
}

/// Hundreds of real publications traverse one bounded graph without recursive reentry.
/// ´test:crate:deep-statistical-provenance-obeys-inspection-budget´
#[test]
fn deep_statistical_provenance_obeys_inspection_budget() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut value =
        heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let target = TargetMap::capture(&heap, value.value())?;
    let limits = JointLimits::default();
    let source = Source::new(&Descriptor::ratio(), 1)?;
    let model = JointModel::from_factor(
        &[source],
        SourceFactor::new(1, &[1.0], &[0.25], limits)?,
        &[CovarianceBlock::Known],
        limits,
    )?;
    heap.publish_joint(
        CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]]),
        limits,
    )?;
    for _ in 0..512 {
        value = output(&execute(
            &mut heap,
            OperationDraft::copy(vec![contract.clone()]),
            vec![value.value()],
        )?)?;
    }
    assert_eq!(heap.provenance_usage().depth(), 512);
    let reference = heap.moment_companion(value.value())?;
    let stamp = heap.value(value.value())?.stamp();
    // Each lookup scans 512 propagated records and one K record, then checks
    // the matching root index, its two targets and its two parameter regions.
    let required = 512 + 1 + 1 + 2 + 2;
    heap.limits.layout_work = required - 1;
    assert!(matches!(
        heap.validate_statistical_roots(stamp),
        Err(crate::HeapError::WorkLimit)
    ));
    heap.limits.layout_work = required;
    heap.validate_statistical_roots(stamp)?;
    let view = heap.moments(reference)?;
    assert!(
        crate::kernel_oracle::Rational::new(1, 4)?
            .covered(view.covariance().get(0, 0)?, view.covariance().error(0, 0)?)?
    );
    Ok(())
}
