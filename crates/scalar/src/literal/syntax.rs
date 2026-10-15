// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed numeric tokens; no arbitrary-precision numeric representation.

use super::LiteralLimits;
use crate::ArithmeticFailure;

/// One signed significand and its checked effective base-ten exponent.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Decimal<'a> {
    /// Integral significand digits, possibly empty when fractional digits exist.
    pub(crate) whole: &'a str,
    /// Fractional significand digits, with the decimal point already removed.
    pub(crate) fraction: &'a str,
    /// Whether to negate the magnitude after evaluation.
    pub(crate) negative: bool,
    /// Written exponent minus the number of fractional digits.
    pub(crate) exponent: i64,
}

impl Decimal<'_> {
    /// Combined digit count; both slices are disjoint portions of one input string.
    pub(crate) const fn digits(self) -> usize {
        self.whole.len() + self.fraction.len()
    }
}

/// Split a single optional sign without accepting repeated signs.
fn sign(text: &str) -> (bool, &str) {
    text.strip_prefix('-').map_or_else(
        || (false, text.strip_prefix('+').unwrap_or(text)),
        |body| (true, body),
    )
}

/// Validate decimal grammar and both written and effective exponent bounds.
pub(crate) fn decimal(text: &str, limits: LiteralLimits) -> Result<Decimal<'_>, ArithmeticFailure> {
    let (negative, body) = sign(text);
    let (mantissa, written) = body.find(['e', 'E']).map_or_else(
        || Ok::<_, ArithmeticFailure>((body, 0)),
        |position| {
            let (prefix, suffix) = body.split_at(position);
            Ok((
                prefix,
                exponent(
                    suffix.get(1..).ok_or(ArithmeticFailure::InvalidLiteral)?,
                    limits,
                )?,
            ))
        },
    )?;
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if whole.is_empty() && fraction.is_empty() {
        return Err(ArithmeticFailure::InvalidLiteral);
    }
    if !whole
        .bytes()
        .chain(fraction.bytes())
        .all(|byte| byte.is_ascii_digit())
    {
        return Err(ArithmeticFailure::InvalidLiteral);
    }
    let fraction_length =
        i64::try_from(fraction.len()).map_err(|_| ArithmeticFailure::ResourceLimit)?;
    let effective = written
        .checked_sub(fraction_length)
        .ok_or(ArithmeticFailure::ResourceLimit)?;
    if effective.unsigned_abs() > u64::from(limits.max_exponent()) {
        return Err(ArithmeticFailure::ResourceLimit);
    }
    Ok(Decimal {
        whole,
        fraction,
        negative,
        exponent: effective,
    })
}

/// Accumulate a bounded exponent with checked fixed-width metadata arithmetic.
fn exponent(text: &str, limits: LiteralLimits) -> Result<i64, ArithmeticFailure> {
    let (negative, body) = sign(text);
    if body.is_empty() {
        return Err(ArithmeticFailure::InvalidLiteral);
    }
    let mut magnitude = 0_u32;
    for byte in body.bytes() {
        if !byte.is_ascii_digit() {
            return Err(ArithmeticFailure::InvalidLiteral);
        }
        magnitude = magnitude
            .checked_mul(10)
            .and_then(|value| value.checked_add(u32::from(byte - b'0')))
            .ok_or(ArithmeticFailure::ResourceLimit)?;
        if magnitude > limits.max_exponent() {
            return Err(ArithmeticFailure::ResourceLimit);
        }
    }
    let value = i64::from(magnitude);
    Ok(if negative { -value } else { value })
}
