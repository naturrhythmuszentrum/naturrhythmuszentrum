// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact sums, products and squared-norm bounds.

use crate::testing::{Binary, Rational, TestResult, contains};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Finite, dot, norm_upper, sum};
use num_traits::Zero;

/// Exact rational sums and products form the expectation, not an enclosure fold.
fn reductions<P: Binary>(left: &[P], right: &[P]) -> TestResult {
    let context = ArithmeticContext::new()?;
    let exact_sum: Rational = left.iter().map(|value| value.rational()).sum();
    let exact_dot: Rational = left
        .iter()
        .zip(right)
        .map(|(&first, &second)| first.rational() * second.rational())
        .sum();
    contains(sum(left, &context)?, &exact_sum);
    contains(dot(left, right, &context)?, &exact_dot);
    Ok(())
}

/// Cancellation, underflow and exact small answers succeed in each format.
/// ´test:integration:fixed-reduction-rational-oracles´
#[test]
fn fixed_reduction_rational_oracles() -> TestResult {
    reductions(&[1.0_f32, 2.0, 3.0], &[4.0, -1.0, 2.0])?;
    reductions(&[1.0_f64, 2.0, 3.0], &[4.0, -1.0, 2.0])?;
    reductions(&[1.0e8_f32, 1.0, -1.0e8], &[1.0, 1.0, 1.0])?;
    reductions(&[1.0e16_f64, 1.0, -1.0e16], &[1.0, 1.0, 1.0])?;
    reductions(&[f32::from_bits(1), -f32::MIN_POSITIVE], &[0.5, 0.5])?;
    reductions(&[f64::from_bits(1), -f64::MIN_POSITIVE], &[0.5, 0.5])
}

/// Empty reductions are exact and overflowing intermediate order is observable.
fn order<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let zero = Rational::zero();
    let empty = sum::<P>(&[], &context)?;
    assert_eq!(empty.lower(), Finite::ZERO);
    assert_eq!(empty.upper(), Finite::ZERO);
    contains(dot::<P>(&[], &[], &context)?, &zero);
    assert_eq!(
        dot(&[P::ONE], &[], &context),
        Err(ArithmeticFailure::LengthMismatch)
    );
    let large = P::max_finite() / (P::ONE + P::ONE);
    let increasing = [large, large, -large];
    let cancelled_first = [large, -large, large];
    assert_eq!(sum(&increasing, &context), Err(ArithmeticFailure::Overflow));
    contains(sum(&cancelled_first, &context)?, &large.rational());
    assert_eq!(
        dot(&increasing, &[P::ONE; 3], &context),
        Err(ArithmeticFailure::Overflow)
    );
    contains(
        dot(&cancelled_first, &[P::ONE; 3], &context)?,
        &large.rational(),
    );
    Ok(())
}

/// Increasing-index left folds refuse intermediate overflow instead of reassociating.
/// ´test:integration:reduction-order-and-empty-laws´
#[test]
fn reduction_order_and_empty_laws() -> TestResult {
    order::<f32>()?;
    order::<f64>()
}

/// The returned norm upper bound is squared and compared with the exact sum of squares.
fn norm<P: Binary>(values: &[P]) -> TestResult {
    let context = ArithmeticContext::new()?;
    let exact: Rational = values
        .iter()
        .map(|value| {
            let decoded = value.rational();
            &decoded * &decoded
        })
        .sum();
    let bound = norm_upper(values, &context)?.get().rational();
    assert!(bound >= Rational::zero());
    assert!(&bound * &bound >= exact);
    Ok(())
}

/// Scaling contains norms of huge, tiny, mixed and empty data without direct squaring overflow.
/// ´test:integration:scaled-norm-rational-oracle´
#[test]
fn scaled_norm_rational_oracle() -> TestResult {
    norm::<f32>(&[])?;
    norm::<f64>(&[])?;
    norm(&[0.0_f32, -0.0])?;
    norm(&[0.0_f64, -0.0])?;
    norm(&[3.0_f32, -4.0])?;
    norm(&[3.0_f64, -4.0])?;
    norm(&[1.0e30_f32, -1.0e30, f32::from_bits(1)])?;
    norm(&[1.0e200_f64, -1.0e200, f64::from_bits(1)])?;
    norm(&[f32::from_bits(1), -f32::from_bits(2), f32::MIN_POSITIVE])?;
    norm(&[f64::from_bits(1), -f64::from_bits(2), f64::MIN_POSITIVE])?;
    norm(&[f32::from_bits(1)])?;
    norm(&[f64::from_bits(1)])
}

/// Invalid input and unbounded norms are never converted into a finite certificate.
/// ´test:integration:reduction-and-norm-refusals´
#[test]
fn reduction_and_norm_refusals() -> TestResult {
    let context = ArithmeticContext::new()?;
    assert_eq!(
        sum(&[f32::NAN], &context),
        Err(ArithmeticFailure::NonFinite)
    );
    assert_eq!(
        dot(&[0.0_f64], &[f64::INFINITY], &context),
        Err(ArithmeticFailure::NonFinite)
    );
    assert_eq!(
        norm_upper(&[f32::INFINITY], &context),
        Err(ArithmeticFailure::NonFinite)
    );
    assert_eq!(
        norm_upper(&[f64::NAN], &context),
        Err(ArithmeticFailure::NonFinite)
    );
    assert_eq!(
        norm_upper(&[f32::MAX, f32::MAX], &context),
        Err(ArithmeticFailure::Overflow)
    );
    assert_eq!(
        norm_upper(&[f64::MAX, f64::MAX], &context),
        Err(ArithmeticFailure::Overflow)
    );
    Ok(())
}
