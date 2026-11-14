// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational rectangles and independent integer nearest-even rounding have separate obligations.

mod integers;
use super::{BinaryElementwise, TestResult};
use integers::Natural;
use std::cmp::Ordering;

/// Exact unreduced signed fraction, kept separate from production scalar and CAS code.
#[derive(Clone)]
pub(super) struct Rational {
    negative: bool,
    numerator: Natural,
    denominator: Natural,
}
impl Rational {
    /// Decode sign, significand and exponent from binary64 bits.
    pub(super) fn binary(value: f64) -> TestResult<Self> {
        let (negative, significand, exponent) = decode(value)?;
        let mut numerator = Natural::new(significand);
        let mut denominator = Natural::new(1);
        if exponent < 0 {
            denominator = denominator.shift(usize::try_from(-exponent)?);
        } else {
            numerator = numerator.shift(usize::try_from(exponent)?);
        }
        Ok(Self {
            negative: negative && significand != 0,
            numerator,
            denominator,
        })
    }
    /// Signed addition uses exact cross products; no reduction or budget is needed.
    pub(super) fn add(&self, other: &Self) -> Self {
        let left = self.numerator.mul(&other.denominator);
        let right = other.numerator.mul(&self.denominator);
        let (negative, numerator) = if self.negative == other.negative {
            (self.negative, left.add(&right))
        } else if left.compare(&right) == Ordering::Less {
            (other.negative, right.sub(&left))
        } else {
            (self.negative, left.sub(&right))
        };
        Self {
            negative: negative && !numerator.is_zero(),
            numerator,
            denominator: self.denominator.mul(&other.denominator),
        }
    }
    /// Exact subtraction retains canonical sign for zero.
    pub(super) fn sub(&self, other: &Self) -> Self {
        let mut opposite = other.clone();
        opposite.negative = !opposite.negative && !opposite.numerator.is_zero();
        self.add(&opposite)
    }
    /// Ordered rational images directly multiply or divide the two exact operands.
    pub(super) fn image(&self, other: &Self, function: BinaryElementwise) -> TestResult<Self> {
        let (numerator, denominator) = match function {
            BinaryElementwise::Product => (
                self.numerator.mul(&other.numerator),
                self.denominator.mul(&other.denominator),
            ),
            BinaryElementwise::Quotient => {
                if other.numerator.is_zero() {
                    return Err("oracle zero divisor".into());
                }
                (
                    self.numerator.mul(&other.denominator),
                    self.denominator.mul(&other.numerator),
                )
            }
        };
        Ok(Self {
            negative: (self.negative != other.negative) && !numerator.is_zero(),
            numerator,
            denominator,
        })
    }
    /// Cross-multiplied numerators prove containment without rounded endpoint arithmetic.
    pub(super) fn compare(&self, other: &Self) -> Ordering {
        if self.negative != other.negative {
            return other.negative.cmp(&self.negative);
        }
        let order = self
            .numerator
            .mul(&other.denominator)
            .compare(&other.numerator.mul(&self.denominator));
        if self.negative {
            order.reverse()
        } else {
            order
        }
    }
}

/// Decode finite centers without invoking any platform multiplication or division.
fn decode(value: f64) -> TestResult<(bool, u64, i32)> {
    if !value.is_finite() {
        return Err("finite oracle operand required".into());
    }
    let bits = value.to_bits();
    let field = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1_u64 << 52), i32::try_from(field)? - 1075)
    };
    Ok((bits >> 63 != 0, significand, exponent))
}

/// Exact center operands have at most 106 significand bits; integer quotient/remainder proves RN.
pub(super) fn nearest(left: f64, right: f64, function: BinaryElementwise) -> TestResult<u64> {
    nearest_format(left, right, function, 52, 1023)
}

