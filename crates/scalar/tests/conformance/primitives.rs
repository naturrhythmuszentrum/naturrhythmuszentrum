// SPDX-License-Identifier: AGPL-3.0-only
//! Independent boundary oracles for each correctly rounded primitive.

use nrz_scalar::{ArithmeticContext, Enclosure, Scalar};
use num_traits::Zero;

use crate::testing::{Binary, TestResult, contains, contains_or_overflows, point, root_contains};

/// Compare every endpoint pair against exact rational arithmetic.
fn binary_corpus<P: Binary>(values: &[P]) -> TestResult {
    let context = ArithmeticContext::new()?;
    for &left in values {
        for &right in values {
            let lhs = point(left)?;
            let rhs = point(right)?;
            let exact_left = left.rational();
            let exact_right = right.rational();
            contains_or_overflows(lhs.add(rhs, &context), &(&exact_left + &exact_right))?;
            contains_or_overflows(lhs.sub(rhs, &context), &(&exact_left - &exact_right))?;
            contains_or_overflows(lhs.mul(rhs, &context), &(&exact_left * &exact_right))?;
            if !exact_right.is_zero() {
                contains_or_overflows(lhs.div(rhs, &context), &(&exact_left / &exact_right))?;
            }
        }
        let exact = left.rational();
        contains_or_overflows(point(left)?.square(&context), &(&exact * &exact))?;
        if left >= P::ZERO {
            root_contains(point(left)?.sqrt(&context)?, &exact);
        }
    }
    Ok(())
}

/// Binary32 boundaries include ties, cancellation, subnormals and both finite limits.
/// ´test:integration:binary32-primitive-boundary-oracle´
#[test]
fn binary32_primitive_boundary_oracle() -> TestResult {
    let bits = [
        0,
        1,
        2,
        3,
        0x007f_ffff,
        0x0080_0000,
        0x0080_0001,
        0x3f00_0000,
        0x3f7f_ffff,
        0x3f80_0000,
        0x3f80_0001,
        0x3f80_0002,
        0x3380_0000,
        0x4000_0000,
        0x4040_0000,
        0x7f7f_fffe,
        0x7f7f_ffff,
    ];
    let values: Vec<_> = bits
        .into_iter()
        .flat_map(|bits| [f32::from_bits(bits), f32::from_bits(bits | 0x8000_0000)])
        .collect();
    binary_corpus(&values)
}

/// Binary64 boundaries exercise the same laws at its independent exponent range.
/// ´test:integration:binary64-primitive-boundary-oracle´
#[test]
fn binary64_primitive_boundary_oracle() -> TestResult {
    let bits = [
        0,
        1,
        2,
        3,
        0x000f_ffff_ffff_ffff,
        0x0010_0000_0000_0000,
        0x0010_0000_0000_0001,
        0x3fe0_0000_0000_0000,
        0x3fef_ffff_ffff_ffff,
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0001,
        0x3ff0_0000_0000_0002,
        0x3ca0_0000_0000_0000,
        0x4000_0000_0000_0000,
        0x4008_0000_0000_0000,
        0x7fef_ffff_ffff_fffe,
        0x7fef_ffff_ffff_ffff,
    ];
    let values: Vec<_> = bits
        .into_iter()
        .flat_map(|bits| {
            [
                f64::from_bits(bits),
                f64::from_bits(bits | 0x8000_0000_0000_0000),
            ]
        })
        .collect();
    binary_corpus(&values)
}

/// Check non-singleton endpoint hulls without assuming endpoint independence is tight.
fn endpoint_hulls<P: Binary>(intervals: &[(P, P)]) -> TestResult {
    let context = ArithmeticContext::new()?;
    for &(lower, upper) in intervals {
        let lhs = Enclosure::new(lower, upper)?;
        for &(start, end) in intervals {
            let rhs = Enclosure::new(start, end)?;
            for left in [lower, upper] {
                for right in [start, end] {
                    contains(
                        lhs.add(rhs, &context)?,
                        &(left.rational() + right.rational()),
                    );
                    contains(
                        lhs.sub(rhs, &context)?,
                        &(left.rational() - right.rational()),
                    );
                    contains(
                        lhs.mul(rhs, &context)?,
                        &(left.rational() * right.rational()),
                    );
                    if start > P::ZERO || end < P::ZERO {
                        contains(
                            lhs.div(rhs, &context)?,
                            &(left.rational() / right.rational()),
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

/// All four signs and zero crossings preserve inclusion in both formats.
/// ´test:integration:endpoint-hulls-enclose´
#[test]
fn endpoint_hulls_enclose() -> TestResult {
    endpoint_hulls(&[(-3.0_f32, -2.0), (-2.0, 3.0), (0.0, 2.0), (2.0, 3.0)])?;
    endpoint_hulls(&[(-3.0_f64, -2.0), (-2.0, 3.0), (0.0, 2.0), (2.0, 3.0)])
}

/// Exact representability alone does not justify an exactness shortcut.
fn neighbours<P: Scalar>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let two = P::ONE + P::ONE;
    let value = point(P::ONE)?.add(point(two)?, &context)?;
    assert!(value.lower().get().same_bits((P::ONE + two).next_down()));
    assert!(value.upper().get().same_bits((P::ONE + two).next_up()));
    let tiny = point(P::ZERO.next_up())?;
    let square = tiny.square(&context)?;
    assert!(square.lower().get().same_bits(P::ZERO));
    assert!(square.upper().get().same_bits(P::ZERO.next_up()));
    let crossing = Enclosure::new(-P::ONE, two)?.square(&context)?;
    assert!(crossing.lower().get().same_bits(P::ZERO));
    Ok(())
}

/// The outward-neighbour rule applies at exact ordinary sums and underflow alike.
/// ´test:integration:outward-neighbours-without-residual-shortcuts´
#[test]
fn outward_neighbours_without_residual_shortcuts() -> TestResult {
    neighbours::<f32>()?;
    neighbours::<f64>()
}
