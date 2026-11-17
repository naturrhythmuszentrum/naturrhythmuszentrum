// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact binary rounding and alternating-series test arithmetic.
use super::{Budget, Rational, TestResult, number, power};
use std::cmp::Ordering;
/// Exact value of finite positive binary64 bits, without a floating operation.
pub(super) fn value(bits: u64, b: &mut Budget) -> TestResult<Rational> {
    if bits >= 0x7ff0_0000_0000_0000 {
        return Err("oracle cannot decode a nonfinite encoding".into());
    }
    let field = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction + (1_u64 << 52), i64::try_from(field)? - 1075)
    };
    Ok(Rational::from_ratio(i64::try_from(significand)?, 1, b)?
        .mul(&Rational::from_ratio(2, 1, b)?.pow(exponent, b)?, b)?)
}

/// Binary search over the ordered finite encodings, then compare exact distances.
pub(super) fn nearest(q: &Rational, b: &mut Budget) -> TestResult<Rational> {
    let negative = q.is_negative();
    let magnitude = absolute(q, b)?;
    let bits = nearest_positive_bits(&magnitude, b)?.ok_or("nearest binary64 overflow")?;
    let chosen = value(bits, b)?;
    Ok(if negative { chosen.negated(b)? } else { chosen })
}

/// Independently find a nonnegative nearest encoding across the full finite range.
/// None means nearest binary64 overflows.
pub(super) fn nearest_positive_bits(q: &Rational, b: &mut Budget) -> TestResult<Option<u64>> {
    let maximum = 0x7fef_ffff_ffff_ffff_u64;
    if value(maximum, b)?.compare(q, b)? == Ordering::Less {
        let threshold = power(1024)?.sub(&power(970)?, b)?;
        return Ok(if q.compare(&threshold, b)? == Ordering::Less {
            Some(maximum)
        } else {
            None
        });
    }
    let mut low = 0;
    let mut high = maximum;
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if value(middle, b)?.compare(q, b)? == Ordering::Greater {
            high = middle;
        } else {
            low = middle;
        }
    }
    let lower = value(low, b)?;
    let upper = value(high, b)?;
    let left = q.sub(&lower, b)?;
    let right = upper.sub(q, b)?;
    let order = left.compare(&right, b)?;
    Ok(Some(
        if order == Ordering::Greater || (order == Ordering::Equal && low % 2 != 0) {
            high
        } else {
            low
        },
    ))
}

/// Exact alternating partial sums bracket exp(-a)-1; the next term bounds the remainder.
pub(super) fn truth(a: &Rational, b: &mut Budget) -> TestResult<[Rational; 2]> {
    if a.is_zero() {
        return Ok([Rational::zero(), Rational::zero()]);
    }
    let negative = a.negated(b)?;
    let threshold = a.mul(&power(-160)?, b)?;
    let mut term = negative.clone();
    let mut sum = term.clone();
    for n in 1..192 {
        let next = term.mul(&negative, b)?.div(&number(n + 1, 1)?, b)?;
        let adjacent = sum.add(&next, b)?;
        if n >= 2 && absolute(&next, b)?.compare(&threshold, b)? != Ordering::Greater {
            return Ok(if next.is_negative() {
                [adjacent, sum]
            } else {
                [sum, adjacent]
            });
        }
        sum = adjacent;
        term = next;
    }
    Err("oracle did not reach its proved remainder threshold".into())
}

/// Exact absolute value for independent test arithmetic.
pub(super) fn absolute(q: &Rational, b: &mut Budget) -> TestResult<Rational> {
    Ok(if q.is_negative() {
        q.negated(b)?
    } else {
        q.clone()
    })
}

/// Require an exact inequality rather than comparing rounded decimal output.
/// # Panics
/// Panics if the independently computed inequality fails.
pub(super) fn le(a: &Rational, c: &Rational, b: &mut Budget) -> TestResult {
    assert_ne!(a.compare(c, b)?, Ordering::Greater);
    Ok(())
}

/// Find the neighbouring exact binary64 value of an already rounded number.
/// # Panics
/// Panics if its caller supplies an unrounded value.
pub(super) fn neighbour(q: &Rational, upward: bool, b: &mut Budget) -> TestResult<Rational> {
    if q.is_zero() {
        return Ok(if upward {
            power(-1074)?
        } else {
            power(-1074)?.negated(b)?
        });
    }
    let magnitude = absolute(q, b)?;
    let mut low = 0;
    let mut high = 0x7ff0_0000_0000_0000_u64;
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if value(middle, b)?.compare(&magnitude, b)? == Ordering::Greater {
            high = middle;
        } else {
            low = middle;
        }
    }
    assert_eq!(value(low, b)?, magnitude);
    let bits = if upward == q.is_negative() {
        low - 1
    } else {
        low + 1
    };
    let result = value(bits, b)?;
    Ok(if q.is_negative() {
        result.negated(b)?
    } else {
        result
    })
}
