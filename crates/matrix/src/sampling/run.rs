// SPDX-License-Identifier: AGPL-3.0-only
//! Complete-row admission, payload accounting and analysis orchestration.

use super::{
    AnalysisCaps, AnalysisError, AnalysisPhase, AnalysisReport, AnalysisSettings, CapacityKind,
    Observation, SampleRow, TukeyLabel,
};
use super::{
    criterion,
    memory::{Control, Memory, push},
    report::throughput,
    settings::{count, positive},
};

/// Analyze a complete schedule with deterministic statistics on fixed rows.
///
/// `cancelled` returns true to stop analysis. It is called before and after
/// allocation, for each traversed row or resample draw, at every sorting heap
/// level, and for each RNG rejection attempt. Rejection is capped at 128 draws
/// per index. All other loops are bounded by validated sample/bootstrap counts.
/// No partial report escapes cancellation or failure. Callers own callback
/// execution bounds and the allocation/deallocation scheduling assumptions.
///
/// Successful settings are both requested and achieved settings. Seed replay
/// reproduces statistics on fixed rows, never the observed wall times. Reports
/// assume representative, sufficiently stable data; a narrow interval does not
/// establish those assumptions. No outlier is removed.
///
/// # Errors
/// Returns typed settings, row, arithmetic, capacity, allocation or cancellation
/// errors. An absent element count gives no element-rate claim; a declared count
/// must be positive and exactly representable in binary64.
pub fn analyze(
    rows: &[SampleRow],
    settings: AnalysisSettings,
    elements_per_item: Option<u64>,
    mut cancelled: impl FnMut(AnalysisPhase) -> bool,
) -> Result<AnalysisReport, AnalysisError> {
    let mut control = Control::new(&mut cancelled);
    control.check(AnalysisPhase::Validation)?;
    validate_capacity(settings)?;
    let expected = usize::try_from(settings.samples()).map_err(|_| AnalysisError::SampleCount)?;
    if rows.len() != expected {
        return Err(AnalysisError::RowCount {
            expected,
            actual: rows.len(),
        });
    }
    if let Some(elements) = elements_per_item
        && (elements == 0 || count(elements).is_err())
    {
        return Err(AnalysisError::InvalidElements);
    }
    let caps = settings.caps();
    let mut report_memory = Memory::new(
        CapacityKind::Report,
        caps.report_bytes,
        size_of::<AnalysisReport>(),
    )?;
    let mut observations = report_memory.reserve(expected, &mut control)?;
    let mut scratch = Memory::new(
        CapacityKind::Scratch,
        caps.scratch_bytes,
        scratch_fixed_bytes(),
    )?;
    let mut normalized = scratch.reserve(expected, &mut control)?;
    normalize(rows, &mut normalized, &mut control)?;
    let (estimates, fences) =
        criterion::analyze(&normalized, rows, settings, &mut scratch, &mut control)?;
    let mut label_counts = [0_usize; 5];
    for (&row, &seconds_per_item) in rows.iter().zip(&normalized) {
        control.check(AnalysisPhase::Report)?;
        let label = criterion::label(seconds_per_item, fences);
        let bucket = match label {
            TukeyLabel::LowSevere => 0,
            TukeyLabel::LowMild => 1,
            TukeyLabel::NotAnOutlier => 2,
            TukeyLabel::HighMild => 3,
            TukeyLabel::HighSevere => 4,
        };
        let counter = label_counts
            .get_mut(bucket)
            .ok_or(AnalysisError::InvalidIndex)?;
        *counter = counter.checked_add(1).ok_or(AnalysisError::Arithmetic)?;
        push(
            &mut observations,
            Observation {
                row,
                seconds_per_item,
                label,
            },
        )?;
    }
    control.check(AnalysisPhase::Report)?;
    let throughput = throughput(estimates.slope, elements_per_item)?;
    control.check(AnalysisPhase::Report)?;
    Ok(AnalysisReport {
        source_revision: super::SOURCE_REVISION,
        estimator_revision: super::ESTIMATOR_REVISION,
        settings,
        observations,
        estimates,
        fences,
        label_counts,
        elements_per_item,
        throughput,
    })
}

/// Check every original time/count before normalizing it into admitted storage.
fn normalize(
    rows: &[SampleRow],
    normalized: &mut Vec<f64>,
    control: &mut Control<'_>,
) -> Result<(), AnalysisError> {
    for (index, row) in rows.iter().enumerate() {
        control.check(AnalysisPhase::Validation)?;
        let valid_time = row.elapsed_seconds > 0.0;
        let finite_time = row.elapsed_seconds.is_finite();
        if !(valid_time && finite_time) {
            return Err(AnalysisError::InvalidTime { index });
        }
        if row.iterations == 0 {
            return Err(AnalysisError::InvalidIterations { index });
        }
        let iterations =
            count(row.iterations).map_err(|_| AnalysisError::InvalidIterations { index })?;
        push(normalized, positive(row.elapsed_seconds / iterations)?)?;
    }
    Ok(())
}

/// Persistent vector/workspace records; fixed scalar call frames are excluded.
const fn scratch_fixed_bytes() -> usize {
    criterion::fixed_bytes() + size_of::<Vec<f64>>()
}

/// Requested complete payload sizes, with checked products before allocation.
pub(super) fn required_caps(samples: u32, resamples: u32) -> Result<AnalysisCaps, AnalysisError> {
    let samples = usize::try_from(samples).map_err(|_| AnalysisError::SampleCount)?;
    let resamples = usize::try_from(resamples).map_err(|_| AnalysisError::ResampleCount)?;
    let report_bytes = samples
        .checked_mul(size_of::<Observation>())
        .and_then(|n| n.checked_add(size_of::<AnalysisReport>()))
        .ok_or(AnalysisError::Capacity {
            kind: CapacityKind::Report,
        })?;
    let scratch_bytes = samples
        .checked_mul(3)
        .and_then(|n| resamples.checked_mul(5).and_then(|r| n.checked_add(r)))
        .and_then(|n| n.checked_mul(size_of::<f64>()))
        .and_then(|n| n.checked_add(scratch_fixed_bytes()))
        .ok_or(AnalysisError::Capacity {
            kind: CapacityKind::Scratch,
        })?;
    if scratch_bytes > isize::MAX as usize {
        return Err(AnalysisError::Capacity {
            kind: CapacityKind::Scratch,
        });
    }
    if report_bytes > isize::MAX as usize {
        return Err(AnalysisError::Capacity {
            kind: CapacityKind::Report,
        });
    }
    Ok(AnalysisCaps {
        report_bytes,
        scratch_bytes,
    })
}

/// Enforce all finite count/storage caps before traversing rows or allocating.
pub(super) fn validate_capacity(settings: AnalysisSettings) -> Result<(), AnalysisError> {
    let required = required_caps(settings.samples(), settings.resamples())?;
    let supplied = settings.caps();
    if supplied.report_bytes < required.report_bytes {
        return Err(AnalysisError::Capacity {
            kind: CapacityKind::Report,
        });
    }
    if supplied.scratch_bytes < required.scratch_bytes {
        return Err(AnalysisError::Capacity {
            kind: CapacityKind::Scratch,
        });
    }
    Ok(())
}
