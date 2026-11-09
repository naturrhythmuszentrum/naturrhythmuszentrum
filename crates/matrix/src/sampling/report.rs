// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable completed reports, reciprocal rates and scoped batch extrapolation.

use super::settings::{count, finite, positive};
use super::{
    AnalysisError, AnalysisReport, AnalysisSettings, BatchEstimate, ConfidenceInterval, Estimate,
    Estimates, Observation, RateEstimate, Throughput, TukeyFences,
};

impl AnalysisReport {
    /// Requested and achieved settings; success means every row and draw exists.
    #[must_use]
    pub const fn settings(&self) -> AnalysisSettings {
        self.settings
    }

    /// Source revision of the attributed Criterion extraction.
    #[must_use]
    pub const fn source_revision(&self) -> &'static str {
        self.source_revision
    }

    /// Explicit estimator and uniform-index semantics revision.
    #[must_use]
    pub const fn estimator_revision(&self) -> &'static str {
        self.estimator_revision
    }

    /// Every completed row in original order, with diagnostic labels only.
    #[must_use]
    pub fn observations(&self) -> &[Observation] {
        &self.observations
    }

    /// Absolute time statistics and complete bootstrap uncertainty.
    #[must_use]
    pub const fn estimates(&self) -> Estimates {
        self.estimates
    }

    /// Fences computed from the complete original normalized sample.
    #[must_use]
    pub const fn fences(&self) -> TukeyFences {
        self.fences
    }

    /// Counts in low-severe, low-mild, ordinary, high-mild, high-severe order.
    #[must_use]
    pub const fn label_counts(&self) -> [usize; 5] {
        self.label_counts
    }

    /// Explicit element meaning, absent when the host supplied none.
    #[must_use]
    pub const fn elements_per_item(&self) -> Option<u64> {
        self.elements_per_item
    }

    /// Finite reciprocal primary estimates, with reversed confidence endpoints.
    #[must_use]
    pub const fn throughput(&self) -> Throughput {
        self.throughput
    }

    /// Scale primary time uncertainty for a homogeneous serial batch.
    ///
    /// This estimates the mean total under the same warmed operation,
    /// representative population, machine, tracing and sequential conditions.
    /// It is not a future-batch prediction interval, worst-case time or numerical
    /// certificate. Cold preparation, I/O, contention, throttling, changing
    /// caches and different parallelism require separately measured scopes.
    /// Confidence endpoints scale by N, never by its square root.
    ///
    /// # Errors
    /// Rejects counts beyond 2^53 and nonfinite products. Zero items return exact
    /// zero without multiplying time bounds. Only a complete report has this helper.
    pub fn extrapolate_batch(&self, items: u64) -> Result<BatchEstimate, AnalysisError> {
        batch(self.estimates.slope, items)
    }
}

/// Preserve interval confidence while inverting the ordered time bounds.
fn rate(estimate: Estimate, units: f64) -> Result<RateEstimate, AnalysisError> {
    let point = positive(estimate.point_estimate)?;
    let lower = positive(estimate.confidence_interval.lower_bound)?;
    let upper = positive(estimate.confidence_interval.upper_bound)?;
    Ok(RateEstimate {
        point_estimate: positive(units / point)?,
        confidence_interval: ConfidenceInterval {
            confidence_level: estimate.confidence_interval.confidence_level,
            lower_bound: positive(units / upper)?,
            upper_bound: positive(units / lower)?,
        },
    })
}

/// Absence of declared element meaning never invents an element rate.
pub(super) fn throughput(
    estimate: Estimate,
    elements: Option<u64>,
) -> Result<Throughput, AnalysisError> {
    Ok(Throughput {
        items_per_second: rate(estimate, 1.0)?,
        elements_per_second: elements
            .map(|n| count(n).and_then(|n| rate(estimate, n)))
            .transpose()?,
    })
}

/// Batch arithmetic stays separate from statistical confidence construction.
fn batch(estimate: Estimate, items: u64) -> Result<BatchEstimate, AnalysisError> {
    let confidence_level = estimate.confidence_interval.confidence_level;
    if items == 0 {
        return Ok(BatchEstimate {
            items,
            estimated_seconds: 0.0,
            confidence_interval: ConfidenceInterval {
                confidence_level,
                lower_bound: 0.0,
                upper_bound: 0.0,
            },
        });
    }
    let factor = count(items)?;
    Ok(BatchEstimate {
        items,
        estimated_seconds: finite(factor * estimate.point_estimate)?,
        confidence_interval: ConfidenceInterval {
            confidence_level,
            lower_bound: finite(factor * estimate.confidence_interval.lower_bound)?,
            upper_bound: finite(factor * estimate.confidence_interval.upper_bound)?,
        },
    })
}
