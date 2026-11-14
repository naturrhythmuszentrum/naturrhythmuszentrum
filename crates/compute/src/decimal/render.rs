// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical decimal spellings from exact integer coefficients.

use super::{DecimalError, Exact, Rounding};
use std::fmt::Write;

/// Render fixed-grid notation; positive powers denote explicitly scaled units.
///
/// A power of -9 means seconds at nanosecond resolution. A positive power means
/// a coefficient in units of ten to that power, with no decimal point.
/// # Errors
/// Rejects grids outside -326 through 309 or exact arithmetic overflow.
pub fn fixed(value: Exact, power: i32, role: Rounding) -> Result<String, DecimalError> {
    if !(-326..=309).contains(&power) {
        return Err(DecimalError::Grid);
    }
    let (negative, coefficient) = value.coefficient(power, role)?;
    let digits = coefficient.digits()?;
    let places = if power < 0 {
        power.unsigned_abs() as usize
    } else {
        0
    };
    let mut output = String::new();
    if negative {
        output.push('-');
    }
    if places == 0 {
        output.push_str(&digits);
    } else if digits.len() <= places {
        output.push_str("0.");
        output.extend(std::iter::repeat_n('0', places - digits.len()));
        output.push_str(&digits);
    } else {
        let split = digits.len() - places;
        output.push_str(&digits[..split]);
        output.push('.');
        output.push_str(&digits[split..]);
    }
    Ok(output)
}

/// Render six significant figures, including exponent carries and directed bounds.
/// # Errors
/// Rejects sources or arithmetic outside the designed representation.
pub fn significant(value: Exact, role: Rounding) -> Result<String, DecimalError> {
    if value.is_zero() {
        return Ok("0.00000e0".to_owned());
    }
    let mut exponent = value.exponent()?;
    let (negative, coefficient) = value.coefficient(exponent - 5, role)?;
    let mut digits = coefficient.digits()?;
    if digits.len() == 7 {
        digits.pop();
        exponent += 1;
    }
    if digits.len() != 6 {
        return Err(DecimalError::Grid);
    }
    let mut output = String::new();
    if negative {
        output.push('-');
    }
    write!(output, "{}.{}e{exponent}", &digits[..1], &digits[1..])
        .map_err(|_| DecimalError::Capacity)?;
    Ok(output)
}
