// SPDX-License-Identifier: AGPL-3.0-only
//! Owned typed sampler outcomes, preserving the unchanged host enum distinctions.

use crate::recipe::RecipeError;
use serde::{Deserialize, Serialize};

/// Owned wire form of the host `CapacityKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CapacityKind {
    /// Returned report and completed observations.
    Report,
    /// Temporary analysis storage.
    Scratch,
}

impl From<nrz_matrix::sampling::CapacityKind> for CapacityKind {
    fn from(value: nrz_matrix::sampling::CapacityKind) -> Self {
        match value {
            nrz_matrix::sampling::CapacityKind::Report => Self::Report,
            nrz_matrix::sampling::CapacityKind::Scratch => Self::Scratch,
        }
    }
}

/// Owned wire form of the host `AnalysisPhase`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

impl From<nrz_matrix::sampling::AnalysisPhase> for AnalysisPhase {
    fn from(value: nrz_matrix::sampling::AnalysisPhase) -> Self {
        match value {
            nrz_matrix::sampling::AnalysisPhase::Validation => Self::Validation,
            nrz_matrix::sampling::AnalysisPhase::Allocation => Self::Allocation,
            nrz_matrix::sampling::AnalysisPhase::Statistics => Self::Statistics,
            nrz_matrix::sampling::AnalysisPhase::Resampling => Self::Resampling,
            nrz_matrix::sampling::AnalysisPhase::Sorting => Self::Sorting,
            nrz_matrix::sampling::AnalysisPhase::Report => Self::Report,
        }
    }
}

/// Owned wire form of the host `AnalysisError`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum AnalysisError {
    /// Sample count must be in `10..=u32::MAX`.
    SampleCount,
    /// Bootstrap count must be in `1000..=u32::MAX`.
    ResampleCount,
    /// Confidence must be finite and strictly inside (0, 1).
    Confidence,
    /// A prefix or an excess of the requested rows cannot yield a report.
    RowCount {
        /// Required schedule size.
        expected: usize,
        /// Supplied completed rows.
        actual: usize,
    },
    /// A measured elapsed time is nonfinite or not positive.
    InvalidTime {
        /// Original row position.
        index: usize,
    },
    /// A row count is zero or cannot be represented exactly in binary64.
    InvalidIterations {
        /// Original row position.
        index: usize,
    },
    /// Declared element meaning must have a positive, exactly represented count.
    InvalidElements,
    /// An integer exceeds the exact binary64 count domain, 0..=2^53.
    CountRepresentation,
    /// A finite input produced a nonfinite intermediate or result.
    Arithmetic,
    /// Squared iteration counts summed to zero.
    ZeroRegressionDenominator,
    /// A normalized time or reciprocal time bound is zero or negative.
    NonPositiveTimeBound,
    /// Checked byte arithmetic or allocation capacity exceeded its allowance.
    Capacity {
        /// Exhausted payload allowance.
        kind: CapacityKind,
    },
    /// The allocator refused a reservation already within its byte allowance.
    Allocation {
        /// Reservation channel.
        kind: CapacityKind,
    },
    /// The caller stopped ordinary analysis; no partial estimates are returned.
    Cancelled {
        /// Checkpoint that observed cancellation.
        phase: AnalysisPhase,
    },
    /// A private statistic needs at least two finite observations.
    InvalidSample,
    /// A percentile is outside the finite closed interval [0, 100].
    InvalidPercentile,
    /// A checked statistical buffer or resample index is outside its extent.
    InvalidIndex,
    /// Uniform selection exhausted its finite rejection allowance.
    IndexRejectionLimit,
}

