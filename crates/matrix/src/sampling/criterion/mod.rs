// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Adapted from Criterion 0.8.2, revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream: `src/analysis/mod.rs`; `src/estimate.rs`; `src/stats/mod.rs`; items: common normalization/classification, estimates, regression and concrete distribution assembly.
//! Modifications: bounded private analysis with explicit settings, safe views and no harness side effects.
//! Copyright (c) 2014 Jorge Aparicio; MIT option elected.
//! Attribution: crates/matrix/src/sampling/criterion/NOTICE;
//! upstream license: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

mod bootstrap;
mod estimate;
mod percentiles;
mod random;
mod regression;
mod sample;
mod tukey;

use super::{
    AnalysisError, AnalysisPhase, AnalysisSettings, ConfidenceInterval, Estimate, Estimates,
    SampleRow, TukeyFences, TukeyLabel,
};
use super::{
    memory::{Control, Memory, push},
    settings::{count, finite, positive},
};
use bootstrap::Distributions;
use random::SeededIndices;
use regression::Data;
use sample::Sample;

/// Scratch records remain live together, with no bootstrap-time allocation.
#[derive(Debug)]
struct Workspace {
    /// One univariate resample, overwritten for each draw.
    stage: Vec<f64>,
    /// Reused ordering and absolute-deviation workspace.
    sorted: Vec<f64>,
    /// Exactly the requested number of draws in each distribution.
    distributions: Distributions,
    /// One explicit seed's private generator state.
    indices: SeededIndices,
}

/// Fixed persistent workspace record bytes, excluding bounded scalar call frames.
pub(super) const fn fixed_bytes() -> usize {
    size_of::<Workspace>()
}

/// Analyze all supplied data without collecting or timing an operation.
pub(super) fn analyze(
    normalized: &[f64],
    rows: &[SampleRow],
    settings: AnalysisSettings,
    memory: &mut Memory,
    control: &mut Control<'_>,
) -> Result<(Estimates, TukeyFences), AnalysisError> {
    let length = normalized.len();
    let count = usize::try_from(settings.resamples()).map_err(|_| AnalysisError::ResampleCount)?;
    let mut workspace = Workspace {
        stage: memory.reserve(length, control)?,
        sorted: memory.reserve(length, control)?,
        distributions: Distributions::new(count, memory, control)?,
        indices: SeededIndices::new(settings.seed()),
    };
    let sample = Sample::new(normalized, control)?;
    let [mean, deviation, median, mad] = sample.statistics(&mut workspace.sorted, control)?;
    percentiles::copy_sort(normalized, &mut workspace.sorted, control)?;
    let fences = tukey::fences(&workspace.sorted)?;
    let data = Data::new(rows, control)?;
    let slope = data.slope(control)?;
    workspace.distributions.univariate(
        normalized,
        count,
        (&mut workspace.stage, &mut workspace.sorted),
        &mut workspace.indices,
        control,
    )?;
    workspace
        .distributions
        .paired(data, count, &mut workspace.indices, control)?;
    let distributions = &mut workspace.distributions;
    let confidence = settings.confidence();
    let estimates = Estimates {
        mean: estimate::assemble(mean, &mut distributions.mean, confidence, control)?,
        median: estimate::assemble(median, &mut distributions.median, confidence, control)?,
        standard_deviation: estimate::assemble(
            deviation,
            &mut distributions.deviation,
            confidence,
            control,
        )?,
        median_absolute_deviation: estimate::assemble(
            mad,
            &mut distributions.mad,
            confidence,
            control,
        )?,
        slope: estimate::assemble(slope, &mut distributions.slope, confidence, control)?,
    };
    Ok((estimates, fences))
}

/// Preserve Criterion's diagnostic-only classification for report construction.
pub(super) const fn label(value: f64, fences: TukeyFences) -> TukeyLabel {
    tukey::label(value, fences)
}

#[cfg(test)]
mod tests;
