// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed-cost admission and aggregate payload accounting.

use super::{
    AdapterBounds, IterationRecord, Representative, SamplingError, SamplingReport, SamplingSettings,
};
use std::time::Duration;

/// All validated reservations and composed execution declarations.
#[derive(Clone, Copy, Debug)]
pub(super) struct Admission {
    /// Full iteration-record reservation, including reset failures.
    pub(super) iterations: usize,
    /// Complete measurement-row reservation.
    pub(super) rows: usize,
    /// Payload not available to collection vectors.
    pub(super) fixed_report: usize,
    /// Composed uninterrupted bound U.
    pub(super) uninterrupted: Duration,
    /// Complete sequential finalization bound C.
    pub(super) finalization: Duration,
}

/// Validate every policy/count/product before executing even a reset callback.
pub(super) fn validate<W, E, I: ?Sized>(
    settings: SamplingSettings,
    representative: &Representative<'_, I>,
    bounds: AdapterBounds,
) -> Result<Admission, SamplingError> {
    validate_settings(settings, representative)?;
    let iterations =
        usize::try_from(settings.max_total_iterations).map_err(|_| SamplingError::Capacity)?;
    let rows = usize::try_from(settings.analysis.samples()).map_err(|_| SamplingError::Capacity)?;
    let fixed_report = size_of::<SamplingReport<'_, W, E>>()
        .checked_add(settings.analysis.caps().report_bytes)
        .ok_or(SamplingError::Capacity)?;
    let report_bytes = bytes::<IterationRecord<W, E>>(iterations)?
        .checked_add(bytes::<super::SampleRow>(rows)?)
        .and_then(|n| n.checked_add(fixed_report))
        .ok_or(SamplingError::Capacity)?;
    if report_bytes > settings.caps.report_bytes {
        return Err(SamplingError::Capacity);
    }
    // Fixed scalar call frames and caller-owned adapter/clock/input storage are
    // excluded, as in analysis. No collection scratch vector is allocated.
    if settings.analysis.caps().scratch_bytes > settings.caps.scratch_bytes {
        return Err(SamplingError::Capacity);
    }
    let longest = bounds
        .uninterrupted
        .max(bounds.receipt)
        .max(bounds.disposal)
        .max(bounds.cleanup);
    let uninterrupted = longest
        .checked_add(settings.bounds.uninterrupted)
        .ok_or(SamplingError::Arithmetic)?;
    let finalization = bounds
        .receipt
        .checked_add(bounds.disposal)
        .and_then(|n| n.checked_add(bounds.cleanup))
        .and_then(|n| n.checked_add(settings.bounds.finalization))
        .ok_or(SamplingError::Arithmetic)?;
    settings
        .wall_budget
        .checked_add(uninterrupted)
        .and_then(|n| n.checked_add(finalization))
        .ok_or(SamplingError::Arithmetic)?;
    Ok(Admission {
        iterations,
        rows,
        fixed_report,
        uninterrupted,
        finalization,
    })
}

/// Validate user-controlled scalar policy; analysis settings are already validated.
fn validate_settings<I: ?Sized>(
    settings: SamplingSettings,
    item: &Representative<'_, I>,
) -> Result<(), SamplingError> {
    let zero_time = settings.warm_up.is_zero()
        || settings.measurement.is_zero()
        || settings.wall_budget.is_zero();
    let invalid_caps = settings.max_total_iterations == 0
        || settings.max_iterations_per_sample == 0
        || settings.max_iterations_per_sample > settings.max_total_iterations
        || settings.max_total_iterations > (1_u64 << 53);
    let identity = item.identity;
    let missing_identity = identity.representative.is_empty()
        || identity.operation_revision.is_empty()
        || identity.workload.is_empty()
        || identity.scope.is_empty()
        || identity.trace_configuration.is_empty();
    if zero_time || invalid_caps || missing_identity {
        return Err(SamplingError::InvalidSettings);
    }
    if let Some(elements) = item.elements_per_item
        && (elements == 0 || elements > (1_u64 << 53))
    {
        return Err(SamplingError::InvalidSettings);
    }
    let samples = u64::from(settings.analysis.samples());
    let minimum = samples
        .checked_add(1)
        .and_then(|n| n.checked_mul(samples))
        .map(|n| n / 2)
        .ok_or(SamplingError::Arithmetic)?;
    if samples > settings.max_iterations_per_sample || minimum >= settings.max_total_iterations {
        return Err(SamplingError::IterationCap);
    }
    Ok(())
}

/// Checked vector payload, including the allocator's actual capacity.
pub(super) const fn bytes<T>(length: usize) -> Result<usize, SamplingError> {
    match length.checked_mul(size_of::<T>()) {
        Some(n) if n <= isize::MAX as usize => Ok(n),
        Some(_) | None => Err(SamplingError::Capacity),
    }
}

/// Fallible reservation; the caller checks actual aggregate capacity afterwards.
pub(super) fn reserve<T>(length: usize) -> Result<Vec<T>, SamplingError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| SamplingError::Allocation)?;
    Ok(values)
}