impl From<nrz_matrix::sampling::AnalysisError> for AnalysisError {
    fn from(value: nrz_matrix::sampling::AnalysisError) -> Self {
        match value {
            nrz_matrix::sampling::AnalysisError::SampleCount => Self::SampleCount,
            nrz_matrix::sampling::AnalysisError::ResampleCount => Self::ResampleCount,
            nrz_matrix::sampling::AnalysisError::Confidence => Self::Confidence,
            nrz_matrix::sampling::AnalysisError::RowCount { expected, actual } => {
                Self::RowCount { expected, actual }
            }
            nrz_matrix::sampling::AnalysisError::InvalidTime { index } => {
                Self::InvalidTime { index }
            }
            nrz_matrix::sampling::AnalysisError::InvalidIterations { index } => {
                Self::InvalidIterations { index }
            }
            nrz_matrix::sampling::AnalysisError::InvalidElements => Self::InvalidElements,
            nrz_matrix::sampling::AnalysisError::CountRepresentation => Self::CountRepresentation,
            nrz_matrix::sampling::AnalysisError::Arithmetic => Self::Arithmetic,
            nrz_matrix::sampling::AnalysisError::ZeroRegressionDenominator => {
                Self::ZeroRegressionDenominator
            }
            nrz_matrix::sampling::AnalysisError::NonPositiveTimeBound => Self::NonPositiveTimeBound,
            nrz_matrix::sampling::AnalysisError::Capacity { kind } => {
                Self::Capacity { kind: kind.into() }
            }
            nrz_matrix::sampling::AnalysisError::Allocation { kind } => {
                Self::Allocation { kind: kind.into() }
            }
            nrz_matrix::sampling::AnalysisError::Cancelled { phase } => Self::Cancelled {
                phase: phase.into(),
            },
            nrz_matrix::sampling::AnalysisError::InvalidSample => Self::InvalidSample,
            nrz_matrix::sampling::AnalysisError::InvalidPercentile => Self::InvalidPercentile,
            nrz_matrix::sampling::AnalysisError::InvalidIndex => Self::InvalidIndex,
            nrz_matrix::sampling::AnalysisError::IndexRejectionLimit => Self::IndexRejectionLimit,
        }
    }
}

/// Owned wire form of the host `SamplingPhase`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SamplingPhase {
    /// Fixed policy admission before callbacks.
    Validation,
    /// Fallible reservation, before any operation is admitted.
    Allocation,
    /// Fresh context or lawful reset.
    Reset,
    /// Entry Work snapshot.
    Snapshot,
    /// Timer start and invocation admission.
    Operation,
    /// Mandatory exit snapshot and receipt preservation.
    Receipt,
    /// Mandatory output disposal.
    Disposal,
    /// Mandatory private-state cleanup.
    Cleanup,
    /// Checked warm-up/schedule bookkeeping.
    Schedule,
    /// An existing analysis checkpoint.
    Analysis(AnalysisPhase),
    /// Complete bounded return-report construction.
    Report,
}

impl From<nrz_matrix::sampling::SamplingPhase> for SamplingPhase {
    fn from(value: nrz_matrix::sampling::SamplingPhase) -> Self {
        match value {
            nrz_matrix::sampling::SamplingPhase::Validation => Self::Validation,
            nrz_matrix::sampling::SamplingPhase::Allocation => Self::Allocation,
            nrz_matrix::sampling::SamplingPhase::Reset => Self::Reset,
            nrz_matrix::sampling::SamplingPhase::Snapshot => Self::Snapshot,
            nrz_matrix::sampling::SamplingPhase::Operation => Self::Operation,
            nrz_matrix::sampling::SamplingPhase::Receipt => Self::Receipt,
            nrz_matrix::sampling::SamplingPhase::Disposal => Self::Disposal,
            nrz_matrix::sampling::SamplingPhase::Cleanup => Self::Cleanup,
            nrz_matrix::sampling::SamplingPhase::Schedule => Self::Schedule,
            nrz_matrix::sampling::SamplingPhase::Analysis(a) => Self::Analysis(a.into()),
            nrz_matrix::sampling::SamplingPhase::Report => Self::Report,
        }
    }
}

