// SPDX-License-Identifier: AGPL-3.0-only
//! Owned observation conversion, preserving incomplete rows and separate failures.

use crate::report::{
    AttemptException, CaseOutcome, Measurement, Observations, PartialMeasurement, RawRow, Reason,
};
use crate::{
    adapter::NoEngineWork,
    decimal::Rounding,
    recipe::{Numerators, RecipeError},
    statistics::{RawAnalysis, RawValue},
};
use nrz_matrix::sampling::{IterationOutcome, SamplingReport, SchedulePosition};

/// Reserve every possible conversion slot before admitting host sampling.
pub(super) fn reserve(out: &mut Observations, remaining: u64, samples: u32) -> Result<(), Reason> {
    let attempts = remaining
        .checked_sub(out.counts.attempted)
        .ok_or(Reason::Capacity)?;
    let attempts = usize::try_from(attempts).map_err(|_| Reason::Capacity)?;
    let per_attempt = size_of::<Option<u64>>()
        .checked_add(size_of::<AttemptException>())
        .ok_or(Reason::Capacity)?;
    let bytes = attempts
        .checked_mul(per_attempt)
        .and_then(|n| n.checked_add((samples as usize).checked_mul(size_of::<RawRow>())?))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(Reason::Capacity)?;
    let mut budget = crate::memory::Budget::new(bytes);
    out.item_ns = budget.reserve(attempts).map_err(|_| Reason::Capacity)?;
    out.exceptions = budget.reserve(attempts).map_err(|_| Reason::Capacity)?;
    out.rows = budget
        .reserve(samples as usize)
        .map_err(|_| Reason::Capacity)?;
    Ok(())
}

/// Copy every item observation and sparse failure before sampler state is released.
fn observations(
    report: &SamplingReport<'_, NoEngineWork, RecipeError>,
    out: &mut Observations,
) -> Result<(), Reason> {
    out.linear_step = report.linear_step();
    out.sampler_stop = (*report.stop()).into();
    for (index, record) in report.iterations().iter().enumerate() {
        let duration = record
            .elapsed
            .map(|duration| u64::try_from(duration.as_nanos()))
            .transpose()
            .map_err(|_| Reason::Clock)?;
        out.item_ns.push(duration);
        let finalization = [
            record.disposal.and_then(Result::err),
            record.cleanup.and_then(Result::err),
        ];
        let failed = !matches!(record.outcome, IterationOutcome::Returned(Ok(())));
        if failed || finalization.iter().any(Option::is_some) {
            out.exceptions.push(AttemptException {
                attempt: u64::try_from(index).map_err(|_| Reason::Capacity)?,
                outcome: record.outcome.into(),
                finalization,
            });
        }
        match record.position {
            SchedulePosition::WarmUp(_) => out.counts.warm_up += 1,
            SchedulePosition::Measurement(_) => out.counts.measurement += 1,
        }
    }
    for (index, row) in report.rows().iter().enumerate() {
        let row_index = u32::try_from(index).map_err(|_| Reason::Capacity)?;
        let elapsed_ns = report
            .iterations()
            .iter()
            .filter(|iteration| iteration.position == SchedulePosition::Measurement(row_index))
            .try_fold(0_u64, |sum, iteration| {
                let nanos = u64::try_from(iteration.elapsed?.as_nanos()).ok()?;
                sum.checked_add(nanos)
            })
            .ok_or(Reason::Clock)?;
        out.rows.push(RawRow {
            iterations: row.iterations,
            elapsed_ns,
            seconds: RawValue::time(row.elapsed_seconds, -9, Rounding::Nearest)
                .map_err(|_| Reason::Clock)?,
        });
        out.counts.measured = out
            .counts
            .measured
            .checked_add(row.iterations)
            .ok_or(Reason::Capacity)?;
    }
    out.counts.rows = u64::try_from(out.rows.len()).map_err(|_| Reason::Capacity)?;
    out.counts.attempted = out.counts.pilot + out.counts.warm_up + out.counts.measurement;
    out.counts.discarded = out
        .counts
        .attempted
        .checked_sub(out.counts.measured)
        .ok_or(Reason::Capacity)?;
    out.counts.resamples = u64::from(report.achieved_resamples());
    out.phases.warm_up_ns =
        u64::try_from(report.warm_up_elapsed().as_nanos()).map_err(|_| Reason::Clock)?;
    out.phases.measurement_ns = u64::try_from(
        report
            .measured_elapsed()
            .saturating_sub(report.warm_up_elapsed())
            .as_nanos(),
    )
    .map_err(|_| Reason::Clock)?;
    Ok(())
}

/// Cancellation before publication removes complete estimates even after internal success.
pub(super) fn outcome(
    report: &SamplingReport<'_, NoEngineWork, RecipeError>,
    mut raw: Observations,
    numerators: Numerators,
    stopped: Option<Reason>,
) -> CaseOutcome {
    let converted = observations(report, &mut raw);
    let worker_failed = raw.exceptions.iter().any(|entry| {
        matches!(
            entry.outcome,
            crate::sampling_evidence::IterationOutcome::ResetFailed(RecipeError::Worker)
                | crate::sampling_evidence::IterationOutcome::Returned(Err(RecipeError::Worker))
        )
    });
    let reason = if worker_failed {
        Some(Reason::Worker)
    } else if let Some(reason) = stopped {
        Some(reason)
    } else {
        converted.err()
    };
    if let Some(reason) = reason {
        raw.counts.resamples = 0;
        return CaseOutcome::Incomplete(Box::new(PartialMeasurement {
            reason,
            observations: raw,
        }));
    }
    let Some(Ok(analysis)) = report.analysis().map(RawAnalysis::from_host) else {
        return incomplete(raw);
    };
    let [items, elements, vectors, floating, bytes] = [
        Some(numerators.items),
        Some(numerators.elements),
        numerators.vector_operations,
        numerators.floating_operations,
        numerators.logical_bytes,
    ]
    .map(|count| {
        count
            .map(|n| crate::statistics::RawRate::from_time(&analysis.estimates[4], n))
            .transpose()
    });
    let rates = (|| {
        Ok::<_, crate::decimal::DecimalError>([items?, elements?, vectors?, floating?, bytes?])
    })();
    if let Ok(rates) = rates {
        CaseOutcome::Complete(Box::new(Measurement {
            observations: raw,
            analysis,
            rates,
            numerators,
        }))
    } else {
        incomplete(raw)
    }
}

/// A failed numeric projection preserves observations and removes complete analysis counts.
fn incomplete(mut observations: Observations) -> CaseOutcome {
    observations.counts.resamples = 0;
    CaseOutcome::Incomplete(Box::new(PartialMeasurement {
        reason: Reason::Sampling,
        observations,
    }))
}
