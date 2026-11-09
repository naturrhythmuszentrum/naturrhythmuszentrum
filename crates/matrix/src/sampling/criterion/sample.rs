// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Adapted from Criterion 0.8.2, revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream: `src/stats/univariate/sample.rs`; `src/stats/mod.rs`; items: `Sample::new`, sum, mean, var, `std_dev`, percentiles, `median_abs_dev`.
//! Modifications: validated f64 borrowed slices, checked arithmetic, reusable scratch and cancellation.
//! Copyright (c) 2014 Jorge Aparicio; MIT option elected.
//! Attribution: crates/matrix/src/sampling/criterion/NOTICE;
//! upstream license: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

use super::{AnalysisError, AnalysisPhase, Control, finite, percentiles};

/// At least two finite observations with an exactly represented length.
#[derive(Clone, Copy, Debug)]
pub(super) struct Sample<'a> {
    /// Borrowed safe data; no representation conversion.
    values: &'a [f64],
    /// Validated count, representable without rounding.
    length: u32,
}

impl<'a> Sample<'a> {
    /// Validate all entries before any statistic consumes the view.
    pub(super) fn new(values: &'a [f64], control: &mut Control<'_>) -> Result<Self, AnalysisError> {
        let length = u32::try_from(values.len()).map_err(|_| AnalysisError::InvalidSample)?;
        if length < 2 {
            return Err(AnalysisError::InvalidSample);
        }
        for &value in values {
            control.check(AnalysisPhase::Validation)?;
            if !value.is_finite() {
                return Err(AnalysisError::InvalidSample);
            }
        }
        Ok(Self { values, length })
    }

    /// Sequential upstream summation with finite intermediates.
    pub(super) fn mean(self, control: &mut Control<'_>) -> Result<f64, AnalysisError> {
        let mut sum = 0.0;
        for &value in self.values {
            control.check(AnalysisPhase::Statistics)?;
            sum = finite(sum + value)?;
        }
        finite(sum / f64::from(self.length))
    }

    /// Sample variance with divisor n-1 and Criterion's original arithmetic order.
    pub(super) fn variance(
        self,
        mean: f64,
        control: &mut Control<'_>,
    ) -> Result<f64, AnalysisError> {
        let mut sum = 0.0;
        for &value in self.values {
            control.check(AnalysisPhase::Statistics)?;
            let difference = finite(value - mean)?;
            sum = finite(sum + finite(difference.powi(2))?)?;
        }
        finite(sum / f64::from(self.length - 1))
    }

    /// Standard deviation is the square root of the sample variance.
    pub(super) fn standard_deviation(
        self,
        control: &mut Control<'_>,
    ) -> Result<f64, AnalysisError> {
        let mean = self.mean(control)?;
        finite(self.variance(mean, control)?.sqrt())
    }

    /// Mean, standard deviation, median and scaled MAD on one resample.
    pub(super) fn statistics(
        self,
        sorted: &mut Vec<f64>,
        control: &mut Control<'_>,
    ) -> Result<[f64; 4], AnalysisError> {
        let mean = self.mean(control)?;
        let deviation = finite(self.variance(mean, control)?.sqrt())?;
        percentiles::copy_sort(self.values, sorted, control)?;
        let median = percentiles::at(sorted, 50.0)?;
        sorted.clear();
        for &value in self.values {
            control.check(AnalysisPhase::Statistics)?;
            super::push(sorted, finite(value - median)?.abs())?;
        }
        percentiles::sort(sorted, control)?;
        let mad = finite(percentiles::at(sorted, 50.0)? * 1.4826)?;
        Ok([mean, deviation, median, mad])
    }
}
