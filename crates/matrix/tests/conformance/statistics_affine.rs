// SPDX-License-Identifier: AGPL-3.0-only
//! Exact selections, signed cancellation and missing-source refusals at the public boundary.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, execute, output, owner, shape};
use crate::statistics_testing::{check, covariance, scalar, vector, worked_covariance};
use nrz_matrix::{EntryMap, Extent, Heap, HeapLimits, JointLimits, OperationDraft, ShapeLimits};

/// Repeated entry maps keep their full dependence and select the matching cross rows.
/// ´test:integration:statistical-entry-duplication-retains-full-covariance´
#[test]
fn statistical_entry_duplication_retains_full_covariance() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let column = axis(1)?;
    let input = shape(&declarations, &column, &coordinates)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<f64>(&mut heap, false)?;
    let map = EntryMap::new(
        input.extent(),
        Extent::new(2, 1)?,
        vec![1, 1],
        ShapeLimits::default(),
    )?;
    let draft =
        OperationDraft::transform_entries(input, column, coordinates, map, ShapeLimits::default())?;
    let result = output(&execute(&mut heap, draft, vec![source.value()])?)?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    assert!(moments.means().is_none());
    check(
        moments.covariance(),
        &covariance(&[0, 0, 1, 1], 2, &[0, 0, 1, 1], 2, 2, &worked_covariance()?)?,
    )?;
    let cross =
        heap.materialize_cross_covariance(result.value(), source.value(), JointLimits::default())?;
    check(
        heap.moments(cross)?.covariance(),
        &covariance(&[0, 0, 1, 1], 2, &[1, 0, 0, 1], 2, 2, &worked_covariance()?)?,
    )?;
    Ok(())
}

/// Statistical cancellation follows the common source; it does not cancel unrelated deterministic bounds.
/// ´test:integration:statistical-difference-preserves-common-source-cancellation´
#[test]
fn statistical_difference_preserves_common_source_cancellation() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let input = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = scalar::<f64>(&mut heap)?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::subtract(input.clone(), input, ShapeLimits::default())?,
        vec![source.value(), source.value()],
    )?)?;
    let view = heap.moments(heap.moment_companion(result.value())?)?;
    crate::statistics_testing::nonnegative_variances(view.covariance())?;
    check(view.covariance(), &[Rational::new(0, 1)?])?;
    Ok(())
}

/// A matrix value retains a full covariance over all logical entries, including cross columns.
/// ´test:integration:matrix-entry-transpose-propagates-full-covariance´
#[test]
fn matrix_entry_transpose_propagates_full_covariance() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let input = shape(&declarations, &coordinates, &coordinates)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(
        Extent::new(2, 2)?,
        &nrz_units::Descriptor::ratio(),
        &[2.0_f64, 3.0, 4.0, 5.0],
    )?;
    let limits = JointLimits::default();
    let sources = (0..4)
        .map(|_| nrz_matrix::Source::new(&nrz_units::Descriptor::ratio(), 1))
        .collect::<Result<Vec<_>, _>>()?;
    let model = nrz_matrix::JointModel::from_factor(
        &sources,
        nrz_matrix::SourceFactor::new(4, &[1.0, 2.0, -1.0, 3.0], &[0.25], limits)?,
        &vec![nrz_matrix::CovarianceBlock::Known; 16],
        limits,
    )?;
    let target = nrz_matrix::TargetMap::capture(&heap, source.value())?;
    heap.publish_joint(
        nrz_matrix::CompanionSetDraft::new(
            model,
            [target.clone(), target],
            [vec![0, 1, 2, 3], vec![0, 1, 2, 3]],
        ),
        limits,
    )?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::dual_transpose(input, ShapeLimits::default())?,
        vec![source.value()],
    )?)?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    assert_eq!(moments.covariance().extent(), Extent::new(4, 4)?);
    check(
        moments.covariance(),
        &covariance(
            &[1, -1, 2, 3],
            4,
            &[1, -1, 2, 3],
            4,
            1,
            &[Rational::new(1, 4)?],
        )?,
    )?;
    Ok(())
}

/// An empty full covariance is still a paired parameter, independently of optional empty means.
/// ´test:integration:empty-statistical-selection-retains-parameter-roles´
#[test]
fn empty_statistical_selection_retains_parameter_roles() -> crate::kernel_testing::TestResult {
    let declarations = crate::kernel_testing::owner()?;
    let coordinates = crate::kernel_testing::axis(2)?;
    let column = crate::kernel_testing::axis(1)?;
    let input_shape = crate::kernel_testing::shape(&declarations, &column, &coordinates)?;
    let mut heap = nrz_matrix::Heap::new(nrz_matrix::HeapLimits::default())?;
    let source = crate::statistics_testing::vector::<f64>(&mut heap, true)?;
    let limits = nrz_matrix::ShapeLimits::default();
    let map = nrz_matrix::EntryMap::new(
        input_shape.extent(),
        nrz_matrix::Extent::new(0, 1)?,
        Vec::new(),
        limits,
    )?;
    let draft = nrz_matrix::OperationDraft::transform_entries(
        input_shape,
        column,
        crate::kernel_testing::axis(0)?,
        map,
        limits,
    )?;
    let result = crate::kernel_testing::output(&crate::kernel_testing::execute(
        &mut heap,
        draft,
        vec![source.value()],
    )?)?;
    let view = heap.moments(heap.moment_companion(result.value())?)?;
    assert_eq!(view.covariance().extent(), nrz_matrix::Extent::new(0, 0)?);
    assert!(view.covariance().mean_descriptor().is_none());
    let means = view.means().ok_or("empty declared means")?;
    assert_eq!(means.extent(), nrz_matrix::Extent::new(0, 1)?);
    assert_eq!(
        means.mean_descriptor(),
        Some(&nrz_units::Descriptor::ratio())
    );
    Ok(())
}
