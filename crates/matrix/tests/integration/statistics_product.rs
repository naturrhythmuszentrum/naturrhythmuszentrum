// SPDX-License-Identifier: AGPL-3.0-only
//! Full matrix-product covariance includes the joint cross blocks of all entries.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, execute, output, owner, shape};
use crate::statistics_testing::{check, covariance};
use nrz_matrix::{
    CompanionSetDraft, CovarianceBlock, Extent, Heap, HeapLimits, JointLimits, JointModel,
    MomentAssurance, OperationDraft, PublishedBundle, ShapeLimits, Source, SourceFactor, TargetMap,
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};
use nrz_units::Descriptor;

/// All sixteen output covariance entries agree with an independent rational joint contraction.
/// ´test:integration:matrix-product-propagates-all-joint-entry-cross-blocks´
#[test]
fn matrix_product_propagates_all_joint_entry_cross_blocks() -> TestResult {
    product::<f32>(false)?;
    product::<f64>(false)
}

/// An explicitly zero-variation factor at its declared mean makes the product exactly affine.
/// ´test:integration:fixed-factor-product-retains-exact-affine-moments´
#[test]
fn fixed_factor_product_retains_exact_affine_moments() -> TestResult {
    product::<f32>(true)?;
    product::<f64>(true)
}

/// Compare every output and output-input covariance entry in each supported precision.
fn product<P: Scalar + 'static>(fixed: bool) -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(2)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let (left, right, expected_joint) = operands::<P>(&mut heap, fixed)?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::matmul(contract.clone(), contract, ShapeLimits::default())?,
        vec![left.value(), right.value()],
    )?)?;
    let jacobian = [
        5, 0, 7, 0, 0, 5, 0, 7, 6, 0, 8, 0, 0, 6, 0, 8, 1, 2, 0, 0, 3, 4, 0, 0, 0, 0, 1, 2, 0, 0,
        3, 4,
    ];
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    assert_eq!(
        moments.assurance(),
        if fixed {
            MomentAssurance::AffineMoments
        } else {
            MomentAssurance::LinearizedMoments
        }
    );
    check(
        moments.covariance(),
        &covariance(&jacobian, 4, &jacobian, 4, 8, &expected_joint)?,
    )?;
    let expected_jacobian = jacobian
        .into_iter()
        .map(|value| Rational::new(value, 1))
        .collect::<Result<Vec<_>, _>>()?;
    check(
        moments.jacobian().ok_or("product Jacobian")?,
        &expected_jacobian,
    )?;
    if fixed {
        let means = [23, 34, 31, 46]
            .into_iter()
            .map(|value| Rational::new(value, 1))
            .collect::<Result<Vec<_>, _>>()?;
        check(moments.means().ok_or("exact product means")?, &means)?;
    } else {
        assert!(moments.means().is_none());
    }
    let cross =
        heap.materialize_cross_covariance(result.value(), right.value(), JointLimits::default())?;
    let mut selection = [0; 32];
    for index in 0..4 {
        *selection
            .get_mut(index + 4 * (index + 4))
            .ok_or("oracle selector")? = 1;
    }
    check(
        heap.moments(cross)?.covariance(),
        &covariance(&jacobian, 4, &selection, 4, 8, &expected_joint)?,
    )?;
    Ok(())
}

/// A single original model couples both matrices; zero rows explicitly declare the fixed variant.
fn operands<P: Scalar + 'static>(
    heap: &mut Heap,
    fixed: bool,
) -> TestResult<(PublishedBundle<P>, PublishedBundle<P>, Vec<Rational>)> {
    let quantity = Descriptor::ratio();
    let left = heap.import_represented(
        Extent::new(2, 2)?,
        &quantity,
        &[P::ONE, P::from_u8(2), P::from_u8(3), P::from_u8(4)],
    )?;
    let right = heap.import_represented(
        Extent::new(2, 2)?,
        &quantity,
        &[P::from_u8(5), P::from_u8(6), P::from_u8(7), P::from_u8(8)],
    )?;
    let sources = (0..8)
        .map(|_| Source::new(&quantity, 1))
        .collect::<Result<Vec<_>, _>>()?;
    let limits = JointLimits::default();
    let (coefficients, expected) = factor::<P>(fixed)?;
    let mut weights = vec![P::ONE; 8];
    weights.push(P::ONE / P::from_u8(4));
    let blocks = vec![CovarianceBlock::Known; 64];
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(8, &coefficients, &weights, limits)?,
        &blocks,
        limits,
    )?;
    let targets = [
        TargetMap::capture(heap, left.value())?,
        TargetMap::capture(heap, right.value())?,
    ];
    let mut draft = CompanionSetDraft::new(model, targets, [vec![0, 1, 2, 3], vec![4, 5, 6, 7]]);
    let families = [vec![1, 2, 3, 4], vec![5, 6, 7, 8]].map(|values| {
        values
            .into_iter()
            .map(|value| Finite::new(P::from_u8(value)).map(Enclosure::singleton))
            .collect::<Result<Vec<_>, _>>()
    });
    let [first, second] = families;
    draft = draft.with_means(heap, [first?, second?], &ArithmeticContext::new()?, limits)?;
    heap.publish_joint(draft, limits)?;
    Ok((left, right, expected))
}

/// Build factor input and exact rational truth independently of production contractions.
fn factor<P: Scalar>(fixed: bool) -> TestResult<(Vec<P>, Vec<Rational>)> {
    let mut coefficients = vec![P::ZERO; 72];
    for index in 0..8 {
        if fixed && index < 4 {
            continue;
        }
        *coefficients
            .get_mut(index + 8 * index)
            .ok_or("factor diagonal")? = P::ONE;
        *coefficients
            .get_mut(index + 64)
            .ok_or("factor shared column")? = P::from_u8(u8::try_from(index + 1)?);
    }
    let mut expected = Vec::new();
    for column in 0..8 {
        for row in 0..8 {
            let value = if fixed && (row < 4 || column < 4) {
                Rational::new(0, 1)?
            } else {
                Rational::new(
                    (row + 1) * (column + 1) + if row == column { 4 } else { 0 },
                    4,
                )?
            };
            expected.push(value);
        }
    }
    Ok((coefficients, expected))
}
