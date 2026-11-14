// SPDX-License-Identifier: AGPL-3.0-only
//! Independent rational examples from the design, never live timing assertions.

use super::{DecimalError, Exact, Integer, MAX_INTEGER, Rounding, fixed, significant};

/// Exact construction from finite test constants, not from printed decimals.
fn source(value: f64) -> Result<Exact, DecimalError> {
    Exact::from_bits(value.to_bits())
}

#[test]
fn confidence_tie_and_negative_tie() -> Result<(), DecimalError> {
    assert_eq!(
        fixed(source(1.0 / 128.0)?, -6, Rounding::Nearest)?,
        "0.007813"
    );
    assert_eq!(
        fixed(source(-1.0 / 128.0)?, -6, Rounding::Nearest)?,
        "-0.007813"
    );
    assert_eq!(fixed(source(-0.0)?, 0, Rounding::Lower)?, "0");
    Ok(())
}

#[test]
fn worked_shared_estimate_grid() -> Result<(), DecimalError> {
    let point = source(0.125)?;
    let error = source(2_f64.powi(-20))?;
    let upper = source(129.0 / 1024.0)?;
    let grid = Exact::estimate_grid(point, error, point, upper)?;
    assert_eq!(grid, -9);
    assert_eq!(fixed(point, grid, Rounding::Nearest)?, "0.125000000");
    assert_eq!(fixed(upper, grid, Rounding::Upper)?, "0.125976563");
    assert_eq!(fixed(error, grid, Rounding::Nearest)?, "0.000000954");
    Ok(())
}

#[test]
fn small_dispersion_is_not_zero() -> Result<(), DecimalError> {
    let point = source(2_f64.powi(-13))?;
    let error = source(2_f64.powi(-42))?;
    let upper = source(2_f64.powi(-13) + 2_f64.powi(-40))?;
    let grid = Exact::estimate_grid(point, error, point, upper)?;
    assert_eq!(grid, -15);
    assert_eq!(fixed(error, grid, Rounding::Nearest)?, "0.000000000000227");
    Ok(())
}

#[test]
fn exact_reciprocal_tie() -> Result<(), DecimalError> {
    let rate = source(2.0)?.reciprocal(2_469_130)?;
    assert_eq!(significant(rate, Rounding::Nearest)?, "1.23457e6");
    assert_eq!(significant(rate, Rounding::Lower)?, "1.23456e6");
    assert_eq!(significant(rate, Rounding::Upper)?, "1.23457e6");
    assert_eq!(
        fixed(Exact::decimal(150, 2)?, -2, Rounding::Nearest)?,
        "1.50"
    );
    Ok(())
}

#[test]
fn extrema_and_exponent_carry() -> Result<(), DecimalError> {
    let tiny = Exact::from_bits(1)?;
    assert_eq!(tiny.exponent()?, -324);
    assert_eq!(significant(tiny, Rounding::Nearest)?, "4.94066e-324");
    assert_eq!(
        significant(source(f64::MAX)?, Rounding::Upper)?,
        "1.79770e308"
    );
    assert_eq!(
        significant(Exact::ratio(9_999_999, 10)?, Rounding::Nearest)?,
        "1.00000e6"
    );
    let wide = Exact::width(source(-f64::MAX)?, source(f64::MAX)?)?;
    assert_eq!(wide.exponent()?, 308);
    assert_eq!(
        Exact::from_bits(1)?.reciprocal(MAX_INTEGER)?.exponent()?,
        339
    );
    Ok(())
}

#[test]
fn zero_width_and_excluded_point() -> Result<(), DecimalError> {
    assert_eq!(
        Exact::estimate_grid(source(0.0)?, source(0.0)?, source(0.0)?, source(0.0)?)?,
        -9
    );
    assert_eq!(
        Exact::estimate_grid(source(1.0)?, source(0.0)?, source(2.0)?, source(2.0)?)?,
        -2
    );
    assert_eq!(
        Exact::width(source(2.0)?, source(1.0)?),
        Err(DecimalError::Source)
    );
    Ok(())
}

