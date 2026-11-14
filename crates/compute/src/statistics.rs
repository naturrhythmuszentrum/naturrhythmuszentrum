// SPDX-License-Identifier: AGPL-3.0-only
//! Owned host estimates with exact source evidence and deterministic projections.

use crate::{
    Binary64,
    decimal::{DecimalError, Exact, Rounding, fixed, significant},
};
use nrz_matrix::sampling;
use serde::{Deserialize, Serialize};

/// One rounded view retains its authoritative source bits only in raw evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawValue {
    /// Original computed binary64 bits, including the sign of source zero.
    pub source_bits: Binary64,
    /// Canonical grid projection, never used as a substitute source.
    pub decimal: String,
}

impl RawValue {
    /// Project a computed time value using its exact bits.
    pub(crate) fn time(value: f64, power: i32, role: Rounding) -> Result<Self, DecimalError> {
        let source_bits = Binary64(value.to_bits());
        let decimal = fixed(Exact::from_bits(source_bits.0)?, power, role)?;
        Ok(Self {
            source_bits,
            decimal,
        })
    }

    /// Validate source/view agreement without a floating-point decimal parse.
    pub(crate) fn validate(&self, power: i32, role: Rounding) -> Result<(), DecimalError> {
        let expected = fixed(Exact::from_bits(self.source_bits.0)?, power, role)?;
        if self.decimal != expected {
            return Err(DecimalError::Source);
        }
        Ok(())
    }
}

/// Fixed-grid time estimate in seconds, with shared grid for all four fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawEstimate {
    /// Decimal quantum in seconds; positive powers denote scaled seconds.
    pub grid_power: i32,
    /// Exact source and nearest point view.
    pub point: RawValue,
    /// Exact source and nearest bootstrap standard error view.
    pub standard_error: RawValue,
    /// Exact source and outward lower endpoint view.
    pub lower: RawValue,
    /// Exact source and outward upper endpoint view.
    pub upper: RawValue,
}

impl RawEstimate {
    /// Own one complete unchanged host estimate.
    pub(crate) fn from_host(value: sampling::Estimate) -> Result<Self, DecimalError> {
        let lower = value.confidence_interval.lower_bound;
        let upper = value.confidence_interval.upper_bound;
        let power = Exact::estimate_grid(
            Exact::from_bits(value.point_estimate.to_bits())?,
            Exact::from_bits(value.standard_error.to_bits())?,
            Exact::from_bits(lower.to_bits())?,
            Exact::from_bits(upper.to_bits())?,
        )?;
        Ok(Self {
            grid_power: power,
            point: RawValue::time(value.point_estimate, power, Rounding::Nearest)?,
            standard_error: RawValue::time(value.standard_error, power, Rounding::Nearest)?,
            lower: RawValue::time(lower, power, Rounding::Lower)?,
            upper: RawValue::time(upper, power, Rounding::Upper)?,
        })
    }

    /// Recompute the grid and all views from the original exact evidence.
    pub(crate) fn validate(&self) -> Result<(), DecimalError> {
        let power = Exact::estimate_grid(
            Exact::from_bits(self.point.source_bits.0)?,
            Exact::from_bits(self.standard_error.source_bits.0)?,
            Exact::from_bits(self.lower.source_bits.0)?,
            Exact::from_bits(self.upper.source_bits.0)?,
        )?;
        if power != self.grid_power {
            return Err(DecimalError::Grid);
        }
        for (value, role) in [
            (&self.point, Rounding::Nearest),
            (&self.standard_error, Rounding::Nearest),
            (&self.lower, Rounding::Lower),
            (&self.upper, Rounding::Upper),
        ] {
            value.validate(power, role)?;
        }
        Ok(())
    }

    /// Project a validated estimate without source bits or live work.
    pub(crate) fn summary(&self) -> Result<DecimalEstimate, DecimalError> {
        self.validate()?;
        Ok(DecimalEstimate {
            grid_power: self.grid_power,
            point: self.point.decimal.clone(),
            standard_error: self.standard_error.decimal.clone(),
            lower: self.lower.decimal.clone(),
            upper: self.upper.decimal.clone(),
        })
    }

    /// Positive source endpoints admit a reciprocal interval independently of its point.
    fn reciprocal_interval(&self, numerator: u64) -> Result<Option<[Exact; 2]>, DecimalError> {
        let lower = Exact::from_bits(self.lower.source_bits.0)?;
        let upper = Exact::from_bits(self.upper.source_bits.0)?;
        let zero = Exact::from_bits(0)?;
        if lower.compare(zero)? != std::cmp::Ordering::Greater
            || upper.compare(zero)? != std::cmp::Ordering::Greater
        {
            return Ok(None);
        }
        Ok(Some([
            upper.reciprocal(numerator)?,
            lower.reciprocal(numerator)?,
        ]))
    }

    /// Exact reciprocal endpoints, never reciprocals of written time values.
    pub(crate) fn rate(&self, numerator: u64) -> Result<RateEstimate, DecimalError> {
        let interval = self.reciprocal_interval(numerator)?;
        Ok(RateEstimate {
            numerator,
            point: significant(
                Exact::from_bits(self.point.source_bits.0)?.reciprocal(numerator)?,
                Rounding::Nearest,
            )?,
            lower: interval
                .map(|values| significant(values[0], Rounding::Lower))
                .transpose()?,
            upper: interval
                .map(|values| significant(values[1], Rounding::Upper))
                .transpose()?,
            interval_unavailable: interval
                .is_none()
                .then(|| "NonpositiveTimeEndpoint".to_owned()),
        })
    }
}

