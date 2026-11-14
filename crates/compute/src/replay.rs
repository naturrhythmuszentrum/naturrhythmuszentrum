// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit raw reanalysis; summary decoding never executes this path.

use crate::{RunReport, report::CaseOutcome};
use nrz_matrix::sampling::{self, AnalysisCaps, AnalysisPhase, AnalysisSettings, SampleRow};

/// Raw replay refusal cannot be confused with summary-only verification.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    /// Structural evidence or exact source/view validation failed.
    #[error(transparent)]
    Report(#[from] crate::codec::CodecError),
    /// The named estimator implementation is not the implementation being replayed.
    #[error("raw estimator revision differs from the available implementation")]
    Revision,
    /// Row reconstruction exceeded its independent scratch allowance.
    #[error("raw row reconstruction capacity")]
    Capacity,
    /// Host analysis refused settings, capacity or cancellation.
    #[error(transparent)]
    Analysis(#[from] sampling::AnalysisError),
    /// Recomputed source statistics disagree with the recorded evidence.
    #[error("raw statistical reconstruction mismatch")]
    Mismatch,
    /// Exact statistical projection failed.
    #[error(transparent)]
    Decimal(#[from] crate::decimal::DecimalError),
}

/// Reconstruct every complete case from recorded rows, exact confidence and bootstrap seed.
///
/// This is an explicit bounded reanalysis operation. It takes no clock readings and
/// does not collect measurements. Partial cases have no complete analysis to replay.
/// The caller controls cancellation at the host analyzer's ordinary checkpoints.
/// # Errors
/// Refuses structural corruption, unavailable revisions, capacity, cancellation or disagreement.
pub fn verify_raw_analysis(
    raw: &RunReport,
    mut cancelled: impl FnMut(AnalysisPhase) -> bool,
) -> Result<(), ReplayError> {
    crate::codec::validate_report(raw)?;
    let identity = &raw.payload.identity;
    if identity.source_revision != sampling::SOURCE_REVISION
        || identity.estimator_revision != sampling::ESTIMATOR_REVISION
    {
        return Err(ReplayError::Revision);
    }
    let policy = raw.payload.request.sampling;
    for record in &raw.payload.cases {
        let CaseOutcome::Complete(measurement) = &record.outcome else {
            continue;
        };
        let mut budget = crate::memory::Budget::new(policy.analysis_scratch_bytes);
        let mut rows = budget
            .reserve(measurement.observations.rows.len())
            .map_err(|_| ReplayError::Capacity)?;
        for row in &measurement.observations.rows {
            rows.push(SampleRow {
                iterations: row.iterations,
                elapsed_seconds: f64::from_bits(row.seconds.source_bits.0),
            });
        }
        let settings = AnalysisSettings::new(
            policy.samples as usize,
            policy.resamples as usize,
            f64::from_bits(policy.confidence.0),
            record.seed.0,
            AnalysisCaps {
                report_bytes: usize::try_from(policy.analysis_report_bytes)
                    .map_err(|_| ReplayError::Capacity)?,
                scratch_bytes: usize::try_from(budget.remaining())
                    .map_err(|_| ReplayError::Capacity)?,
            },
        )?;
        let analysis = sampling::analyze(
            &rows,
            settings,
            Some(measurement.numerators.elements),
            &mut cancelled,
        )?;
        if crate::statistics::RawAnalysis::from_host(&analysis)? != measurement.analysis {
            return Err(ReplayError::Mismatch);
        }
    }
    Ok(())
}
