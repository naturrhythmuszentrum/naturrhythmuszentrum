// SPDX-License-Identifier: AGPL-3.0-only
//! Local nonlinear moments use replayed derivative laws and independent rational truth.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use crate::statistics_testing::{check, covariance, vector, worked_covariance};
use nrz_matrix::{
    Elementwise, Heap, HeapLimits, JointLimits, MomentAssurance, OperationDraft, ShapeLimits,
};
use nrz_scalar::Scalar;

/// The square worked example preserves off-diagonal covariance and separate derivative errors.
/// ´test:integration:square-worked-covariance-is-only-linearized-moments´
#[test]
fn square_worked_covariance_is_only_linearized_moments() -> TestResult {
    square::<f32>()?;
    square::<f64>()
}
/// Run both formats against exact integer-fraction matrix contractions.
fn square<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let column = axis(1)?;
    let input = shape(&declarations, &column, &coordinates)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<P>(&mut heap, true)?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::map_elementwise(input.clone(), input.clone(), Elementwise::Square)?,
        vec![source.value()],
    )?)?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    assert_eq!(moments.assurance(), MomentAssurance::LinearizedMoments);
    assert!(moments.means().is_none());
    let proof = moments
        .checked_sensitivity()
        .ok_or("checked symbolic derivative")?;
    assert_eq!(proof.recipe(), nrz_units::SensitivityRecipe::Square);
    assert!(!proof.jacobian().witnesses().is_empty());
    check(
        moments.covariance(),
        &covariance(&[4, 0, 0, 6], 2, &[4, 0, 0, 6], 2, 2, &worked_covariance()?)?,
    )?;
    check(
        moments.jacobian().ok_or("Jacobian")?,
        &[
            Rational::new(4, 1)?,
            Rational::new(0, 1)?,
            Rational::new(0, 1)?,
            Rational::new(6, 1)?,
        ],
    )?;
    let cross =
        heap.materialize_cross_covariance(result.value(), source.value(), JointLimits::default())?;
    assert_eq!(
        heap.moments(cross)?.assurance(),
        MomentAssurance::LinearizedMoments
    );
    check(
        heap.moments(cross)?.covariance(),
        &covariance(&[4, 0, 0, 6], 2, &[1, 0, 0, 1], 2, 2, &worked_covariance()?)?,
    )?;
    let copied = output(&execute(
        &mut heap,
        OperationDraft::copy(vec![input.clone()]),
        vec![result.value()],
    )?)?;
    assert_eq!(
        heap.moments(heap.moment_companion(copied.value())?)?
            .assurance(),
        MomentAssurance::LinearizedMoments
    );
    let summed = output(&execute(
        &mut heap,
        OperationDraft::add(input.clone(), input, ShapeLimits::default())?,
        vec![result.value(), copied.value()],
    )?)?;
    check(
        heap.moments(heap.moment_companion(summed.value())?)?
            .covariance(),
        &covariance(
            &[8, 0, 0, 12],
            2,
            &[8, 0, 0, 12],
            2,
            2,
            &worked_covariance()?,
        )?,
    )?;
    for (index, exact) in [4, 9].into_iter().enumerate() {
        let (center, radius, error) = entry(&heap, result, index)?;
        assert!(Rational::new(exact, 1)?.covered(center, error)?);
        assert!(Rational::binary(radius)?.at_most(Rational::new(1, 1000)?)?);
    }
    Ok(())
}

/// Finite exact input moments do not make the linearized square variance the actual variance.
/// ´test:integration:nonlinear-nominal-center-is-not-an-expectation´
#[test]
fn nonlinear_nominal_center_is_not_an_expectation() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let input = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(
        nrz_matrix::Extent::new(1, 1)?,
        &nrz_units::Descriptor::ratio(),
        &[0.0_f64],
    )?;
    crate::statistics_testing::attach(&mut heap, source.value(), &[2.0 / 3.0], true)?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::map_elementwise(input.clone(), input, Elementwise::Square)?,
        vec![source.value()],
    )?)?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    assert_eq!(moments.assurance(), MomentAssurance::LinearizedMoments);
    assert!(moments.means().is_none());
    check(moments.covariance(), &[Rational::new(0, 1)?])?;
    let mean = Rational::new(1, 3)?.add(Rational::new(1, 3)?)?;
    let variance = mean.subtract(mean.multiply(mean)?)?;
    assert!(Rational::new(2, 9)?.at_most(variance)?);
    assert!(!variance.covered(
        moments.covariance().get(0, 0)?,
        moments.covariance().error(0, 0)?
    )?);
    Ok(())
}
