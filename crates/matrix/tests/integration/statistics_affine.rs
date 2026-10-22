// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine moments, complete cross blocks and source-preserving public lifecycles.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use crate::statistics_testing::{check, covariance, vector, worked_covariance};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    FixedCoefficient, Heap, HeapLimits, JointLimits, LinearMap, MomentAssurance, OperationDraft,
    ShapeLimits,
};
use nrz_scalar::{LiteralLimits, Scalar};

/// The worked affine map propagates means, every covariance entry and the output/input cross block.
/// ´test:integration:affine-worked-moments-retain-complete-cross-blocks´
#[test]
fn affine_worked_moments_retain_complete_cross_blocks() -> TestResult {
    worked::<f32>()?;
    worked::<f64>()
}
/// Execute the exact same mathematical fixture in each sealed format.
fn worked<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let column = axis(1)?;
    let input = shape(&declarations, &column, &coordinates)?;
    let operator = shape(&declarations, &coordinates, &coordinates)?;
    let coefficients = [1.0, -1.0, 2.0, 1.0]
        .into_iter()
        .map(|value| FixedCoefficient::represented(Descriptor::ratio(), value))
        .collect::<Result<_, _>>()?;
    let map = LinearMap::new(
        operator,
        input.clone(),
        coefficients,
        Vec::new(),
        LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<P>(&mut heap, true)?;
    let original = heap
        .statistical(
            *heap
                .joint_companions(source.value())?
                .first()
                .ok_or("source companion")?,
        )?
        .model()
        .id();
    let transformed = output(&execute(
        &mut heap,
        OperationDraft::linear_map(map),
        vec![source.value()],
    )?)?;
    let identity = heap.moment_companion(transformed.value())?;
    let moments = heap.moments(identity)?;
    assert_eq!(moments.model(), original);
    assert_eq!(moments.assurance(), MomentAssurance::AffineMoments);
    let expected = covariance(
        &[1, -1, 2, 1],
        2,
        &[1, -1, 2, 1],
        2,
        2,
        &worked_covariance()?,
    )?;
    check(moments.covariance(), &expected)?;
    check(
        moments.means().ok_or("affine means")?,
        &[Rational::new(8, 1)?, Rational::new(1, 1)?],
    )?;
    check(
        moments.jacobian().ok_or("Jacobian")?,
        &[
            Rational::new(1, 1)?,
            Rational::new(-1, 1)?,
            Rational::new(2, 1)?,
            Rational::new(1, 1)?,
        ],
    )?;
    assert_eq!(
        moments.targets(),
        [heap.value(transformed.value())?.stamp(); 2]
    );
    let parameter = moments.covariance().error_target();
    assert!(matches!(
        heap.companion(parameter),
        Err(nrz_matrix::HeapError::CategoryMismatch)
    ));
    let cross = heap.materialize_cross_covariance(
        transformed.value(),
        source.value(),
        JointLimits::default(),
    )?;
    check(
        heap.moments(cross)?.covariance(),
        &covariance(
            &[1, -1, 2, 1],
            2,
            &[1, 0, 0, 1],
            2,
            2,
            &worked_covariance()?,
        )?,
    )?;
    for (index, nominal) in [8, 1].into_iter().enumerate() {
        let (center, radius, error) = entry(&heap, transformed, index)?;
        assert!(Rational::new(nominal, 1)?.covered(center, error)?);
        assert!(Rational::binary(radius)?.at_most(Rational::new(1, 1000)?)?);
    }
    repeated_and_stale(&mut heap, source, transformed, input, identity, cross)
}
/// Repetition, further arithmetic, compaction and source replacement preserve the same relation.
fn repeated_and_stale<P: Scalar + 'static>(
    heap: &mut Heap,
    source: nrz_matrix::PublishedBundle<P>,
    transformed: nrz_matrix::PublishedBundle<P>,
    input: nrz_matrix::CheckedShape,
    identity: nrz_matrix::MomentRef<P>,
    cross: nrz_matrix::MomentRef<P>,
) -> TestResult {
    let doubled = output(&execute(
        heap,
        OperationDraft::add(input.clone(), input, ShapeLimits::default())?,
        vec![source.value(), source.value()],
    )?)?;
    let expected = covariance(&[2, 0, 0, 2], 2, &[2, 0, 0, 2], 2, 2, &worked_covariance()?)?;
    check(
        heap.moments(heap.moment_companion(doubled.value())?)?
            .covariance(),
        &expected,
    )?;
    heap.compact()?;
    assert_eq!(heap.moments(identity)?.covariance().extent().entries(), 4);
    assert!(heap.moments(cross).is_ok());
    let version = heap.value(source.value())?.stamp().version();
    heap.replace_represented(source.value(), version, &[P::from_u8(2), P::from_u8(3)])?;
    assert!(heap.moments(identity).is_err());
    assert!(heap.moments(cross).is_err());
    assert!(heap.value(transformed.value()).is_err());
    assert!(heap.value(doubled.value()).is_err());
    Ok(())
}

/// Replacing an intermediate value invalidates direct descendant moment access even when original K roots are unchanged.
/// ´test:integration:direct-moment-access-validates-intermediate-provenance´
#[test]
fn direct_moment_access_validates_intermediate_provenance() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = crate::statistics_testing::scalar::<f64>(&mut heap)?;
    let first = output(&execute(
        &mut heap,
        OperationDraft::copy(vec![contract.clone()]),
        vec![source.value()],
    )?)?;
    let second = output(&execute(
        &mut heap,
        OperationDraft::copy(vec![contract]),
        vec![first.value()],
    )?)?;
    let moments = heap.moment_companion(second.value())?;
    let stamp = heap.value(first.value())?.stamp();
    heap.replace_represented(first.value(), stamp.version(), &[2.0])?;
    assert!(heap.moments(moments).is_err());
    assert!(heap.value(source.value()).is_ok());
    Ok(())
}
