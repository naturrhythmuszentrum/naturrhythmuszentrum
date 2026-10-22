// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational solve differential and common-operator covariance oracles in both precisions.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{execute, output};
use crate::solve_testing::{TestResult, draft};
use crate::statistics_testing::check;
use nrz_matrix::{Heap, HeapLimits, JointLimits, MomentAssurance, RecipeEvidence};
use nrz_scalar::Scalar;

/// Shared operator fluctuations correlate exact RHS columns; shared A/B fluctuations retain cross terms.
/// ´test:integration:solve-common-operator-covariance-matches-rational-oracle´
#[test]
fn solve_common_operator_covariance_matches_rational_oracle() -> TestResult {
    common::<f32>()?;
    common::<f64>()
}
/// Three independent exact scenarios separate fixed-A affine moments from nonlinear covariance.
fn common<P: Scalar + 'static>() -> TestResult {
    let zero = P::ZERO;
    let one = P::ONE;
    for (coefficients, covariance, assurance) in [
        (
            [one, zero, zero],
            [(1, 64), (1, 32), (1, 32), (1, 16)],
            MomentAssurance::LinearizedMoments,
        ),
        (
            [one, P::from_u8(3), one],
            [(1, 16), (-1, 32), (-1, 32), (1, 64)],
            MomentAssurance::LinearizedMoments,
        ),
        (
            [zero, one, P::from_u8(2)],
            [(1, 64), (1, 32), (1, 32), (1, 16)],
            MomentAssurance::AffineMoments,
        ),
    ] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let [operator, rhs] =
            crate::solve_statistics_testing::common(&mut heap, &coefficients, true)?;
        let outcome = execute(&mut heap, draft(1, 2)?, vec![operator.value(), rhs.value()])?;
        let result = output(&outcome)?;
        crate::solve_public::check(&heap, result, &[2.0], &[2.0, 4.0], 1)?;
        let expected = covariance
            .into_iter()
            .map(|(n, d)| Rational::new(n, d))
            .collect::<TestResult<Vec<_>>>()?;
        inspect(&heap, &outcome, assurance, &expected)?;
        let cross = heap.materialize_cross_covariance(
            result.value(),
            result.value(),
            JointLimits::default(),
        )?;
        check(heap.moments(cross)?.covariance(), &expected)?;
    }
    Ok(())
}

/// Inspect the complete derivative, statistical law and retained per-direction certificates.
fn inspect<P: Scalar + 'static>(
    heap: &Heap,
    outcome: &nrz_matrix::Outcome<Vec<nrz_matrix::PublishedBundle<P>>>,
    assurance: MomentAssurance,
    expected: &[Rational],
) -> TestResult {
    let result = output(outcome)?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    assert_eq!(moments.assurance(), assurance);
    check(moments.covariance(), expected)?;
    let jacobian = [(-1, 2), (-1, 1), (1, 2), (0, 1), (0, 1), (1, 2)]
        .into_iter()
        .map(|(n, d)| Rational::new(n, d))
        .collect::<TestResult<Vec<_>>>()?;
    check(moments.jacobian().ok_or("Jacobian")?, &jacobian)?;
    assert!(moments.checked_sensitivity().is_some());
    if assurance == MomentAssurance::AffineMoments {
        check(
            moments.means().ok_or("means")?,
            &[Rational::new(1, 1)?, Rational::new(2, 1)?],
        )?;
    } else {
        assert!(moments.means().is_none());
    }
    let record = heap.operation_record(outcome.record())?;
    let Some(RecipeEvidence::Solve(evidence)) = record.evidence().first() else {
        return Err("solve proof".into());
    };
    assert_eq!(evidence.sensitivities().len(), 3);
    for (direction, proof) in evidence.sensitivities().iter().enumerate() {
        assert_eq!(proof.direction(), direction);
        assert!(proof.residual_norm() >= 0.0 && proof.radius() >= 0.0);
    }
    Ok(())
}

/// A statistical inverse differentiates the exact typed-identity solve without adding identity sources.
/// ´test:integration:statistical-inverse-has-certified-rational-derivative´
#[test]
fn statistical_inverse_has_certified_rational_derivative() -> TestResult {
    inverse::<f32>()?;
    inverse::<f64>()
}
/// Both formats contain the exact inverse derivative and its variance.
fn inverse<P: Scalar + 'static>() -> TestResult {
    let declarations = crate::kernel_testing::owner()?;
    let variables = crate::kernel_testing::axis(1)?;
    let equations = crate::kernel_testing::axis(1)?;
    let operator = crate::kernel_testing::shape(&declarations, &variables, &equations)?;
    let solution = crate::kernel_testing::shape(&declarations, &equations, &variables)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = crate::statistics_testing::scalar::<P>(&mut heap)?;
    let outcome = execute(
        &mut heap,
        nrz_matrix::OperationDraft::inverse(
            operator,
            solution,
            nrz_matrix::ShapeLimits::default(),
        )?,
        vec![source.value()],
    )?;
    let result = output(&outcome)?;
    crate::solve_public::check(&heap, result, &[2.0], &[1.0], 1)?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    check(
        moments.jacobian().ok_or("Jacobian")?,
        &[Rational::new(-1, 4)?],
    )?;
    check(moments.covariance(), &[Rational::new(1, 64)?])?;
    assert_eq!(moments.assurance(), MomentAssurance::LinearizedMoments);
    Ok(())
}

