// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`named_norm_rational_oracles`] | accuracy | Zero, huge, subnormal and mixed data have independent successful norm bounds. |
//! | [`norm_empty_and_overflow_contracts`] | accuracy | Norm overflow never becomes a nonfinite bound; empty summaries are exact zero. |

//! Independent rational expectations for all named entry norms.

use crate::testing::{Binary, Rational, TestResult, covers};
use nrz_accuracy::{AccuracyFailure, BoundVectorRef, Nonnegative, NormKind};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure};
use num_traits::Zero;

/// Decode inputs independently; compare squared root bounds without floating roots.
fn norms<P: Binary>(values: &[P]) -> TestResult {
    let context = ArithmeticContext::new()?;
    let vector = BoundVectorRef::new(values)?;
    let exact_sum: Rational = values.iter().map(|value| value.rational()).sum();
    let squares: Rational = values
        .iter()
        .map(|value| {
            let decoded = value.rational();
            &decoded * &decoded
        })
        .sum();
    let maximum = values
        .iter()
        .map(|value| value.rational())
        .max()
        .unwrap_or_else(Rational::zero);
    for kind in [
        NormKind::One,
        NormKind::Euclidean,
        NormKind::Frobenius,
        NormKind::Infinity,
    ] {
        let summary = vector.norm(kind, &context)?;
        assert_eq!(summary.kind(), kind);
        assert_eq!(summary.entry_count(), values.len());
        let upper = summary.upper().get().rational();
        match kind {
            NormKind::One => covers(summary.upper(), &exact_sum),
            NormKind::Euclidean | NormKind::Frobenius => assert!(&upper * &upper >= squares),
            NormKind::Infinity => assert_eq!(upper, maximum),
        }
    }
    Ok(())
}

/// Zero, huge, subnormal and mixed data have independent successful norm bounds.
/// ´claim:accuracy:named-norm-rational-oracles´
/// ´test:integration:named-norm-rational-oracles´
#[test]
fn named_norm_rational_oracles() -> TestResult {
    norms::<f32>(&[])?;
    norms::<f64>(&[])?;
    norms(&[0.0_f32, -0.0])?;
    norms(&[0.0_f64, -0.0])?;
    norms(&[3.0_f32, 4.0])?;
    norms(&[3.0_f64, 4.0])?;
    norms(&[1.0e30_f32, 1.0e30, f32::from_bits(1)])?;
    norms(&[1.0e200_f64, 1.0e200, f64::from_bits(1)])?;
    norms(&[f32::from_bits(1), f32::from_bits(2), f32::MIN_POSITIVE])?;
    norms(&[f64::from_bits(1), f64::from_bits(2), f64::MIN_POSITIVE])?;
    norms(&[f32::from_bits(1)])?;
    norms(&[f64::from_bits(1)])
}

/// Infinity remains finite at the format limit; a too-large root norm refuses.
fn extremes<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let entries = [P::max_finite(), P::max_finite()];
    let vector = BoundVectorRef::new(&entries)?;
    assert_eq!(
        vector.norm(NormKind::Infinity, &context)?.upper(),
        Nonnegative::new(P::max_finite())?
    );
    for kind in [NormKind::One, NormKind::Euclidean, NormKind::Frobenius] {
        assert_eq!(
            vector.norm(kind, &context),
            Err(AccuracyFailure::Arithmetic(ArithmeticFailure::Overflow))
        );
    }
    for kind in [
        NormKind::One,
        NormKind::Euclidean,
        NormKind::Frobenius,
        NormKind::Infinity,
    ] {
        assert_eq!(
            BoundVectorRef::<P>::new(&[])?.norm(kind, &context)?.upper(),
            Nonnegative::ZERO
        );
    }
    Ok(())
}

/// Norm overflow never becomes a nonfinite bound; empty summaries are exact zero.
/// ´claim:accuracy:norm-empty-and-overflow-contracts´
/// ´test:integration:norm-empty-and-overflow-contracts´
#[test]
fn norm_empty_and_overflow_contracts() -> TestResult {
    extremes::<f32>()?;
    extremes::<f64>()
}