/// Source-free time estimate projected onto its shared exact decimal grid.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecimalEstimate {
    /// Decimal quantum in seconds.
    pub grid_power: i32,
    /// Nearest point estimate.
    pub point: String,
    /// Nearest bootstrap standard error.
    pub standard_error: String,
    /// Outward lower confidence endpoint.
    pub lower: String,
    /// Outward upper confidence endpoint.
    pub upper: String,
}

/// Reciprocal rate without an invented reciprocal standard error.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateEstimate {
    /// Explicit positive integer work numerator.
    pub numerator: u64,
    /// Six-significant-figure nearest rate.
    pub point: String,
    /// Six-significant-figure outward lower endpoint.
    pub lower: Option<String>,
    /// Six-significant-figure outward upper endpoint.
    pub upper: Option<String>,
    /// Nonpositive source interval endpoints have no reciprocal interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_unavailable: Option<String>,
}

/// Complete fixed-shape statistical evidence, with no partial estimates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawAnalysis {
    /// Mean, median, sample standard deviation, scaled MAD and slope, in that order.
    pub estimates: [RawEstimate; 5],
    /// Shared confidence source and six-place rounded view.
    pub confidence: RawValue,
    /// Fence grid, refined from the median grid as necessary.
    pub fence_grid: i32,
    /// Lower outer, lower inner, upper inner and upper outer fences.
    pub fences: [RawValue; 4],
    /// Low severe, low mild, ordinary, high mild and high severe row counts.
    pub outlier_counts: [u64; 5],
    /// Original chronological row classification, omitted from summaries.
    pub outlier_labels: Vec<u8>,
}

impl RawAnalysis {
    /// Convert complete host statistics once, retaining every source bit.
    pub(crate) fn from_host(report: &sampling::AnalysisReport) -> Result<Self, DecimalError> {
        let estimates = report.estimates();
        let estimates = [
            estimates.mean,
            estimates.median,
            estimates.standard_deviation,
            estimates.median_absolute_deviation,
            estimates.slope,
        ]
        .map(RawEstimate::from_host);
        let [mean, median, deviation, mad, slope] = estimates;
        let estimates = [mean?, median?, deviation?, mad?, slope?];
        let fences = report.fences();
        let fences = [
            fences.lower_outer,
            fences.lower_inner,
            fences.upper_inner,
            fences.upper_outer,
        ];
        let mut grid = estimates[1].grid_power;
        for fence in fences {
            let exact = Exact::from_bits(fence.to_bits())?;
            if !exact.is_zero() {
                grid = grid.min(exact.exponent()? - 2);
            }
        }
        let fences = [
            RawValue::time(fences[0], grid, Rounding::Lower)?,
            RawValue::time(fences[1], grid, Rounding::Lower)?,
            RawValue::time(fences[2], grid, Rounding::Upper)?,
            RawValue::time(fences[3], grid, Rounding::Upper)?,
        ];
        let [low_severe, low_mild, ordinary, high_mild, high_severe] = report
            .label_counts()
            .map(|count| u64::try_from(count).map_err(|_| DecimalError::Capacity));
        Ok(Self {
            estimates,
            confidence: RawValue::time(report.settings().confidence(), -6, Rounding::Nearest)?,
            fence_grid: grid,
            fences,
            outlier_counts: [low_severe?, low_mild?, ordinary?, high_mild?, high_severe?],
            outlier_labels: report
                .observations()
                .iter()
                .map(|observation| match observation.label {
                    sampling::TukeyLabel::LowSevere => 0,
                    sampling::TukeyLabel::LowMild => 1,
                    sampling::TukeyLabel::NotAnOutlier => 2,
                    sampling::TukeyLabel::HighMild => 3,
                    sampling::TukeyLabel::HighSevere => 4,
                })
                .collect(),
        })
    }
}

/// Exact reciprocal rate views retain correctly rounded binary64 evidence in raw reports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRate {
    /// Point, lower and upper correctly rounded nearest-even views of exact ratios.
    pub source_bits: [Option<Binary64>; 3],
    /// Direct exact-rational decimal projection, independent of the view bits.
    pub decimal: RateEstimate,
}

impl RawRate {
    /// Derive the raw views and the direct decimal projection from authoritative time bits.
    pub(crate) fn from_time(time: &RawEstimate, numerator: u64) -> Result<Self, DecimalError> {
        let point = Exact::from_bits(time.point.source_bits.0)?.reciprocal(numerator)?;
        let interval = time.reciprocal_interval(numerator)?;
        Ok(Self {
            source_bits: [
                Some(Binary64(point.binary64_bits()?)),
                interval
                    .map(|values| values[0].binary64_bits().map(Binary64))
                    .transpose()?,
                interval
                    .map(|values| values[1].binary64_bits().map(Binary64))
                    .transpose()?,
            ],
            decimal: time.rate(numerator)?,
        })
    }
}
