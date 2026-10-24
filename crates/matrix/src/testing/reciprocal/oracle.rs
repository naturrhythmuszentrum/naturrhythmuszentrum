// SPDX-License-Identifier: AGPL-3.0-only
//! Test-only exact fractions decode binary inputs independently of scalar enclosure arithmetic.

use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::readings::algebra::Rational as Exact;
use std::cmp::Ordering;

/// Arbitrary-precision oracle avoids fixed-width cross-product overflow for small physical inputs.
pub(crate) struct Rational(Exact);

impl Rational {
    /// Independently specified integer fraction.
    pub(crate) fn new(top: i64, bottom: u64) -> TestResult<Self> {
        Ok(Self(Exact::from_ratio(top, bottom, &mut ledger()?)?))
    }

    /// Decode every finite binary64 exactly; widened binary32 values retain their exact value.
    pub(crate) fn binary(value: f64) -> TestResult<Self> {
        if !value.is_finite() {
            return Err("nonfinite oracle input".into());
        }
        let bits = value.to_bits();
        let field = (bits >> 52) & 2047;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (mantissa, exponent) = if field == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1_u64 << 52), i64::try_from(field)? - 1075)
        };
        if mantissa == 0 {
            return Ok(Self(Exact::zero()));
        }
        let trailing = mantissa.trailing_zeros();
        let significant = mantissa >> trailing;
        let exponent = exponent + i64::from(trailing);
        let sign = if bits >> 63 == 0 { 1 } else { -1 };
        let numerator = Exact::from_ratio(sign * i64::try_from(significant)?, 1, &mut ledger()?)?;
        let scale = Exact::from_ratio(2, 1, &mut ledger()?)?.pow(exponent, &mut ledger()?)?;
        Ok(Self(numerator.mul(&scale, &mut ledger()?)?))
    }

    /// Exact addition of independently decoded endpoints.
    pub(crate) fn add(&self, other: &Self) -> TestResult<Self> {
        Ok(Self(self.0.add(&other.0, &mut ledger()?)?))
    }

    /// Exact coefficient multiplication.
    pub(crate) fn multiply(&self, other: &Self) -> TestResult<Self> {
        Ok(Self(self.0.mul(&other.0, &mut ledger()?)?))
    }

    /// Exact physical reciprocal ratio, independent of the executed coefficient schedule.
    pub(crate) fn divide(&self, other: &Self) -> TestResult<Self> {
        Ok(Self(self.0.div(&other.0, &mut ledger()?)?))
    }

    /// Exact ordering supports squared-norm comparisons without fixed-width cross products.
    pub(crate) fn at_most(&self, other: &Self) -> TestResult<bool> {
        Ok(self.0.compare(&other.0, &mut ledger()?)? != Ordering::Greater)
    }

    /// Exact endpoint containment without a rounded midpoint or radius.
    pub(crate) fn enclosed(&self, lower: f64, upper: f64) -> TestResult<bool> {
        let lower = Self::binary(lower)?;
        let upper = Self::binary(upper)?;
        let above = self.0.compare(&lower.0, &mut ledger()?)? != Ordering::Less;
        let below = self.0.compare(&upper.0, &mut ledger()?)? != Ordering::Greater;
        Ok(above && below)
    }

    /// Compare against exact binary center plus/minus radius without rounding either endpoint.
    pub(crate) fn covered(&self, center: f64, radius: f64) -> TestResult<bool> {
        if radius < 0.0 {
            return Ok(false);
        }
        let center = Self::binary(center)?;
        let radius = Self::binary(radius)?;
        let lower = center.0.sub(&radius.0, &mut ledger()?)?;
        let upper = center.0.add(&radius.0, &mut ledger()?)?;
        let above = self.0.compare(&lower, &mut ledger()?)? != Ordering::Less;
        let below = self.0.compare(&upper, &mut ledger()?)? != Ordering::Greater;
        Ok(above && below)
    }
}
