// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Adapted from Criterion 0.8.2, revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream: `src/stats/mod.rs`; `src/estimate.rs`; `src/analysis/mod.rs`; items: `Distribution::from/confidence_interval`, `build_estimates`, estimates/regression.
//! Modifications: finite public records, borrowed distributions and cancellable percentile ordering.
//! Copyright (c) 2014 Jorge Aparicio; MIT option elected.
//! Attribution: crates/matrix/src/sampling/criterion/NOTICE;
//! upstream license: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

use super::{
    AnalysisError, ConfidenceInterval, Control, Estimate, finite, percentiles, sample::Sample,
};

/// Assemble a point, bootstrap sample standard deviation and percentile bounds.
pub(super) fn assemble(
    point: f64,
    distribution: &mut [f64],
    confidence: f64,
    control: &mut Control<'_>,
) -> Result<Estimate, AnalysisError> {
    super::super::settings::validate_confidence(confidence)?;
    let point_estimate = finite(point)?;
    let standard_error = Sample::new(distribution, control)?.standard_deviation(control)?;
    percentiles::sort(distribution, control)?;
    let lower_bound = percentiles::at(distribution, 50.0 * (1.0 - confidence))?;
    let upper_bound = percentiles::at(distribution, 50.0 * (1.0 + confidence))?;
    Ok(Estimate {
        point_estimate,
        standard_error,
        confidence_interval: ConfidenceInterval {
            confidence_level: confidence,
            lower_bound,
            upper_bound,
        },
    })
}
