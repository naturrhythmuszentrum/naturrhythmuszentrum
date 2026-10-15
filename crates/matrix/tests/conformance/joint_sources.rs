// SPDX-License-Identifier: AGPL-3.0-only
//! Public source admission laws with independent dyadic rational expectations.

use crate::testing::TestResult;
use nrz_matrix::{CovarianceBlock, JointError, JointLimits, JointModel, Source, SourceFactor};
use nrz_scalar::{ArithmeticContext, Scalar};
use nrz_units::Descriptor;

/// Decode the small binary fixture endpoints independently as exact integer fractions.
fn fraction(value: f64) -> Result<(i128, i128), Box<dyn std::error::Error>> {
    if value == 0.0 {
        return Ok((0, 1));
    }
    let bits = value.to_bits();
    let exponent = i32::try_from((bits >> 52) & 2047)? - 1023 - 52;
    let magnitude = i128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52));
    let numerator = if bits >> 63 == 0 {
        magnitude
    } else {
        -magnitude
    };
    if exponent >= 0 {
        Ok((
            numerator
                .checked_shl(exponent.unsigned_abs())
                .ok_or("oracle range")?,
            1,
        ))
    } else {
        Ok((
            numerator,
            1_i128
                .checked_shl(exponent.unsigned_abs())
                .ok_or("oracle range")?,
        ))
    }
}
/// Compare endpoints to an independently specified rational parameter.
fn contains<P: Scalar>(
    interval: nrz_scalar::Enclosure<P>,
    numerator: i128,
    denominator: i128,
) -> TestResult {
    let (lower, lower_denominator) = fraction(interval.lower().get().to_f64())?;
    let (upper, upper_denominator) = fraction(interval.upper().get().to_f64())?;
    assert!(lower * denominator <= numerator * lower_denominator);
    assert!(upper * denominator >= numerator * upper_denominator);
    Ok(())
}
/// The exact outer product of (1,2) with weight 1/4 is PSD, singular and rank one.
fn rank_one<P: Scalar>() -> TestResult {
    let limits = JointLimits::default();
    let factor = SourceFactor::new(
        2,
        &[P::ONE, P::from_u8(2)],
        &[P::ONE / P::from_u8(4)],
        limits,
    )?;
    let sources = [
        Source::new(&Descriptor::ratio(), 1)?,
        Source::new(&Descriptor::ratio(), 1)?,
    ];
    let model =
        JointModel::from_factor(&sources, factor, &vec![CovarianceBlock::Known; 4], limits)?;
    let context = ArithmeticContext::new()?;
    for (row, column, numerator) in [(0, 0, 1), (1, 0, 2), (0, 1, 2), (1, 1, 4)] {
        contains(
            model.covariance(row, column, &context, limits)?,
            numerator,
            4,
        )?;
    }
    assert_eq!(model.sources()[0].id(), sources[0].id());
    assert_eq!(model.clone().id(), model.id());
    Ok(())
}
/// Both precisions admit a genuinely singular nonzero covariance without a pivot test.
/// ´test:integration:factor-rank-one-rational-oracle´
#[test]
fn factor_rank_one_rational_oracle() -> TestResult {
    rank_one::<f32>()?;
    rank_one::<f64>()
}

/// Rectangular zero-column and dependent-column factors retain exact PSD semantics.
/// ´test:integration:factor-rectangular-and-zero-oracles´
#[test]
fn factor_rectangular_and_zero_oracles() -> TestResult {
    let limits = JointLimits::default();
    let context = ArithmeticContext::new()?;
    let factor = SourceFactor::new(
        2,
        &[1.0_f64, 2.0, 2.0, 4.0, 0.0, 0.0],
        &[0.25, 0.5, 1.0],
        limits,
    )?;
    contains(factor.entry(0, 0, &context, limits)?, 9, 4)?;
    contains(factor.entry(1, 0, &context, limits)?, 9, 2)?;
    contains(factor.entry(1, 1, &context, limits)?, 9, 1)?;
    let zero = SourceFactor::<f32>::new(2, &[], &[], limits)?;
    contains(zero.entry(0, 1, &context, limits)?, 0, 1)
}

