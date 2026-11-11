// SPDX-License-Identifier: AGPL-3.0-only
//! Exact integer squares prove nearest rounding and both enclosures without a root implementation.

use super::TestResult;
use nrz_bindings::{Budget, Limits, readings::algebra::Rational};
use std::cmp::Ordering;

/// Every finite nonnegative binary64 value is an integer multiple of 2^-1074.
/// Decode bits directly; signed zero has the same exact integer as positive zero.
fn binary(value: f64) -> TestResult<(u64, usize)> {
    if !value.is_finite() || value < 0.0 {
        return Err("nonnegative finite oracle operand required".into());
    }
    let bits = value.to_bits();
    let exponent = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    Ok(if exponent == 0 {
        (fraction, 0)
    } else {
        (fraction | (1_u64 << 52), usize::try_from(exponent - 1)?)
    })
}
/// Work directly with arbitrary-size integer numerators over the common denominator 2^1074.
/// `Rational::new` infers the already linked integer type from its public constructor;
/// only this unit type witness uses CAS, with the unchanged default budget.
/// Midpoint and enclosure comparisons use integer products, never normalized CAS arithmetic.
pub(super) fn check(input: f64, radius: f64, readings: [f64; 3]) -> TestResult {
    let [center, total, error] = readings;
    assert!(center.is_finite());
    assert!(total.is_finite());
    assert!(error.is_finite());
    assert!(error >= 0.0);
    assert!(error <= total);
    let unit = 1_u64.into();
    assert_eq!(
        Rational::new(false, &unit, &unit, &mut Budget::new(Limits::default())?)?,
        Rational::one()
    );
    let integer = |value| -> TestResult<_> {
        let (significand, shift) = binary(value)?;
        Ok((&unit * significand) << shift)
    };
    let radicand = integer(input)?;
    let middle = integer(center)?;
    if input == 0.0 {
        assert_eq!(readings.map(f64::to_bits), [0; 3]);
    } else {
        assert!(center > 0.0);
        let below = &middle + integer(f64::from_bits(center.to_bits() - 1))?;
        let above = &middle + integer(f64::from_bits(center.to_bits() + 1))?;
        // ((y + adjacent)/2)^2 versus x, with denominators cross multiplied exactly.
        let scaled = &radicand << 1076_usize;
        let lower = (&below * &below).cmp(&scaled);
        let upper = scaled.cmp(&(&above * &above));
        assert_ne!(lower, Ordering::Greater);
        assert_ne!(upper, Ordering::Greater);
        if lower == Ordering::Equal || upper == Ordering::Equal {
            assert_eq!(
                center.to_bits() & 1,
                0,
                "midpoint ties select an even significand"
            );
        }
    }
    for (output_radius, input_radius) in [(total, radius), (error, 0.0)] {
        let spread = integer(output_radius)?;
        let input_spread = integer(input_radius)?;
        assert!(radicand >= input_spread);
        let upper = &middle + &spread;
        assert!(&upper * &upper >= ((&radicand + &input_spread) << 1074_usize));
        // A nonpositive stored lower endpoint already contains the nonnegative root.
        if middle > spread {
            let lower = &middle - &spread;
            assert!(&lower * &lower <= ((&radicand - &input_spread) << 1074_usize));
        }
    }
    Ok(())
}
/// Golden powers and the irrational root of two independently exercise integer scaling.
#[test]
fn square_root_oracle_golden_and_rejected_neighbors() -> TestResult {
    for (input, root) in [
        (1.0, 1.0),
        (4.0, 2.0),
        (f64::from_bits(1), f64::from_bits(0x1e60_0000_0000_0000)),
        (
            f64::from_bits(0x7fd0_0000_0000_0000),
            f64::from_bits(0x5fe0_0000_0000_0000),
        ),
    ] {
        check(input, 0.0, [root, 0.0, 0.0])?;
    }
    check(
        2.0,
        0.25,
        [f64::from_bits(0x3ff6_a09e_667f_3bcd), 0.25, f64::EPSILON],
    )?;
    // Every deliberately adjacent candidate violates the exact midpoint-square rounding test.
    for bits in [0x3ff6_a09e_667f_3bcc, 0x3ff6_a09e_667f_3bce] {
        assert!(
            std::panic::catch_unwind(|| check(2.0, 0.0, [f64::from_bits(bits), 1.0, 1.0])).is_err()
        );
    }
    Ok(())
}
