// SPDX-License-Identifier: AGPL-3.0-only
//! Independent bounded IEEE decoding and integer norm comparisons for the mixed component boxes.

use super::TestResult;

/// Compare a represented upper bound with a separately specified positive rational endpoint.
pub(super) fn above(upper: f64, numerator: i128, denominator: i128) -> TestResult<bool> {
    compare(upper, numerator, denominator, false)
}

/// Square the returned IEEE bound exactly and compare with the rational sum of squared component radii.
pub(super) fn square_above(upper: f64, numerator: i128, denominator: i128) -> TestResult<bool> {
    compare(upper, numerator, denominator, true)
}

/// These independent fixture bounds lie strictly between zero and two and need no unbounded integers.
fn compare(upper: f64, numerator: i128, denominator: i128, square: bool) -> TestResult<bool> {
    let bits = upper.to_bits();
    let exponent = i32::try_from((bits >> 52) & 2047)?;
    if bits >> 63 != 0 || !(1010..=1023).contains(&exponent) {
        return Err("norm oracle range".into());
    }
    let mut top = i128::from(bits & ((1_u64 << 52) - 1)) + (1_i128 << 52);
    let mut bottom = 1_i128
        .checked_shl(u32::try_from(1075 - exponent)?)
        .ok_or("norm oracle range")?;
    if square {
        top = top.checked_mul(top).ok_or("norm oracle overflow")?;
        bottom = bottom.checked_mul(bottom).ok_or("norm oracle overflow")?;
    }
    let left = top.checked_mul(denominator).ok_or("norm oracle overflow")?;
    let right = bottom
        .checked_mul(numerator)
        .ok_or("norm oracle overflow")?;
    Ok(left >= right)
}