/// Missing cross blocks refuse despite distinct source IDs or a PSD completion witness.
/// ´test:integration:joint-unknown-is-never-independent´
#[test]
fn joint_unknown_is_never_independent() -> TestResult {
    let limits = JointLimits::default();
    let sources = [
        Source::new(&Descriptor::ratio(), 1)?,
        Source::new(&Descriptor::ratio(), 1)?,
    ];
    let factor = SourceFactor::new(2, &[1.0_f64, 0.0, 0.0, 1.0], &[1.0, 1.0], limits)?;
    let blocks = [
        CovarianceBlock::Known,
        CovarianceBlock::Unknown,
        CovarianceBlock::Unknown,
        CovarianceBlock::Known,
    ];
    let model = JointModel::from_factor(&sources, factor, &blocks, limits)?;
    model.require_blocks(&[0], &[0], limits)?;
    assert!(matches!(
        model.require_blocks(&[0], &[1], limits),
        Err(JointError::MissingCrossBlock)
    ));
    assert!(matches!(
        model.covariance(0, 1, &ArithmeticContext::new()?, limits),
        Err(JointError::MissingCrossBlock)
    ));
    Ok(())
}

/// A shared identity cannot acquire another declaration or covariance in another model.
/// ´test:integration:joint-source-definitions-conflict-globally´
#[test]
fn joint_source_definitions_conflict_globally() -> TestResult {
    let limits = JointLimits::default();
    let source = Source::new(&Descriptor::ratio(), 7)?;
    let factor = SourceFactor::new(1, &[1.0_f64], &[1.0], limits)?;
    let model = JointModel::from_factor(
        std::slice::from_ref(&source),
        factor.clone(),
        &[CovarianceBlock::Known],
        limits,
    )?;
    let again = JointModel::from_factor(
        std::slice::from_ref(&source),
        factor,
        &[CovarianceBlock::Known],
        limits,
    )?;
    assert_eq!(model.id(), again.id());
    assert!(matches!(
        source.redeclare(&Descriptor::ratio(), 8),
        Err(JointError::SourceConflict)
    ));
    let different = SourceFactor::new(1, &[2.0], &[1.0], limits)?;
    assert!(matches!(
        JointModel::from_factor(
            std::slice::from_ref(&source),
            different,
            &[CovarianceBlock::Known],
            limits
        ),
        Err(JointError::SourceConflict)
    ));
    Ok(())
}

/// Explicit zeros require both an assumption and a sufficient exact zero witness.
/// ´test:integration:joint-zero-table-refusals´
#[test]
fn joint_zero_table_refusals() -> TestResult {
    let limits = JointLimits::default();
    let sources = [
        Source::new(&Descriptor::ratio(), 1)?,
        Source::new(&Descriptor::ratio(), 1)?,
    ];
    let factor = SourceFactor::new(2, &[1.0_f64, 1.0], &[1.0], limits)?;
    let zero = CovarianceBlock::DeclaredZero("declared uncorrelated".into());
    let blocks = [
        CovarianceBlock::Known,
        zero.clone(),
        zero,
        CovarianceBlock::Known,
    ];
    assert!(matches!(
        JointModel::from_factor(&sources, factor.clone(), &blocks, limits),
        Err(JointError::InvalidZero)
    ));
    assert!(matches!(
        JointModel::from_factor(&sources, factor.clone(), &[CovarianceBlock::Known], limits),
        Err(JointError::LengthMismatch)
    ));
    let asymmetric = [
        CovarianceBlock::Known,
        CovarianceBlock::Unknown,
        CovarianceBlock::Known,
        CovarianceBlock::Known,
    ];
    assert!(matches!(
        JointModel::from_factor(&sources, factor, &asymmetric, limits),
        Err(JointError::Asymmetric)
    ));
    Ok(())
}

/// Factor scan work is exactly entries plus weights; boundary policies are inclusive.
/// ´test:integration:joint-factor-limit-boundaries´
#[test]
fn joint_factor_limit_boundaries() {
    for work in [2, 3, 4] {
        let result = SourceFactor::new(2, &[1.0_f32, 2.0], &[1.0], JointLimits::new(2, 2, work));
        assert_eq!(result.is_ok(), work >= 3);
    }
    for entries in [1, 2, 3] {
        let result = SourceFactor::new(2, &[1.0_f64, 2.0], &[1.0], JointLimits::new(2, entries, 3));
        assert_eq!(result.is_ok(), entries >= 2);
    }
    assert!(matches!(
        SourceFactor::new(1, &[1.0], &[-1.0], JointLimits::default()),
        Err(JointError::InvalidWeight)
    ));
}
