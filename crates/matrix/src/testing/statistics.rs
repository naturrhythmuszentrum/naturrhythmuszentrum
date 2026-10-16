// SPDX-License-Identifier: AGPL-3.0-only
//! Public statistical fixtures and independent exact rational covariance contractions.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::TestResult;
use crate::test_api::{
    CompanionSetDraft, CovarianceBlock, DenseCovariance, Extent, Handle, Heap, JointLimits,
    JointModel, PublishedBundle, Source, TargetMap, Value,
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};
use nrz_units::Descriptor;

/// Attach a complete model to an existing vector, with explicitly optional means.
pub(crate) fn attach<P: Scalar + 'static>(
    heap: &mut Heap,
    value: Handle<Value, P>,
    covariance: &[P],
    means: bool,
) -> TestResult<crate::test_api::CompanionRef<P>> {
    let view = heap.value(value)?;
    let quantity = view.region().descriptor().difference();
    let centers = view.region().export()?;
    let sources = (0..centers.len())
        .map(|_| Source::new(&quantity, 1))
        .collect::<Result<Vec<_>, _>>()?;
    let limits = JointLimits::default();
    let context = ArithmeticContext::new()?;
    let family = covariance
        .iter()
        .map(|value| Enclosure::new(value.next_down(), value.next_up()))
        .collect::<Result<Vec<_>, _>>()?;
    let dense =
        DenseCovariance::with_enclosures(centers.len(), covariance, &family, &context, limits)?;
    let model = JointModel::from_dense(
        &sources,
        dense,
        &vec![CovarianceBlock::Known; covariance.len()],
        limits,
    )?;
    let target = TargetMap::capture(heap, value)?;
    let indices = (0..centers.len()).collect::<Vec<_>>();
    let mut draft =
        CompanionSetDraft::new(model, [target.clone(), target], [indices.clone(), indices]);
    if means {
        let family = centers
            .into_iter()
            .map(|center| Finite::new(center).map(Enclosure::singleton))
            .collect::<Result<Vec<_>, _>>()?;
        draft = draft.with_means(heap, [family.clone(), family], &context, limits)?;
    }
    Ok(heap.publish_joint(draft, limits)?)
}
/// The two-entry worked covariance includes exact rational targets inside its parameter family.
pub(crate) fn vector<P: Scalar + 'static>(
    heap: &mut Heap,
    means: bool,
) -> TestResult<PublishedBundle<P>> {
    let value = heap.import_represented(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(2), P::from_u8(3)],
    )?;
    attach(
        heap,
        value.value(),
        &[
            P::ONE / P::from_u8(100),
            P::ONE / P::from_u8(200),
            P::ONE / P::from_u8(200),
            P::ONE / P::from_u8(25),
        ],
        means,
    )?;
    Ok(value)
}
/// Small scalar with a declared variance, convenient for lifecycle and boundary twins.
pub(crate) fn scalar<P: Scalar + 'static>(heap: &mut Heap) -> TestResult<PublishedBundle<P>> {
    let value =
        heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[P::from_u8(2)])?;
    attach(heap, value.value(), &[P::ONE / P::from_u8(4)], false)?;
    Ok(value)
}
/// Independent rational truth for the worked input, without production interval arithmetic.
pub(crate) fn worked_covariance() -> TestResult<Vec<Rational>> {
    [(1, 100), (1, 200), (1, 200), (1, 25)]
        .into_iter()
        .map(|(n, d)| Rational::new(n, d))
        .collect()
}
/// Independently contract full logical covariance, including every off-diagonal entry.
pub(crate) fn covariance(
    left: &[i128],
    rows: usize,
    right: &[i128],
    columns: usize,
    sources: usize,
    joint: &[Rational],
) -> TestResult<Vec<Rational>> {
    let mut result = Vec::new();
    for column in 0..columns {
        for row in 0..rows {
            let mut value = Rational::new(0, 1)?;
            for first in 0..sources {
                for second in 0..sources {
                    let term = Rational::new(
                        *left.get(row + rows * first).ok_or("left oracle shape")?,
                        1,
                    )?
                    .multiply(
                        *joint
                            .get(first + sources * second)
                            .ok_or("joint oracle shape")?,
                    )?
                    .multiply(Rational::new(
                        *right
                            .get(column + columns * second)
                            .ok_or("right oracle shape")?,
                        1,
                    )?)?;
                    value = value.add(term)?;
                }
            }
            result.push(value);
        }
    }
    Ok(result)
}
/// Decode each displayed parameter and its numerical error independently as exact fractions.
pub(crate) fn check<P: Scalar + 'static>(
    view: &crate::test_api::ParameterView<'_, P>,
    expected: &[Rational],
) -> TestResult {
    assert_eq!(view.extent().entries(), expected.len());
    for (index, truth) in expected.iter().enumerate() {
        let row = index % view.extent().rows();
        let column = index / view.extent().rows();
        let center = view.get(row, column)?.to_f64();
        let radius = view.error(row, column)?.to_f64();
        assert!(
            radius >= 0.0 && !radius.is_sign_negative(),
            "invalid parameter radius {radius:?}"
        );
        assert!(crate::statistics_oracle::covered(*truth, center, radius)?);
    }
    Ok(())
}

/// A covariance's diagonal centers are variances; cross-covariance diagonals are not.
pub(crate) fn nonnegative_variances<P: Scalar + 'static>(
    view: &crate::test_api::ParameterView<'_, P>,
) -> TestResult {
    assert_eq!(view.extent().rows(), view.extent().columns());
    for entry in 0..view.extent().rows() {
        let variance = view.get(entry, entry)?.to_f64();
        assert!(
            variance >= 0.0 && !variance.is_sign_negative(),
            "invalid variance {variance:?}"
        );
    }
    Ok(())
}
