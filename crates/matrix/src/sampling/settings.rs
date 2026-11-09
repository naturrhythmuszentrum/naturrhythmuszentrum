// SPDX-License-Identifier: AGPL-3.0-only
//! Validated count domains and explicit payload limits.

use super::{AnalysisCaps, AnalysisError, AnalysisSettings};

impl AnalysisSettings {
    /// Validate complete sample and bootstrap counts, confidence, seed and caps.
    ///
    /// Counts are bounded by `u32::MAX`; byte caps may impose tighter limits.
    ///
    /// # Errors
    /// Returns a typed invalid count, confidence or capacity error.
    pub fn new(
        samples: usize,
        resamples: usize,
        confidence: f64,
        seed: u128,
        caps: AnalysisCaps,
    ) -> Result<Self, AnalysisError> {
        let samples = u32::try_from(samples).map_err(|_| AnalysisError::SampleCount)?;
        let resamples = u32::try_from(resamples).map_err(|_| AnalysisError::ResampleCount)?;
        if samples < 10 {
            return Err(AnalysisError::SampleCount);
        }
        if resamples < 1000 {
            return Err(AnalysisError::ResampleCount);
        }
        validate_confidence(confidence)?;
        let settings = Self {
            samples,
            resamples,
            confidence,
            seed,
            caps,
        };
        super::run::validate_capacity(settings)?;
        Ok(settings)
    }

    /// Required completed sample count.
    #[must_use]
    pub const fn samples(self) -> u32 {
        self.samples
    }

    /// Required count for each bootstrap distribution.
    #[must_use]
    pub const fn resamples(self) -> u32 {
        self.resamples
    }

    /// Requested percentile confidence level.
    #[must_use]
    pub const fn confidence(self) -> f64 {
        self.confidence
    }

    /// Recorded generator seed, independent of clocks and process state.
    #[must_use]
    pub const fn seed(self) -> u128 {
        self.seed
    }

    /// Independent report and scratch payload caps.
    #[must_use]
    pub const fn caps(self) -> AnalysisCaps {
        self.caps
    }
}

/// Admit confidence without allowing NaN to bypass comparisons.
pub(super) fn validate_confidence(confidence: f64) -> Result<(), AnalysisError> {
    let positive = confidence > 0.0;
    let below_one = confidence < 1.0;
    if positive && below_one {
        Ok(())
    } else {
        Err(AnalysisError::Confidence)
    }
}

/// Convert exactly represented nonnegative counts, rejecting silent rounding.
#[expect(
    clippy::cast_precision_loss,
    reason = "the explicit 2^53 bound makes the conversion exact"
)]
pub(super) const fn count(value: u64) -> Result<f64, AnalysisError> {
    if value > (1_u64 << 53) {
        return Err(AnalysisError::CountRepresentation);
    }
    Ok(value as f64)
}

/// Validate every floating intermediate at its arithmetic boundary.
pub(super) const fn finite(value: f64) -> Result<f64, AnalysisError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(AnalysisError::Arithmetic)
    }
}

/// Require a usable time; underflow must not create an invented reciprocal.
pub(super) fn positive(value: f64) -> Result<f64, AnalysisError> {
    let value = finite(value)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(AnalysisError::NonPositiveTimeBound)
    }
}
