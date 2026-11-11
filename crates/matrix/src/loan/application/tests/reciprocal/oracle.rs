// SPDX-License-Identifier: AGPL-3.0-only
//! Exact integer-rational decoding and ties-to-even division never call scalar enclosures.

use super::TestResult;
use nrz_bindings::{Budget, Limits, readings::algebra::Rational};
use std::cmp::Ordering;

/// Decode all finite binary64 values, including subnormals, from sign/exponent/significand.
fn binary(value: f64, budget: &mut Budget) -> TestResult<Rational> {
    if !value.is_finite() {
        return Err("nonfinite oracle value".into());
    }
    let bits = value.to_bits();
    let field = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1_u64 << 52), i64::try_from(field)? - 1075)
    };
    let mut numerator = significand.into();
    let mut denominator = 1_u64.into();
    if exponent >= 0 {
        numerator <<= usize::try_from(exponent)?;
    } else {
        denominator <<= usize::try_from(exponent.unsigned_abs())?;
    }
    Ok(Rational::new(
        bits >> 63 != 0,
        &numerator,
        &denominator,
        budget,
    )?)
}
/// Divide an exact power of two by the input significand, then round the integer quotient.
/// For a normal result, scaling its 53-bit significand needs at most 105 numerator bits.
/// Subnormal results use the fixed 2^-1074 quantum; finite binary64 divisors need no more bits.
fn nearest(input: f64) -> TestResult<u64> {
    let bits = input.to_bits();
    let field = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1_u64 << 52), i64::try_from(field)? - 1075)
    };
    if significand == 0 || field == 2047 {
        return Err("finite nonzero divisor required".into());
    }
    let width = i64::from(64 - significand.leading_zeros());
    let mut output_exponent = -exponent - width + i64::from(significand.is_power_of_two());
    let quantum = output_exponent.max(-1022) - 52;
    let numerator = 1_u128
        .checked_shl(u32::try_from(-exponent - quantum)?)
        .ok_or("oracle numerator range")?;
    let denominator = u128::from(significand);
    let mut quotient = numerator / denominator;
    let twice_remainder = 2 * (numerator % denominator);
    if twice_remainder > denominator || (twice_remainder == denominator && quotient % 2 != 0) {
        quotient += 1;
    }
    let magnitude = if output_exponent < -1022 {
        u64::try_from(quotient)?
    } else {
        if quotient == 1_u128 << 53 {
            quotient >>= 1;
            output_exponent += 1;
        }
        if output_exponent > 1023 {
            return Err("unbounded reciprocal oracle".into());
        }
        (u64::try_from(output_exponent + 1023)? << 52) | (u64::try_from(quotient)? - (1_u64 << 52))
    };
    Ok(magnitude | (bits & (1_u64 << 63)))
}
/// Both inequalities compare exact dyadic endpoints, without rounded subtraction or addition.
fn covered(value: &Rational, center: &Rational, radius: f64, budget: &mut Budget) -> TestResult {
    let radius = binary(radius, budget)?;
    let lower = center.sub(&radius, budget)?;
    let upper = center.add(&radius, budget)?;
    assert_ne!(lower.compare(value, budget)?, Ordering::Greater);
    assert_ne!(value.compare(&upper, budget)?, Ordering::Greater);
    Ok(())
}
/// Nominal division rounding and full-box image containment are independent obligations.
pub(super) fn check(input: f64, radius: f64, readings: [f64; 3]) -> TestResult {
    let [center, total, error] = readings;
    assert!(center.is_finite());
    assert!(total.is_finite());
    assert!(error.is_finite());
    assert!(error >= 0.0);
    assert!(error <= total);
    let mut budget = Budget::new(Limits::default())?;
    let one = Rational::one();
    let x = binary(input, &mut budget)?;
    let r = binary(radius, &mut budget)?;
    let nominal = one.div(&x, &mut budget)?;
    assert_eq!(center.to_bits(), nearest(input)?);
    let y = binary(center, &mut budget)?;
    covered(&nominal, &y, error, &mut budget)?;
    let lower = one.div(&x.add(&r, &mut budget)?, &mut budget)?;
    let upper = one.div(&x.sub(&r, &mut budget)?, &mut budget)?;
    assert_ne!(lower.compare(&upper, &mut budget)?, Ordering::Greater);
    covered(&lower, &y, total, &mut budget)?;
    covered(&upper, &y, total, &mut budget)
}
/// Independently supplied bounded-box fractions check the oracle's endpoint association.
#[test]
fn reciprocal_oracle_signed_quarters() -> TestResult {
    let mut b = Budget::new(Limits::default())?;
    for (x, numerators) in [(2.0, [4, 4]), (-2.0, [-4, -4])] {
        let denominators = if x > 0.0 { [9, 7] } else { [7, 9] };
        for (offset, (numerator, denominator)) in [0.25, -0.25]
            .into_iter()
            .zip(numerators.into_iter().zip(denominators))
        {
            let endpoint = Rational::one().div(
                &binary(x, &mut b)?.add(&binary(offset, &mut b)?, &mut b)?,
                &mut b,
            )?;
            assert_eq!(
                endpoint,
                Rational::from_ratio(numerator, denominator, &mut b)?
            );
        }
    }
    Ok(())
}
/// Independent encodings pin normal rounding, exact powers and the subnormal result quantum.
#[test]
fn reciprocal_integer_rounding_golden_encodings() -> TestResult {
    for (input, bits) in [
        (1.0, 0x3ff0_0000_0000_0000),
        (2.0, 0x3fe0_0000_0000_0000),
        (3.0, 0x3fd5_5555_5555_5555),
        (5.0, 0x3fc9_9999_9999_999a),
        (f64::from_bits(1_u64 << 51), 0x7fe0_0000_0000_0000),
        (f64::from_bits(0x7fe0_0000_0000_0000), 1_u64 << 51),
    ] {
        assert_eq!(nearest(input)?, bits);
        assert_eq!(nearest(-input)?, bits | (1_u64 << 63));
    }
    Ok(())
}
