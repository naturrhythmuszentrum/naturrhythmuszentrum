// SPDX-License-Identifier: AGPL-3.0-only
//! Integer-only canonical decimal validation for summary readers.

use super::{DecimalError, Integer};

/// Validate a fixed-grid literal without parsing a floating-point value.
/// # Errors
/// Refuses alternate spellings, negative zero, incorrect precision and overflow.
pub fn validate_fixed(text: &str, power: i32) -> Result<(), DecimalError> {
    if !(-326..=309).contains(&power) || text.len() > 637 {
        return Err(DecimalError::Grid);
    }
    let magnitude = text.strip_prefix('-').unwrap_or(text);
    let negative = magnitude.len() != text.len();
    let places = if power < 0 {
        power.unsigned_abs() as usize
    } else {
        0
    };
    let (whole, fraction) = match magnitude.split_once('.') {
        Some(parts) if places != 0 => parts,
        None if places == 0 => (magnitude, ""),
        Some(_) | None => return Err(DecimalError::Source),
    };
    let canonical_whole = whole == "0" || !whole.starts_with('0');
    if whole.is_empty() || !canonical_whole || fraction.len() != places {
        return Err(DecimalError::Source);
    }
    let mut coefficient = Integer::ZERO;
    for digit in whole.bytes().chain(fraction.bytes()) {
        if !digit.is_ascii_digit() {
            return Err(DecimalError::Source);
        }
        coefficient = coefficient
            .mul(Integer::from_u64(10))?
            .add(Integer::from_u64(u64::from(digit - b'0')))?;
    }
    if negative && coefficient.is_zero() {
        return Err(DecimalError::Source);
    }
    Ok(())
}

/// Validate six-significant-figure canonical spelling using integer syntax only.
/// # Errors
/// Refuses alternate exponents, wrong digit counts, negative zero and out-of-range exponents.
pub fn validate_significant(text: &str) -> Result<(), DecimalError> {
    if text == "0.00000e0" {
        return Ok(());
    }
    let magnitude = text.strip_prefix('-').unwrap_or(text);
    let (mantissa, exponent) = magnitude.split_once('e').ok_or(DecimalError::Source)?;
    let bytes = mantissa.as_bytes();
    if bytes.len() != 7 {
        return Err(DecimalError::Source);
    }
    let leading = (b'1'..=b'9').contains(&bytes[0]);
    let point = bytes[1] == b'.';
    let digits = bytes[2..].iter().all(u8::is_ascii_digit);
    if !(leading && point && digits) {
        return Err(DecimalError::Source);
    }
    let exponent_magnitude = exponent.strip_prefix('-').unwrap_or(exponent);
    let canonical = exponent_magnitude == "0" || !exponent_magnitude.starts_with('0');
    let digits =
        !exponent_magnitude.is_empty() && exponent_magnitude.bytes().all(|b| b.is_ascii_digit());
    if !(canonical && digits) || exponent == "-0" {
        return Err(DecimalError::Source);
    }
    let exponent = exponent.parse::<i32>().map_err(|_| DecimalError::Source)?;
    if !(-324..=340).contains(&exponent) {
        return Err(DecimalError::Grid);
    }
    Ok(())
}

/// Parse decimal-text telemetry directly into its retained coefficient and scale.
/// # Errors
/// Rejects exponent notation, more than nine fractional digits or coefficient overflow.
pub fn decimal_source(text: &str) -> Result<(i64, u32), DecimalError> {
    let magnitude = text.strip_prefix('-').unwrap_or(text);
    let negative = magnitude.len() != text.len();
    let (whole, fraction) = magnitude.split_once('.').unwrap_or((magnitude, ""));
    let scale = u32::try_from(fraction.len()).map_err(|_| DecimalError::Source)?;
    if whole.is_empty() || scale > 9 {
        return Err(DecimalError::Source);
    }
    let mut coefficient = 0_i64;
    for digit in whole.bytes().chain(fraction.bytes()) {
        if !digit.is_ascii_digit() {
            return Err(DecimalError::Source);
        }
        coefficient = coefficient
            .checked_mul(10)
            .and_then(|n| n.checked_add(i64::from(digit - b'0')))
            .filter(|n| n.unsigned_abs() <= super::MAX_INTEGER)
            .ok_or(DecimalError::Source)?;
    }
    Ok((if negative { -coefficient } else { coefficient }, scale))
}