#[test]
fn representation_refusals() {
    assert_eq!(
        Exact::from_bits(f64::INFINITY.to_bits()),
        Err(DecimalError::Source)
    );
    assert_eq!(
        Exact::from_bits(f64::NAN.to_bits()),
        Err(DecimalError::Source)
    );
    assert_eq!(Exact::ratio(MAX_INTEGER + 1, 1), Err(DecimalError::Source));
    assert_eq!(Exact::ratio(1, 0), Err(DecimalError::Denominator));
    assert_eq!(Exact::decimal(1, 10), Err(DecimalError::Source));
}

#[test]
fn fixed_limb_boundary_and_division() -> Result<(), DecimalError> {
    let one = Integer::from_u64(1);
    assert!(one.shift(3455).is_ok());
    assert_eq!(one.shift(3456), Err(DecimalError::Capacity));
    let top = one.shift(3455)?;
    assert_eq!(top.add(top), Err(DecimalError::Capacity));
    let numerator = Integer::ten_pow(329)?.mul(one.shift(1126)?)?;
    let divisor = Integer::ten_pow(334)?;
    let (q, r) = numerator.div_rem(divisor)?;
    assert_eq!(q.mul(divisor)?.add(r)?, numerator);
    assert!(r < divisor);
    Ok(())
}

#[test]
fn canonical_literals_refuse_alternate_spellings() {
    for text in [
        "+1.00", "01.00", "1.0", "1.000", "-0.00", "1e0", " 1.00", "1.0x",
    ] {
        assert!(super::validate_fixed(text, -2).is_err(), "{text}");
    }
    for text in [
        "1.00000e+0",
        "1.00000e00",
        "1.00000e-0",
        "0.00000e1",
        "-0.00000e0",
        "1.0000e0",
        "1.00000E0",
    ] {
        assert!(super::validate_significant(text).is_err(), "{text}");
    }
    assert_eq!(super::validate_fixed("1.00", -2), Ok(()));
    assert_eq!(super::validate_significant("1.00000e0"), Ok(()));
}

#[test]
fn telemetry_fraction_grids_preserve_small_nonzero_values() -> Result<(), DecimalError> {
    let value = Exact::utilization(1, 1_000_000_000, 8)?;
    let grid = value.telemetry_grid(-4, Exact::ratio(1, 10_000)?)?;
    assert_eq!(grid, -9);
    assert_eq!(fixed(value, grid, Rounding::Nearest)?, "0.000000125");
    let pressure = Exact::decimal(1, 2)?;
    assert_eq!(fixed(pressure, -2, Rounding::Nearest)?, "0.01");
    assert_eq!(super::decimal_source("1.50")?, (150, 2));
    assert_eq!(Exact::utilization(1, 0, 8), Err(DecimalError::Denominator));
    assert_eq!(Exact::utilization(1, 1, 0), Err(DecimalError::Denominator));
    Ok(())
}

#[test]
fn binary64_views_are_nearest_even_and_do_not_feed_decimal_rounding() -> Result<(), DecimalError> {
    assert_eq!(Exact::ratio(1, 3)?.binary64_bits()?, 0x3fd5_5555_5555_5555);
    assert_eq!(Exact::ratio(1, 10)?.binary64_bits()?, 0x3fb9_9999_9999_999a);
    for bits in [
        1,
        2,
        0x0010_0000_0000_0000,
        0x3ff0_0000_0000_0001,
        0x7fef_ffff_ffff_ffff,
        0xbff0_0000_0000_0000,
    ] {
        assert_eq!(Exact::from_bits(bits)?.binary64_bits()?, bits);
    }
    assert_eq!(Exact::from_bits(1_u64 << 63)?.binary64_bits()?, 0);
    assert_eq!(
        Exact::from_bits(1)?
            .reciprocal(MAX_INTEGER)?
            .binary64_bits(),
        Err(DecimalError::Source)
    );
    Ok(())
}

#[test]
fn fixed_control_median_grid_preserves_half_nanoseconds_at_count_limit() -> Result<(), DecimalError>
{
    assert_eq!(
        fixed(Exact::midpoint_nanos(1, 2)?, -10, Rounding::Nearest)?,
        "0.0000000015"
    );
    assert_eq!(
        fixed(
            Exact::midpoint_nanos(MAX_INTEGER, MAX_INTEGER)?,
            -10,
            Rounding::Nearest
        )?,
        "9007199.2547409910"
    );
    Ok(())
}