/// Owned wire form of the host `SamplingError`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SamplingError {
    /// Durations, count caps or identity are invalid.
    InvalidSettings,
    /// A complete linear schedule cannot fit the checked iteration caps.
    IterationCap,
    /// The host cannot attest isolation, receipts and cooperative execution bounds.
    Unsupported,
    /// Aggregate duration/count arithmetic overflowed.
    Arithmetic,
    /// Aggregate report/scratch payload cannot fit its cap.
    Capacity,
    /// A fallible reservation was refused.
    Allocation,
}

impl From<nrz_matrix::sampling::SamplingError> for SamplingError {
    fn from(value: nrz_matrix::sampling::SamplingError) -> Self {
        match value {
            nrz_matrix::sampling::SamplingError::InvalidSettings => Self::InvalidSettings,
            nrz_matrix::sampling::SamplingError::IterationCap => Self::IterationCap,
            nrz_matrix::sampling::SamplingError::Unsupported => Self::Unsupported,
            nrz_matrix::sampling::SamplingError::Arithmetic => Self::Arithmetic,
            nrz_matrix::sampling::SamplingError::Capacity => Self::Capacity,
            nrz_matrix::sampling::SamplingError::Allocation => Self::Allocation,
        }
    }
}

/// Owned wire form of the host `SamplingStop`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SamplingStop {
    /// Sampling has not been entered.
    NotStarted,
    /// Entire schedule and every requested bootstrap completed before expiry.
    Complete,
    /// Admission ended at this checkpoint; mandatory work still finished.
    Deadline(SamplingPhase),
    /// Collection admission or schedule validation failed.
    Rejected(SamplingError),
    /// A host callback failed; the typed error and phase are retained.
    HostError(SamplingPhase, RecipeError),
    /// An operation receipt lacked complete Work coverage.
    IncompleteReceipt,
    /// An invoked operation had zero measured duration.
    ClockResolution,
    /// The injected clock violated monotonicity.
    ClockReversed,
    /// Complete rows could not yield a complete analysis.
    Analysis(AnalysisError),
}

impl From<nrz_matrix::sampling::SamplingStop<RecipeError>> for SamplingStop {
    fn from(value: nrz_matrix::sampling::SamplingStop<RecipeError>) -> Self {
        match value {
            nrz_matrix::sampling::SamplingStop::Complete => Self::Complete,
            nrz_matrix::sampling::SamplingStop::Deadline(a) => Self::Deadline(a.into()),
            nrz_matrix::sampling::SamplingStop::Rejected(a) => Self::Rejected(a.into()),
            nrz_matrix::sampling::SamplingStop::HostError(a, b) => Self::HostError(a.into(), b),
            nrz_matrix::sampling::SamplingStop::IncompleteReceipt => Self::IncompleteReceipt,
            nrz_matrix::sampling::SamplingStop::ClockResolution => Self::ClockResolution,
            nrz_matrix::sampling::SamplingStop::ClockReversed => Self::ClockReversed,
            nrz_matrix::sampling::SamplingStop::Analysis(a) => Self::Analysis(a.into()),
        }
    }
}

/// Owned wire form of the host `IterationOutcome`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum IterationOutcome {
    /// Reset failed before any operation invocation.
    ResetFailed(RecipeError),
    /// The operation was not invoked, including expiry before reset.
    /// A cleanup result indicates that an owned context was created.
    NotInvoked,
    /// The operation returned its ordinary success or typed error.
    Returned(Result<(), RecipeError>),
}

impl From<nrz_matrix::sampling::IterationOutcome<RecipeError>> for IterationOutcome {
    fn from(value: nrz_matrix::sampling::IterationOutcome<RecipeError>) -> Self {
        match value {
            nrz_matrix::sampling::IterationOutcome::ResetFailed(a) => Self::ResetFailed(a),
            nrz_matrix::sampling::IterationOutcome::NotInvoked => Self::NotInvoked,
            nrz_matrix::sampling::IterationOutcome::Returned(a) => Self::Returned(a),
        }
    }
}
