// SPDX-License-Identifier: AGPL-3.0-only
//! Nonnegative admission and exact-rational composition checks.

use crate::testing::{Binary, Rational, TestResult, covers};
use nrz_accuracy::{AccuracyFailure, BoundVectorRef, Nonnegative};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure};

/// Nonfinite and negative values cannot enter either public bound constructor.
/// ´test:integration:bound-admission-and-zero´
#[test]
fn bound_admission_and_zero() -> TestResult {
    for value in [-1.0_f32, f32::NEG_INFINITY, f32::INFINITY, f32::NAN] {
        assert_eq!(Nonnegative::new(value), Err(AccuracyFailure::InvalidBound));
        assert!(matches!(
            BoundVectorRef::new(&[0.0, value]),
            Err(AccuracyFailure::InvalidBound)
        ));
    }
    for value in [-1.0_f64, f64::NEG_INFINITY, f64::INFINITY, f64::NAN] {
        assert_eq!(Nonnegative::new(value), Err(AccuracyFailure::InvalidBound));
        assert!(matches!(
            BoundVectorRef::new(&[0.0, value]),
            Err(AccuracyFailure::InvalidBound)
        ));
    }
    assert_eq!(Nonnegative::new(-0.0_f32)?.get().to_bits(), 0);
    assert_eq!(Nonnegative::new(-0.0_f64)?.get().to_bits(), 0);
    assert_eq!(
        Nonnegative::new(f32::MAX)?.get().to_bits(),
        f32::MAX.to_bits()
    );
    assert_eq!(
        Nonnegative::new(f64::MAX)?.get().to_bits(),
        f64::MAX.to_bits()
    );
    Ok(())
}

/// Every successful elementary and vector operation covers exact rational truth.
fn composition<P: Binary>(left: &[P], right: &[P]) -> TestResult {
    let context = ArithmeticContext::new()?;
    for (&first, &second) in left.iter().zip(right) {
        let bound = Nonnegative::new(first)?;
        let factor = Nonnegative::new(second)?;
        covers(
            bound.add(factor, &context)?,
            &(first.rational() + second.rational()),
        );
        covers(
            bound.scale(factor, &context)?,
            &(first.rational() * second.rational()),
        );
    }
    let exact_sum: Rational = left.iter().map(|value| value.rational()).sum();
    let exact_dot: Rational = left
        .iter()
        .zip(right)
        .map(|(&first, &second)| first.rational() * second.rational())
        .sum();
    let vector = BoundVectorRef::new(left)?;
    covers(vector.sum(&context)?, &exact_sum);
    covers(
        vector.dot(BoundVectorRef::new(right)?, &context)?,
        &exact_dot,
    );
    assert!(std::ptr::eq(vector.as_slice(), left));
    Ok(())
}

/// Outward sums and products include ties, tiny contributions and underflow.
/// ´test:integration:bound-composition-rational-oracles´
#[test]
fn bound_composition_rational_oracles() -> TestResult {
    composition(&[0.0_f32, 1.0, 0.1, 3.0], &[4.0, 1.0, 0.3, 2.0])?;
    composition(&[0.0_f64, 1.0, 0.1, 3.0], &[4.0, 1.0, 0.3, 2.0])?;
    composition(
        &[f32::from_bits(1), f32::MIN_POSITIVE, 1.0],
        &[0.5, 0.1, f32::EPSILON / 2.0],
    )?;
    composition(
        &[f64::from_bits(1), f64::MIN_POSITIVE, 1.0],
        &[0.5, 0.1, f64::EPSILON / 2.0],
    )?;
    composition(&[1.0e30_f32, 1.0e-30], &[0.1, 1.0e30])?;
    composition(&[1.0e200_f64, 1.0e-200], &[0.1, 1.0e200])
}

/// Empty reductions and zero/unit scaling remain exact; overflow is explicit.
fn boundary<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let empty = BoundVectorRef::<P>::new(&[])?;
    assert_eq!(empty.as_slice(), []);
    assert_eq!(empty.sum(&context)?, Nonnegative::ZERO);
    assert_eq!(empty.dot(empty, &context)?, Nonnegative::ZERO);
    let values = [P::ONE];
    assert_eq!(
        empty.dot(BoundVectorRef::new(&values)?, &context),
        Err(AccuracyFailure::LengthMismatch)
    );
    let largest = Nonnegative::new(P::max_finite())?;
    assert_eq!(
        largest.scale(Nonnegative::ZERO, &context)?,
        Nonnegative::ZERO
    );
    assert_eq!(largest.scale(Nonnegative::ONE, &context)?, largest);
    assert_eq!(largest.add(Nonnegative::ZERO, &context)?, largest);
    let overflow = Err(AccuracyFailure::Arithmetic(ArithmeticFailure::Overflow));
    assert_eq!(largest.add(largest, &context), overflow);
    assert_eq!(
        largest.scale(Nonnegative::new(P::from_u8(2))?, &context),
        overflow
    );
    let entries = [P::max_finite(), P::max_finite()];
    assert_eq!(BoundVectorRef::new(&entries)?.sum(&context), overflow);
    assert_eq!(
        BoundVectorRef::new(&entries)?.dot(BoundVectorRef::new(&entries)?, &context),
        overflow
    );
    Ok(())
}

/// Refusal and exact-identity laws hold in both formats.
/// ´test:integration:bound-empty-identities-and-overflow´
#[test]
fn bound_empty_identities_and_overflow() -> TestResult {
    boundary::<f32>()?;
    boundary::<f64>()
}
