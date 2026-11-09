// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable requested/achieved evidence and complete-only extrapolation.

use super::super::{AnalysisError, AnalysisReport, BatchEstimate, SampleRow};
use super::{
    AdapterBounds, IterationRecord, SamplingIdentity, SamplingReport, SamplingSettings,
    SamplingStop,
};
use std::time::Duration;

impl<W, E> SamplingReport<'_, W, E> {
    /// All requested settings, including confidence, seed and byte/count caps.
    #[must_use]
    pub const fn settings(&self) -> SamplingSettings {
        self.settings
    }
    /// Identity, shape, revision, measured scope and trace configuration.
    #[must_use]
    pub const fn identity(&self) -> SamplingIdentity<'_> {
        self.identity
    }
    /// Explicit element meaning, retained even when collection stops early.
    #[must_use]
    pub const fn elements_per_item(&self) -> Option<u64> {
        self.elements
    }
    /// Every attempt, including warm-up, operation failures and unfinished rows.
    #[must_use]
    pub fn iterations(&self) -> &[IterationRecord<W, E>] {
        &self.iterations
    }
    /// Complete measurement rows only; a partial row remains in iteration evidence.
    #[must_use]
    pub fn rows(&self) -> &[SampleRow] {
        &self.rows
    }
    /// Achieved linear step d, absent before successful warm-up/schedule admission.
    #[must_use]
    pub const fn linear_step(&self) -> Option<u64> {
        self.step
    }
    /// Actual warm-up invocation count, including the final failed attempt if any.
    #[must_use]
    pub const fn warm_up_iterations(&self) -> u64 {
        self.warm_up_iterations
    }
    /// Summed timed warm-up, excluding reset, snapshots and disposal.
    #[must_use]
    pub const fn warm_up_elapsed(&self) -> Duration {
        self.warm_up_elapsed
    }
    /// Summed time of all operations, including failed attempts and partial rows.
    #[must_use]
    pub const fn measured_elapsed(&self) -> Duration {
        self.measured
    }
    /// Total elapsed through cleanup and bounded return-report construction.
    #[must_use]
    pub const fn total_elapsed(&self) -> Duration {
        self.total
    }
    /// Observed excess over D; this does not independently measure descheduling.
    #[must_use]
    pub const fn overshoot(&self) -> Duration {
        self.total.saturating_sub(self.settings.wall_budget)
    }
    /// The adapter's separate declarations, including every sequential tail component.
    #[must_use]
    pub const fn adapter_bounds(&self) -> AdapterBounds {
        self.adapter_bounds
    }
    /// Composed uninterrupted bound U under the declared host execution assumptions.
    #[must_use]
    pub const fn uninterrupted_bound(&self) -> Duration {
        self.uninterrupted
    }
    /// Composed entire finalization tail C, not a maximum cleanup component.
    #[must_use]
    pub const fn finalization_bound(&self) -> Duration {
        self.finalization
    }
    /// Stop/completeness status; receipts also retain individual callback failures.
    #[must_use]
    pub const fn stop(&self) -> &SamplingStop<E> {
        &self.stop
    }
    /// Analysis only after the entire linear schedule and requested bootstraps finish.
    #[must_use]
    pub const fn analysis(&self) -> Option<&AnalysisReport> {
        self.analysis.as_ref()
    }
    /// Requested bootstrap count achieved only when a complete analysis is present.
    #[must_use]
    pub const fn achieved_resamples(&self) -> u32 {
        if self.analysis.is_some() {
            self.settings.analysis.resamples()
        } else {
            0
        }
    }
    /// Statistical source revision, also available on partial collection evidence.
    #[must_use]
    pub const fn source_revision(&self) -> &'static str {
        super::super::SOURCE_REVISION
    }
    /// Unchanged estimator/index-selection revision.
    #[must_use]
    pub const fn estimator_revision(&self) -> &'static str {
        super::super::ESTIMATOR_REVISION
    }
    /// Estimate a homogeneous serial batch only from complete collection evidence.
    /// `None` means partial collection; the scope/uncertainty is that of `AnalysisReport`.
    /// # Errors
    /// Returns the analysis helper's count or finite-arithmetic error.
    pub fn extrapolate_batch(&self, items: u64) -> Result<Option<BatchEstimate>, AnalysisError> {
        self.analysis
            .as_ref()
            .map(|report| report.extrapolate_batch(items))
            .transpose()
    }
}