/// Integer significands are rounded directly to the target format, without double rounding.
fn nearest_format(
    left: f64,
    right: f64,
    function: BinaryElementwise,
    fraction: i32,
    bias: i32,
) -> TestResult<u64> {
    let (left_sign, left_magnitude, left_exponent) = decode(left)?;
    let (right_sign, right_magnitude, right_exponent) = decode(right)?;
    let (numerator, denominator, exponent) = match function {
        BinaryElementwise::Product => (
            u128::from(left_magnitude) * u128::from(right_magnitude),
            1,
            left_exponent + right_exponent,
        ),
        BinaryElementwise::Quotient => (
            u128::from(left_magnitude),
            u128::from(right_magnitude),
            left_exponent - right_exponent,
        ),
    };
    if denominator == 0 {
        return Err("oracle zero divisor".into());
    }
    if numerator == 0 {
        return Ok(0);
    }
    let difference = i32::try_from(128 - numerator.leading_zeros())?
        - i32::try_from(128 - denominator.leading_zeros())?;
    let below = if difference >= 0 {
        numerator < (denominator << u32::try_from(difference)?)
    } else {
        (numerator << u32::try_from(-difference)?) < denominator
    };
    let mut output_exponent = exponent + difference - i32::from(below);
    if output_exponent > bias {
        return Err("oracle overflow".into());
    }
    let quantum = output_exponent.max(1 - bias) - fraction;
    let shift = exponent - quantum;
    let (scaled, divisor) = if shift >= 0 {
        (
            numerator
                .checked_shl(u32::try_from(shift)?)
                .ok_or("oracle numerator range")?,
            denominator,
        )
    } else {
        let width = i32::try_from(128 - denominator.leading_zeros())? - shift;
        if width > i32::try_from(128 - numerator.leading_zeros())? + 1 {
            return Ok(0);
        }
        (
            numerator,
            denominator
                .checked_shl(u32::try_from(-shift)?)
                .ok_or("oracle denominator range")?,
        )
    };
    let mut quotient = scaled / divisor;
    let twice_remainder = 2 * (scaled % divisor);
    if twice_remainder > divisor || (twice_remainder == divisor && quotient % 2 != 0) {
        quotient += 1;
    }
    let magnitude = if output_exponent < 1 - bias {
        u64::try_from(quotient)?
    } else {
        if quotient == 1_u128 << (fraction + 1) {
            quotient >>= 1;
            output_exponent += 1;
        }
        if output_exponent > bias {
            return Err("oracle rounded overflow".into());
        }
        (u64::try_from(output_exponent + bias)? << fraction)
            | (u64::try_from(quotient)? - (1_u64 << fraction))
    };
    Ok(if magnitude == 0 {
        0
    } else {
        magnitude
            | (u64::from(left_sign != right_sign)
                << (fraction + if fraction == 52 { 11 } else { 8 }))
    })
}

/// Nominal rounding and containment are independently checked against the stored center.
pub(super) fn check(
    centers: [f64; 2],
    radii: [f64; 2],
    function: BinaryElementwise,
    readings: [f64; 3],
) -> TestResult {
    assert_eq!(
        readings[0].to_bits(),
        nearest(centers[0], centers[1], function)?
    );
    containment(centers, radii, function, readings)
}
/// Native RN and both radius ceilings use exact rational differences. Binary32 permits
/// eight relative epsilons for outward endpoint construction, image and displacement;
/// binary64 small fixtures retain the fixed dyadic ceiling below radius increments.
pub(super) fn tight_native<P: nrz_scalar::Scalar>(
    centers: [f64; 2],
    radii: [f64; 2],
    function: BinaryElementwise,
    readings: [f64; 3],
) -> TestResult {
    let binary32 = P::PRECISION == nrz_scalar::Precision::F32;
    let expected = if binary32 {
        f64::from(f32::from_bits(u32::try_from(nearest_format(
            centers[0], centers[1], function, 23, 127,
        )?)?))
    } else {
        f64::from_bits(nearest(centers[0], centers[1], function)?)
    };
    assert_eq!(readings[0].to_bits(), expected.to_bits());
    containment(centers, radii, function, readings)?;
    let left = Rational::binary(centers[0])?;
    let right = Rational::binary(centers[1])?;
    let left_radius = Rational::binary(radii[0])?;
    let right_radius = Rational::binary(radii[1])?;
    let output = Rational::binary(readings[0])?;
    let mut maximum = Rational::binary(0.0)?;
    for first in [left.sub(&left_radius), left.add(&left_radius)] {
        for second in [right.sub(&right_radius), right.add(&right_radius)] {
            let mut distance = first.image(&second, function)?.sub(&output);
            distance.negative = false;
            if distance.compare(&maximum) == Ordering::Greater {
                maximum = distance;
            }
        }
    }
    let mut nominal = left.image(&right, function)?.sub(&output);
    nominal.negative = false;
    let allowance = if binary32 {
        let mut magnitude = output;
        magnitude.negative = false;
        magnitude
            .add(&maximum)
            .image(
                &Rational::binary(1.0 / 1_048_576.0)?,
                BinaryElementwise::Product,
            )?
            .add(&Rational::binary(f64::from(f32::from_bits(8)))?)
    } else {
        Rational::binary(1.0 / 1_048_576.0)?
    };
    assert_ne!(
        Rational::binary(readings[2])?.compare(&nominal.add(&allowance)),
        Ordering::Greater
    );
    let ceiling = maximum.add(&allowance);
    assert_ne!(
        Rational::binary(readings[1])?.compare(&ceiling),
        Ordering::Greater
    );
    Ok(())
}