/// A two-by-two operator checks every differential column and all common-source covariance blocks.
/// ´test:integration:matrix-solve-differentials-match-independent-elimination´
#[test]
fn matrix_solve_differentials_match_independent_elimination() -> TestResult {
    matrix::<f32>()?;
    matrix::<f64>()
}
/// Both public representations contain the independently eliminated exact differential.
fn matrix<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = matrix_inputs::<P>(&mut heap)?;
    let outcome = execute(&mut heap, draft(2, 2)?, inputs)?;
    let result = output(&outcome)?;
    crate::solve_public::check(
        &heap,
        result,
        &[3.0, 1.0, 1.0, 2.0],
        &[5.0, 5.0, 5.0, 0.0],
        2,
    )?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    let work = moments
        .checked_sensitivity()
        .ok_or("checked sensitivity")?
        .jacobian()
        .work();
    assert_eq!(work.requests, 96);
    assert!(work.construction <= nrz_bindings::Limits::default().work);
    let jacobian = matrix_jacobian()?;
    check(moments.jacobian().ok_or("Jacobian")?, &jacobian)?;
    check(
        moments.sensitivity().ok_or("source sensitivity")?,
        &jacobian,
    )?;
    check(moments.covariance(), &matrix_covariance(&jacobian)?)?;
    assert_eq!(moments.assurance(), MomentAssurance::LinearizedMoments);
    Ok(())
}
/// The rank-one joint model has nonzero operator/RHS cross blocks and no assumed independence.
fn matrix_inputs<P: Scalar + 'static>(
    heap: &mut Heap,
) -> TestResult<Vec<nrz_matrix::Handle<nrz_matrix::Value, P>>> {
    use nrz_matrix::{
        CompanionSetDraft, CovarianceBlock, Extent, JointModel, Source, SourceFactor, TargetMap,
    };
    let quantity = nrz_bindings::Descriptor::ratio();
    let one = P::ONE;
    let two = P::from_u8(2);
    let three = P::from_u8(3);
    let five = P::from_u8(5);
    let operator =
        heap.import_represented(Extent::new(2, 2)?, &quantity, &[three, one, one, two])?;
    let rhs =
        heap.import_represented(Extent::new(2, 2)?, &quantity, &[five, five, five, P::ZERO])?;
    let limits = JointLimits::default();
    let sources = (0..8)
        .map(|_| Source::new(&quantity, 1))
        .collect::<Result<Vec<_>, _>>()?;
    let factor = SourceFactor::new(
        8,
        &[one, P::ZERO, two, -one, three, one, two, P::ZERO],
        &[one / P::from_u8(16)],
        limits,
    )?;
    let model = JointModel::from_factor(
        &sources,
        factor,
        &std::array::from_fn::<_, 64, _>(|_| CovarianceBlock::Known),
        limits,
    )?;
    let targets = [
        TargetMap::capture(heap, operator.value())?,
        TargetMap::capture(heap, rhs.value())?,
    ];
    heap.publish_joint(
        CompanionSetDraft::new(model, targets, [vec![0, 1, 2, 3], vec![4, 5, 6, 7]]),
        limits,
    )?;
    Ok(vec![operator.value(), rhs.value()])
}
/// Exact integer right sides are eliminated with rational arithmetic, independently of production bounds.
fn matrix_jacobian() -> TestResult<Vec<Rational>> {
    let solution = [1.0, 2.0, 2.0, -1.0];
    let mut jacobian = Vec::new();
    for direction in 0..8 {
        let mut rhs = [0.0; 4];
        if direction < 4 {
            for column in 0..2 {
                rhs[direction % 2 + 2 * column] = -solution[direction / 2 + 2 * column];
            }
        } else {
            rhs[direction - 4] = 1.0;
        }
        jacobian.extend(crate::solve_public::exact(&[3.0, 1.0, 1.0, 2.0], &rhs, 2)?);
    }
    Ok(jacobian)
}
/// Contract the exact differential against the declared rank-one covariance using checked fractions only.
fn matrix_covariance(jacobian: &[Rational]) -> TestResult<Vec<Rational>> {
    let mut response = [Rational::new(0, 1)?; 4];
    for (direction, coefficient) in [1, 0, 2, -1, 3, 1, 2, 0].into_iter().enumerate() {
        for (row, value) in response.iter_mut().enumerate() {
            *value = value
                .add(jacobian[row + 4 * direction].multiply(Rational::new(coefficient, 1)?)?)?;
        }
    }
    let mut covariance = Vec::new();
    for column in response {
        for row in response {
            covariance.push(row.multiply(column)?.divide(Rational::new(16, 1)?)?);
        }
    }
    Ok(covariance)
}
