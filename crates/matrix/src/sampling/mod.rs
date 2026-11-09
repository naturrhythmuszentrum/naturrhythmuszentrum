// SPDX-License-Identifier: AGPL-3.0-only
//! Cooperative collection and bounded analysis of host wall-time observations.
//!
//! Times are seconds and never enter engine Work, limits or proof evidence.
//! [`analyze`] requires the complete requested row count and bootstrap count.
//! [`sample`] borrows a prepared problem and an isolated host adapter. It owns
//! repetition, deadline admission and analysis, without process-global changes.

mod collect;
mod criterion;
mod memory;
mod report;
mod run;
mod settings;

pub use collect::{
    AdapterBounds, HostOperation, InstantClock, IterationOutcome, IterationRecord, ReceiptStatus,
    Representative, SamplerBounds, SamplingClock, SamplingDeadline, SamplingError,
    SamplingIdentity, SamplingPhase, SamplingReport, SamplingSettings, SamplingStop,
    SchedulePosition, WorkReceipt, sample, sample_with_clock,
};
pub use run::analyze;

/// Criterion source revision from which the private analysis is adapted.
pub const SOURCE_REVISION: &str = "7f0d745532e3c7b2e11bbf9de9b911f91790d3b1";
/// Estimator, safety adaptation and seeded index-selection revision.
pub const ESTIMATOR_REVISION: &str = "criterion-0.8.2-nrz-safe-1";

/// Explicit limits on owned Rust payload capacity, in bytes.
///
/// Report bytes include the report record and its observation vector capacity.
/// Scratch bytes include persistent buffer/workspace records and all vector capacities.
/// Allocator metadata, bounded scalar call frames, caller input and callback storage are excluded.
/// Allocation and deallocation run at checkpoints; their execution bound belongs
/// to the caller's host assumptions. No allocation occurs inside a sort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnalysisCaps {
    /// Maximum owned report payload.
    pub report_bytes: usize,
    /// Maximum temporary analysis payload.
    pub scratch_bytes: usize,
}

/// Validated, explicit analysis configuration; there are no inferred defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalysisSettings {
    /// Exact required number of completed sample rows.
    samples: u32,
    /// Exact number of resamples for each bootstrap.
    resamples: u32,
    /// Confidence strictly between zero and one.
    confidence: f64,
    /// Sampler-owned generator seed.
    seed: u128,
    /// Independent output and temporary storage allowances.
    caps: AnalysisCaps,
}

/// One completed sample: an iteration count and its summed elapsed seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleRow {
    /// Nonzero number of representative items, at most 2^53 for exact conversion.
    pub iterations: u64,
    /// Positive finite elapsed time in seconds.
    pub elapsed_seconds: f64,
}

/// A phase in which the caller can stop ordinary analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisPhase {
    /// Settings, completed row and intermediate validation.
    Validation,
    /// Fallible bounded storage reservation.
    Allocation,
    /// Point statistics and estimate assembly.
    Statistics,
    /// Uniform index selection and bootstrap traversal.
    Resampling,
    /// In-place percentile ordering with bounded comparisons between checks.
    Sorting,
    /// Diagnostic labels, rates and complete report assembly.
    Report,
}

/// Which independent payload allowance was exhausted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityKind {
    /// Returned report and completed observations.
    Report,
    /// Temporary analysis storage.
    Scratch,
}

/// Analysis failures carry no engine refusal or mathematical proof meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AnalysisError {
    /// Sample count must be in `10..=u32::MAX`.
    #[error("sample count must be between ten and u32::MAX")]
    SampleCount,
    /// Bootstrap count must be in `1000..=u32::MAX`.
    #[error("bootstrap count must be between one thousand and u32::MAX")]
    ResampleCount,
    /// Confidence must be finite and strictly inside (0, 1).
    #[error("confidence must be strictly between zero and one")]
    Confidence,
    /// A prefix or an excess of the requested rows cannot yield a report.
    #[error("expected {expected} complete rows, received {actual}")]
    RowCount {
        /// Required schedule size.
        expected: usize,
        /// Supplied completed rows.
        actual: usize,
    },
    /// A measured elapsed time is nonfinite or not positive.
    #[error("row {index} has a nonpositive or nonfinite time")]
    InvalidTime {
        /// Original row position.
        index: usize,
    },
    /// A row count is zero or cannot be represented exactly in binary64.
    #[error("row {index} has an invalid iteration count")]
    InvalidIterations {
        /// Original row position.
        index: usize,
    },
    /// Declared element meaning must have a positive, exactly represented count.
    #[error("invalid elements per item")]
    InvalidElements,
    /// An integer exceeds the exact binary64 count domain, 0..=2^53.
    #[error("count is outside the exact binary64 domain")]
    CountRepresentation,
    /// A finite input produced a nonfinite intermediate or result.
    #[error("nonfinite analysis arithmetic")]
    Arithmetic,
    /// Squared iteration counts summed to zero.
    #[error("zero regression denominator")]
    ZeroRegressionDenominator,
    /// A normalized time or reciprocal time bound is zero or negative.
    #[error("no positive finite time bound")]
    NonPositiveTimeBound,
    /// Checked byte arithmetic or allocation capacity exceeded its allowance.
    #[error("{kind:?} capacity exhausted")]
    Capacity {
        /// Exhausted payload allowance.
        kind: CapacityKind,
    },
    /// The allocator refused a reservation already within its byte allowance.
    #[error("{kind:?} allocation failed")]
    Allocation {
        /// Reservation channel.
        kind: CapacityKind,
    },
    /// The caller stopped ordinary analysis; no partial estimates are returned.
    #[error("analysis cancelled during {phase:?}")]
    Cancelled {
        /// Checkpoint that observed cancellation.
        phase: AnalysisPhase,
    },
    /// A private statistic needs at least two finite observations.
    #[error("invalid finite statistical sample")]
    InvalidSample,
    /// A percentile is outside the finite closed interval [0, 100].
    #[error("invalid percentile")]
    InvalidPercentile,
    /// A checked statistical buffer or resample index is outside its extent.
    #[error("invalid statistical buffer index")]
    InvalidIndex,
    /// Uniform selection exhausted its finite rejection allowance.
    #[error("uniform index rejection allowance exhausted")]
    IndexRejectionLimit,
}