/// Exact nominal and four-corner checks remain valid even for a deliberately wrong nearby center.
pub(super) fn containment(
    centers: [f64; 2],
    radii: [f64; 2],
    function: BinaryElementwise,
    [center, total, error]: [f64; 3],
) -> TestResult {
    assert!(center.is_finite());
    assert!(total.is_finite());
    assert!(error.is_finite());
    assert!(error >= 0.0);
    assert!(error <= total);
    let left = Rational::binary(centers[0])?;
    let right = Rational::binary(centers[1])?;
    let left_radius = Rational::binary(radii[0])?;
    let right_radius = Rational::binary(radii[1])?;
    let output = Rational::binary(center)?;
    covered(&left.image(&right, function)?, &output, error)?;
    for first in [left.sub(&left_radius), left.add(&left_radius)] {
        for second in [right.sub(&right_radius), right.add(&right_radius)] {
            covered(&first.image(&second, function)?, &output, total)?;
        }
    }
    Ok(())
}
/// The output interval is an exact dyadic interval around the actual stored center.
fn covered(value: &Rational, center: &Rational, radius: f64) -> TestResult {
    let radius = Rational::binary(radius)?;
    assert_ne!(center.sub(&radius).compare(value), Ordering::Greater);
    assert_ne!(value.compare(&center.add(&radius)), Ordering::Greater);
    Ok(())
}

/// Exact golden encodings include both tie parities and the fixed subnormal quantum.
#[test]
fn binary_nearest_integer_goldens_and_wrong_adjacent_centers() -> TestResult {
    use BinaryElementwise::{Product, Quotient};
    for (left, right, function, bits) in [
        (1.0, 3.0, Quotient, 0x3fd5_5555_5555_5555),
        (1.1, 1.1, Product, 0x3ff3_5c28_f5c2_8f5d),
        (
            1.5,
            f64::from_bits(0x3ff0_0000_0000_0001),
            Product,
            0x3ff8_0000_0000_0002,
        ),
        (
            1.5,
            f64::from_bits(0x3ff0_0000_0000_0003),
            Product,
            0x3ff8_0000_0000_0004,
        ),
        (f64::from_bits(1), 0.5, Product, 0),
        (f64::from_bits(3), 0.5, Product, 2),
        (f64::from_bits(5), 0.5, Product, 2),
        (f64::from_bits(7), 0.5, Product, 4),
        (f64::from_bits(3), 2.0, Quotient, 2),
        (f64::from_bits(5), 2.0, Quotient, 2),
    ] {
        assert_eq!(nearest(left, right, function)?, bits);
        assert_eq!(
            nearest(-left, right, function)?,
            if bits == 0 { 0 } else { bits | (1_u64 << 63) }
        );
    }
    for (centers, function, wrong) in [
        ([1.0, 3.0], Quotient, 0x3fd5_5555_5555_5556),
        ([1.1, 1.1], Product, 0x3ff3_5c28_f5c2_8f5e),
    ] {
        let readings = [f64::from_bits(wrong), 1e-14, 1e-14];
        containment(centers, [0.0; 2], function, readings)?;
        assert!(std::panic::catch_unwind(|| check(centers, [0.0; 2], function, readings)).is_err());
    }
    Ok(())
}

/// Independent binary32 encodings exercise midpoint parity, signs and its subnormal quantum.
#[test]
fn binary32_nearest_integer_goldens_and_wrong_adjacent_centers() -> TestResult {
    use BinaryElementwise::{Product, Quotient};
    for (left, right, function, expected) in [
        (1.0_f32, 3.0_f32, Quotient, 0x3eaa_aaab),
        (1.5, f32::from_bits(0x3f80_0001), Product, 0x3fc0_0002),
        (1.5, f32::from_bits(0x3f80_0003), Product, 0x3fc0_0004),
        (f32::from_bits(1), 0.5, Product, 0),
        (f32::from_bits(3), 0.5, Product, 2),
        (f32::from_bits(5), 0.5, Product, 2),
        (f32::from_bits(7), 0.5, Product, 4),
        (f32::from_bits(3), 2.0, Quotient, 2),
        (f32::from_bits(5), 2.0, Quotient, 2),
    ] {
        assert_eq!(
            nearest_format(f64::from(left), f64::from(right), function, 23, 127)?,
            expected
        );
        assert_eq!(
            nearest_format(-f64::from(left), f64::from(right), function, 23, 127)?,
            if expected == 0 {
                0
            } else {
                expected | (1_u64 << 31)
            }
        );
    }
    let centers = [1.0, 3.0];
    let radii = [0.0; 2];
    let wrong = [f64::from(f32::from_bits(0x3eaa_aaaa)), 1e-7, 1e-7];
    containment(centers, radii, Quotient, wrong)?;
    assert!(
        std::panic::catch_unwind(|| tight_native::<f32>(centers, radii, Quotient, wrong)).is_err()
    );
    let oversized = [f64::from(f32::from_bits(0x3eaa_aaab)), 0.25, 0.25];
    containment(centers, radii, Quotient, oversized)?;
    assert!(
        std::panic::catch_unwind(|| tight_native::<f32>(centers, radii, Quotient, oversized))
            .is_err()
    );
    Ok(())
}
