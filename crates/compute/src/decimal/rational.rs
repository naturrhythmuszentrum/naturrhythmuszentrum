// SPDX-License-Identifier: AGPL-3.0-only
//! Exact signed source rationals and integer-only grid choice.

use super::{DecimalError, Integer, MAX_INTEGER, Rounding};
use std::cmp::Ordering;

/// Exact signed rational with a positive denominator and normalized zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Exact {
    /// Magnitude numerator.
    numerator: Integer,
    /// Positive denominator.
    denominator: Integer,
    /// Sign, false for zero.
    negative: bool,
}

impl Exact {
    /// Reduce a source to avoid unnecessary denominator growth.
    fn new(
        numerator: &Integer,
        denominator: &Integer,
        negative: bool,
    ) -> Result<Self, DecimalError> {
        if denominator.is_zero() {
            return Err(DecimalError::Denominator);
        }
        let divisor = numerator.gcd(*denominator)?;
        Ok(Self {
            numerator: numerator.div_rem(divisor)?.0,
            denominator: denominator.div_rem(divisor)?.0,
            negative: negative && !numerator.is_zero(),
        })
    }

    /// Decode a finite binary64 source without decimal parsing.
    /// # Errors
    /// Rejects NaN, infinity and fixed-integer overflow.
    pub fn from_bits(bits: u64) -> Result<Self, DecimalError> {
        let exponent = i32::try_from((bits >> 52) & 0x7ff).map_err(|_| DecimalError::Source)?;
        if exponent == 0x7ff {
            return Err(DecimalError::Source);
        }
        let fraction = bits & ((1_u64 << 52) - 1);
        let (mantissa, shift) = if exponent == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1_u64 << 52), exponent - 1075)
        };
        let mut numerator = Integer::from_u64(mantissa);
        let mut denominator = Integer::from_u64(1);
        if shift >= 0 {
            numerator = numerator.shift(shift.unsigned_abs() as usize)?;
        } else {
            denominator = denominator.shift(shift.unsigned_abs() as usize)?;
        }
        Self::new(&(numerator), &(denominator), bits >> 63 != 0)
    }

    /// Decode bounded decimal-text evidence, retaining its exact rational value.
    /// # Errors
    /// Rejects coefficients outside the wire domain or scales above nine.
    pub fn decimal(coefficient: i64, scale: u32) -> Result<Self, DecimalError> {
        if coefficient.unsigned_abs() > MAX_INTEGER || scale > 9 {
            return Err(DecimalError::Source);
        }
        Self::new(
            &(Integer::from_u64(coefficient.unsigned_abs())),
            &(Integer::ten_pow(scale)?),
            coefficient < 0,
        )
    }

    /// Construct a bounded native counter ratio.
    /// # Errors
    /// Rejects a zero denominator or counters outside the wire domain.
    pub fn ratio(numerator: u64, denominator: u64) -> Result<Self, DecimalError> {
        if numerator > MAX_INTEGER || denominator > MAX_INTEGER {
            return Err(DecimalError::Source);
        }
        Self::new(
            &(Integer::from_u64(numerator)),
            &(Integer::from_u64(denominator)),
            false,
        )
    }

    /// Exact reciprocal rate N/time before any presentation rounding.
    /// # Errors
    /// Rejects nonpositive time or an invalid positive numerator count.
    pub fn reciprocal(self, count: u64) -> Result<Self, DecimalError> {
        if self.negative || self.numerator.is_zero() || count == 0 || count > MAX_INTEGER {
            return Err(DecimalError::Source);
        }
        Self::new(
            &(self.denominator.mul(Integer::from_u64(count))?),
            &(self.numerator),
            false,
        )
    }

    /// Absolute value; source negative zero has already been normalized.
    #[must_use]
    pub const fn abs(mut self) -> Self {
        self.negative = false;
        self
    }

    /// Whether this source is exactly zero.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.numerator.is_zero()
    }

    /// Compare exact signed values.
    /// # Errors
    /// Rejects cross products exceeding the fixed representation.
    pub fn compare(self, rhs: Self) -> Result<Ordering, DecimalError> {
        if self.negative != rhs.negative {
            return Ok(if self.negative {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let order = self
            .numerator
            .mul(rhs.denominator)?
            .cmp(&rhs.numerator.mul(self.denominator)?);
        Ok(if self.negative {
            order.reverse()
        } else {
            order
        })
    }

    /// Exact nonnegative interval width, including intervals spanning zero.
    /// # Errors
    /// Rejects reversed endpoints or capacity overflow.
    pub fn width(lower: Self, upper: Self) -> Result<Self, DecimalError> {
        if lower.compare(upper)? == Ordering::Greater {
            return Err(DecimalError::Source);
        }
        let left = lower.numerator.mul(upper.denominator)?;
        let right = upper.numerator.mul(lower.denominator)?;
        let numerator = if lower.negative != upper.negative {
            left.add(right)?
        } else if lower.negative {
            left.sub(right)?
        } else {
            right.sub(left)?
        };
        Self::new(
            &(numerator),
            &(lower.denominator.mul(upper.denominator)?),
            false,
        )
    }

    /// Decimal magnitude exponent found only through exact comparisons.
    /// # Errors
    /// Zero has no exponent; arithmetic beyond capacity is refused.
    pub fn exponent(self) -> Result<i32, DecimalError> {
        if self.is_zero() {
            return Err(DecimalError::Source);
        }
        let mut exponent = 0;
        if self.numerator >= self.denominator {
            let mut threshold = self.denominator;
            loop {
                let next = threshold.mul(Integer::from_u64(10))?;
                if self.numerator < next {
                    break;
                }
                threshold = next;
                exponent += 1;
            }
        } else {
            let mut scaled = self.numerator;
            while scaled < self.denominator {
                scaled = scaled.mul(Integer::from_u64(10))?;
                exponent -= 1;
            }
        }
        Ok(exponent)
    }

    /// Shared estimate grid preserving point, standard error and positive width.
    /// # Errors
    /// Rejects reversed endpoints, negative standard error and invalid sources.
    pub fn estimate_grid(
        point: Self,
        error: Self,
        lower: Self,
        upper: Self,
    ) -> Result<i32, DecimalError> {
        if error.negative {
            return Err(DecimalError::Source);
        }
        let width = Self::width(lower, upper)?;
        let mut grid = None;
        for value in [point.abs(), error, width] {
            if !value.is_zero() {
                let power = value.exponent()? - 2;
                grid = Some(grid.map_or(power, |previous: i32| previous.min(power)));
            }
        }
        Ok(grid.unwrap_or(-9))
    }

    /// Round on a decimal quantum, returning sign and unsigned coefficient.
    pub(super) fn coefficient(
        self,
        power: i32,
        role: Rounding,
    ) -> Result<(bool, Integer), DecimalError> {
        if !(-329..=334).contains(&power) {
            return Err(DecimalError::Grid);
        }
        let scale = Integer::ten_pow(power.unsigned_abs())?;
        let (numerator, denominator) = if power < 0 {
            (self.numerator.mul(scale)?, self.denominator)
        } else {
            (self.numerator, self.denominator.mul(scale)?)
        };
        let (mut quotient, remainder) = numerator.div_rem(denominator)?;
        let increment = match role {
            Rounding::Nearest => remainder.add(remainder)? >= denominator,
            Rounding::Lower => self.negative && !remainder.is_zero(),
            Rounding::Upper => !self.negative && !remainder.is_zero(),
        };
        if increment {
            quotient = quotient.add(Integer::from_u64(1))?;
        }
        Ok((self.negative && !quotient.is_zero(), quotient))
    }
}

impl Exact {
    /// Derive CPU utilization exactly from retained counters and allowed CPU count.
    /// # Errors
    /// Rejects unrecorded/zero denominators or counters outside the wire domain.
    pub fn utilization(
        usage_usec: u64,
        elapsed_ns: u64,
        allowed_cpus: u64,
    ) -> Result<Self, DecimalError> {
        if [usage_usec, elapsed_ns, allowed_cpus]
            .into_iter()
            .any(|n| n > MAX_INTEGER)
        {
            return Err(DecimalError::Source);
        }
        Self::new(
            &(Integer::from_u64(usage_usec).mul(Integer::from_u64(1000))?),
            &(Integer::from_u64(elapsed_ns).mul(Integer::from_u64(allowed_cpus))?),
            false,
        )
    }

    /// Retain an exact positive source resolution only when both reduced counts fit the wire.
    pub(crate) fn bounded_ratio(self) -> Result<(u64, u64), DecimalError> {
        if self.negative || self.is_zero() {
            return Err(DecimalError::Source);
        }
        let numerator = self.numerator.to_u64()?;
        let denominator = self.denominator.to_u64()?;
        if numerator > MAX_INTEGER || denominator > MAX_INTEGER {
            return Err(DecimalError::Source);
        }
        Ok((numerator, denominator))
    }

    /// Refine a nominal decimal grid to retain a nonzero source and its resolution.
    /// # Errors
    /// Rejects zero/nonpositive source resolution or unrepresentable grids.
    pub fn telemetry_grid(self, nominal_power: i32, resolution: Self) -> Result<i32, DecimalError> {
        if resolution.negative || resolution.is_zero() {
            return Err(DecimalError::Source);
        }
        let mut grid = nominal_power.min(resolution.exponent()?);
        if !self.is_zero() {
            grid = grid.min(self.exponent()? - 2);
        }
        if !(-326..=309).contains(&grid) {
            return Err(DecimalError::Grid);
        }
        Ok(grid)
    }
}

impl Exact {
    /// Correctly rounded nearest-even binary64 view of an exact rational.
    /// # Errors
    /// Rejects a nonfinite result or checked fixed-capacity overflow.
    pub fn binary64_bits(self) -> Result<u64, DecimalError> {
        if self.is_zero() {
            return Ok(0);
        }
        let mut exponent = i32::try_from(self.numerator.bits())
            .map_err(|_| DecimalError::Capacity)?
            - i32::try_from(self.denominator.bits()).map_err(|_| DecimalError::Capacity)?;
        let below = if exponent >= 0 {
            self.numerator < self.denominator.shift(exponent.unsigned_abs() as usize)?
        } else {
            self.numerator.shift(exponent.unsigned_abs() as usize)? < self.denominator
        };
        if below {
            exponent -= 1;
        }
        let subnormal = exponent < -1022;
        let quantum = if subnormal { -1074 } else { exponent - 52 };
        let (numerator, denominator) = if quantum < 0 {
            (
                self.numerator.shift(quantum.unsigned_abs() as usize)?,
                self.denominator,
            )
        } else {
            (
                self.numerator,
                self.denominator.shift(quantum.unsigned_abs() as usize)?,
            )
        };
        let (quotient, remainder) = numerator.div_rem(denominator)?;
        let mut mantissa = quotient.to_u64()?;
        let comparison = remainder.add(remainder)?.cmp(&denominator);
        let tie_odd = comparison == Ordering::Equal && mantissa % 2 != 0;
        if comparison == Ordering::Greater || tie_odd {
            mantissa = mantissa.checked_add(1).ok_or(DecimalError::Capacity)?;
        }
        let magnitude = if subnormal {
            mantissa
        } else {
            if mantissa == (1_u64 << 53) {
                mantissa >>= 1;
                exponent += 1;
            }
            if exponent > 1023 {
                return Err(DecimalError::Source);
            }
            let encoded = u64::try_from(exponent + 1023).map_err(|_| DecimalError::Source)?;
            (encoded << 52) | (mantissa & ((1_u64 << 52) - 1))
        };
        Ok(magnitude | if self.negative { 1_u64 << 63 } else { 0 })
    }
}

impl Exact {
    /// Decode a canonical fixed-grid summary literal with integer arithmetic only.
    /// # Errors
    /// Rejects alternate spellings, invalid grids and fixed-capacity overflow.
    pub fn from_grid(text: &str, power: i32) -> Result<Self, DecimalError> {
        super::validate_fixed(text, power)?;
        let negative = text.starts_with('-');
        let mut coefficient = Integer::ZERO;
        for digit in text
            .trim_start_matches('-')
            .bytes()
            .filter(|byte| *byte != b'.')
        {
            coefficient = coefficient
                .mul(Integer::from_u64(10))?
                .add(Integer::from_u64(u64::from(digit - b'0')))?;
        }
        let scale = Integer::ten_pow(power.unsigned_abs())?;
        if power < 0 {
            Self::new(&(coefficient), &(scale), negative)
        } else {
            Self::new(
                &(coefficient.mul(scale)?),
                &(Integer::from_u64(1)),
                negative,
            )
        }
    }
}

impl Exact {
    /// Exact central-pair median in seconds from retained integer nanoseconds.
    /// # Errors
    /// Rejects source durations beyond the wire bound; the internal sum may use 54 bits.
    pub fn midpoint_nanos(left: u64, right: u64) -> Result<Self, DecimalError> {
        if left > MAX_INTEGER || right > MAX_INTEGER {
            return Err(DecimalError::Source);
        }
        Self::new(
            &(Integer::from_u64(left).add(Integer::from_u64(right))?),
            &(Integer::from_u64(2_000_000_000)),
            false,
        )
    }
}