/// Percentile confidence bounds, with their requested confidence level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfidenceInterval {
    /// Requested confidence level; bootstrap coverage is approximate.
    pub confidence_level: f64,
    /// Lower endpoint in the estimate's stated units.
    pub lower_bound: f64,
    /// Upper endpoint in the estimate's stated units.
    pub upper_bound: f64,
}

/// Absolute point statistic and its bootstrap uncertainty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    /// Statistic evaluated on all original observations.
    pub point_estimate: f64,
    /// Bootstrap sample standard deviation, with divisor R-1.
    pub standard_error: f64,
    /// Interpolated percentile interval of the bootstrap distribution.
    pub confidence_interval: ConfidenceInterval,
}

/// Criterion's absolute statistics, all in seconds per representative item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimates {
    /// Arithmetic mean of normalized sample times.
    pub mean: Estimate,
    /// Median of normalized sample times.
    pub median: Estimate,
    /// Sample standard deviation of normalized sample times.
    pub standard_deviation: Estimate,
    /// Median absolute deviation scaled by 1.4826.
    pub median_absolute_deviation: Estimate,
    /// Primary time estimate: through-origin least-squares slope of paired rows.
    pub slope: Estimate,
}

/// Tukey classification of one original normalized time; labels never prune.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TukeyLabel {
    /// Below the lower outer fence.
    LowSevere,
    /// Below the lower inner fence, but not the outer fence.
    LowMild,
    /// Inside or exactly on both inner fences.
    NotAnOutlier,
    /// Above the upper inner fence, but not the outer fence.
    HighMild,
    /// Above the upper outer fence.
    HighSevere,
}

/// Inner and outer Tukey fences in seconds per item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TukeyFences {
    /// First quartile minus three interquartile ranges.
    pub lower_outer: f64,
    /// First quartile minus 1.5 interquartile ranges.
    pub lower_inner: f64,
    /// Third quartile plus 1.5 interquartile ranges.
    pub upper_inner: f64,
    /// Third quartile plus three interquartile ranges.
    pub upper_outer: f64,
}

/// Original row, normalized time and diagnostic label retained together.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    /// Unmodified completed row.
    pub row: SampleRow,
    /// Elapsed seconds divided by the original iteration count.
    pub seconds_per_item: f64,
    /// Classification against the complete sample's fences.
    pub label: TukeyLabel,
}

/// A reciprocal rate with correctly reversed time interval endpoints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateEstimate {
    /// Items or declared elements per second.
    pub point_estimate: f64,
    /// Interval [units / upper time, units / lower time].
    pub confidence_interval: ConfidenceInterval,
}

/// Primary slope throughput with an optional explicit element meaning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throughput {
    /// Representative items per second.
    pub items_per_second: RateEstimate,
    /// Declared elements per second, absent when element meaning is absent.
    pub elements_per_second: Option<RateEstimate>,
}

/// Complete, immutable analysis; a failure never exposes partial estimates.
#[derive(Debug)]
pub struct AnalysisReport {
    /// Pinned source identity recorded when the report is assembled.
    source_revision: &'static str,
    /// Pinned estimator and index-selection semantics.
    estimator_revision: &'static str,
    /// The requested settings, all achieved on a successful return.
    settings: AnalysisSettings,
    /// Every input row in original order, including outliers.
    observations: Vec<Observation>,
    /// Complete absolute estimates.
    estimates: Estimates,
    /// Complete-sample Tukey fences.
    fences: TukeyFences,
    /// Counts in low-severe, low-mild, ordinary, high-mild, high-severe order.
    label_counts: [usize; 5],
    /// Declared element count, when supplied.
    elements_per_item: Option<u64>,
    /// Finite reciprocal slope estimates.
    throughput: Throughput,
}

/// Uncertainty in estimated mean serial time under unchanged sampling scope.
///
/// The scope is the same warmed operation, representative population, machine,
/// tracing and sequential execution conditions. This is neither a prediction
/// interval for a future batch, a worst-case bound nor numerical evidence.
/// Cold preparation, I/O, contention, throttling, changing caches and different
/// parallelism need separately measured scopes. Marginal class intervals cannot
/// be combined into a joint-confidence guarantee.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BatchEstimate {
    /// Nonnegative requested item count.
    pub items: u64,
    /// N times the primary slope, in seconds.
    pub estimated_seconds: f64,
    /// N times both primary time endpoints; uncertainty scales with N.
    pub confidence_interval: ConfidenceInterval,
}

#[cfg(test)]
mod tests;
