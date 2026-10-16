// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical support must satisfy whole-family structure independently of the nominal center.

use crate::kernel_testing::{TestResult, axis, owner};
use nrz_matrix::{
    Binding, BlockDraft, CompanionSetDraft, CovarianceBlock, EntryDraft, ExecutionLimits, Extent,
    Heap, HeapLimits, JointLimits, JointModel, OperationDraft, OperationError, ShapeDraft,
    ShapeLimits, Source, SourceFactor, StructureScope, TargetMap,
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite};
use nrz_units::Descriptor;

/// A zero center permits varying center-only statistics, but a whole-family zero needs exact zero moments.
/// ´test:integration:statistical-structure-distinguishes-center-and-family-zeros´
#[test]
fn statistical_structure_distinguishes_center_and_family_zeros() -> TestResult {
    for (scope, weight, admitted) in [
        (StructureScope::WholeFamily, 0.25, false),
        (StructureScope::WholeFamily, 0.0, true),
        (StructureScope::CenterOnly, 0.25, true),
    ] {
        structured_zero(scope, weight, admitted)?;
    }
    Ok(())
}
/// The same exact mean declaration isolates the variation obligation from center storage.
fn structured_zero(scope: StructureScope, weight: f64, admitted: bool) -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = ShapeDraft::new(
        coordinate.clone(),
        coordinate,
        vec![BlockDraft::new(
            0..1,
            0..1,
            vec![EntryDraft::Zero(Descriptor::ratio())],
        )],
        scope,
    )
    .check(declarations, ShapeLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[0.0_f64])?;
    let target = TargetMap::capture(&heap, input.value())?;
    let limits = JointLimits::default();
    let source = Source::new(&Descriptor::ratio(), 1)?;
    let model = JointModel::from_factor(
        &[source],
        SourceFactor::new(1, &[1.0], &[weight], limits)?,
        &[CovarianceBlock::Known],
        limits,
    )?;
    let means = vec![Enclosure::singleton(Finite::ZERO)];
    let draft = CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]])
        .with_means(
            &heap,
            [means.clone(), means],
            &ArithmeticContext::new()?,
            limits,
        )?;
    heap.publish_joint(draft, limits)?;
    let operation = OperationDraft::copy(vec![contract]).check(ExecutionLimits::default())?;
    let bound = Binding::new(operation, vec![input.value()], ExecutionLimits::default())
        .validate(&mut heap);
    if admitted {
        let bound = bound?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let output = bound.publish(candidate)?.into_output();
        let result = output.first().ok_or("structured output")?;
        let view = heap.moments(heap.moment_companion(result.value())?)?;
        assert!(
            crate::kernel_oracle::Rational::binary(weight)?
                .covered(view.covariance().get(0, 0)?, view.covariance().error(0, 0)?)?
        );
    } else {
        assert!(matches!(
            bound,
            Err(OperationError::StructureMismatch { slot: 0, entry: 0 })
        ));
    }
    Ok(())
}
