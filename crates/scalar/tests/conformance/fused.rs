// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`fused_endpoint_triples`] | scalar | Eight-corner hulls cover mixed signs, uncertainty widths and cancellation. |
//! | [`fused_rounding_is_explicit`] | scalar | Fused cancellation has one rounding and differs from multiply-then-add. |
//! | [`fused_boundary_oracle`] | scalar | Fused arithmetic covers underflow, ties and overflow in both formats. |
//! | [`fused_intermediate_range`] | scalar | Fused evaluation does not accidentally round or overflow its product first. |

//! Independent exact products and sums for explicit fused evaluation.

use crate::testing::{Binary, TestResult, contains, contains_or_overflows, point};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Enclosure, Scalar};

/// Compare every endpoint triple with the exact multi-affine expression.
fn triples<P: Binary>(boxes: &[(P, P)]) -> TestResult {
    let context = ArithmeticContext::new()?;
    for &(first, last) in boxes {
        let lhs = Enclosure::new(first, last)?;
        for &(start, end) in boxes {
            let rhs = Enclosure::new(start, end)?;
            for &(low, high) in boxes {
                let offset = Enclosure::new(low, high)?;
                let result = lhs.mul_add(rhs, offset, &context)?;
                for left in [first, last] {
                    for right in [start, end] {
                        for addend in [low, high] {
                            contains(
                                result,
                                &(left.rational() * right.rational() + addend.rational()),
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Eight-corner hulls cover mixed signs, uncertainty widths and cancellation.
/// ´claim:scalar:fused-endpoint-triples´
/// ´test:integration:fused-endpoint-triples´
#[test]
fn fused_endpoint_triples() -> TestResult {
    triples(&[(-3.0_f32, -2.0), (-1.0, 2.0), (0.0, 0.5), (1.0, 3.0)])?;
    triples(&[(-3.0_f64, -2.0), (-1.0, 2.0), (0.0, 0.5), (1.0, 3.0)])
}

/// Cancellation retains the product's low bits only on the explicit fused route.
fn distinction<P: Binary>(left: P, right: P, offset: P) -> TestResult {
    let context = ArithmeticContext::new()?;
    let lhs = point(left)?;
    let rhs = point(right)?;
    let addend = point(offset)?;
    let exact = left.rational() * right.rational() + offset.rational();
    let fused = lhs.mul_add(rhs, addend, &context)?;
    let separate = lhs.mul(rhs, &context)?.add(addend, &context)?;
    contains(fused, &exact);
    contains(separate, &exact);
    assert!(fused.upper().get() < P::ZERO);
    assert!(separate.upper().get() > P::ZERO);
    Ok(())
}

/// Fused cancellation has one rounding and differs from multiply-then-add.
/// ´claim:scalar:fused-rounding-is-explicit´
/// ´test:integration:fused-rounding-is-explicit´
#[test]
fn fused_rounding_is_explicit() -> TestResult {
    distinction(
        f32::from_bits(0x3f80_0001),
        f32::from_bits(0x3f7f_fffe),
        -1.0,
    )?;
    distinction(
        f64::from_bits(0x3ff0_0000_0000_0001),
        f64::from_bits(0x3fef_ffff_ffff_fffe),
        -1.0,
    )
}

/// Tiny products and overflowing unfused products are checked against rational truth.
fn boundaries<P: Binary>(cases: &[(P, P, P)]) -> TestResult {
    let context = ArithmeticContext::new()?;
    for &(left, right, offset) in cases {
        let exact = left.rational() * right.rational() + offset.rational();
        contains_or_overflows(
            point(left)?.mul_add(point(right)?, point(offset)?, &context),
            &exact,
        )?;
    }
    Ok(())
}

/// Fused arithmetic covers underflow, ties and overflow in both formats.
/// ´claim:scalar:fused-boundary-oracle´
/// ´test:integration:fused-boundary-oracle´
#[test]
fn fused_boundary_oracle() -> TestResult {
    boundaries(&[
        (f32::from_bits(1), 0.5, 0.0),
        (f32::MIN_POSITIVE, 0.5, -f32::from_bits(1)),
        (f32::MAX, 1.5, -f32::MAX),
        (f32::MAX, 2.0, f32::MAX),
        (1.0, 1.0, f32::from_bits(0x3380_0000)),
    ])?;
    boundaries(&[
        (f64::from_bits(1), 0.5, 0.0),
        (f64::MIN_POSITIVE, 0.5, -f64::from_bits(1)),
        (f64::MAX, 1.5, -f64::MAX),
        (f64::MAX, 2.0, f64::MAX),
        (1.0, 1.0, f64::from_bits(0x3ca0_0000_0000_0000)),
    ])
}

/// A fused operation may succeed despite overflow of its unfused intermediate.
fn fused_range<P: Scalar>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let large = point(P::max_finite())?;
    let half = P::ONE / (P::ONE + P::ONE);
    let multiplier = point(P::ONE + half)?;
    assert_eq!(
        large.mul(multiplier, &context),
        Err(ArithmeticFailure::Overflow)
    );
    assert!(large.mul_add(multiplier, large.negated(), &context).is_ok());
    Ok(())
}

/// Fused evaluation does not accidentally round or overflow its product first.
/// ´claim:scalar:fused-intermediate-range´
/// ´test:integration:fused-intermediate-range´
#[test]
fn fused_intermediate_range() -> TestResult {
    fused_range::<f32>()?;
    fused_range::<f64>()
}
