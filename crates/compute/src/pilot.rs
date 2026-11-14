// SPDX-License-Identifier: AGPL-3.0-only
//! Timer diagnostics and checked predictive item sizing.

use crate::recipe::RecipeError;
use nrz_matrix::sampling::SamplingClock;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Explicit pilot stop; a failed prefix never supplies estimates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PilotStop {
    /// Accepted a fixed length meeting the diagnostic timing target.
    Complete,
    /// No positive clock tick was observed.
    NoPositiveTick,
    /// Target unmet at a length/count cap or predictive bound refusal.
    ItemTooShort,
    /// An actually executed item exceeded its declared uninterrupted bound.
    ItemExceedsBound,
    /// Outer deadline or cancellation forbids further work.
    Stopped,
    /// Clock reversal or unrepresentable duration.
    Clock,
    /// Preparation, reset or execution failed.
    Operation,
    /// Worker transport or generation validation failed.
    Worker,
    /// Explicit report reservation or attempt capacity was insufficient.
    Capacity,
}

/// One attempted pilot length with chronological item durations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotBatch {
    /// Per-worker update/pass count.
    pub length: u64,
    /// Up to eight observations; a stopped batch remains partial.
    pub elapsed_ns: Vec<u64>,
}

/// Sparse failed pilot attempt, preserving a timed failure when execution began.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotFailure {
    /// Zero-based chronological control/pilot attempt index.
    pub attempt: u64,
    /// Timed duration, absent if reset/admission failed before execution.
    pub elapsed_ns: Option<u64>,
    /// Typed first failure.
    pub stop: PilotStop,
}

/// Complete diagnostic observations retained only in raw evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotReport {
    /// Smallest positive tick among 4096 consecutive reads, when available.
    pub observed_tick_ns: Option<u64>,
    /// Platform-declared resolution, when available.
    pub declared_resolution_ns: crate::Evidence<u64>,
    /// Chronological 256 empty-item durations, or a stopped prefix.
    pub empty_ns: Vec<u64>,
    /// Checked timing target, unavailable before controls finish.
    pub target_ns: Option<u64>,
    /// Chronological attempted lengths and observations.
    pub batches: Vec<PilotBatch>,
    /// Total attempts including untimed failures.
    pub attempted: u64,
    /// Sparse failed attempts, separate from complete-duration arrays.
    pub failures: Vec<PilotFailure>,
    /// Accepted length only after a complete successful pilot.
    pub accepted_length: Option<u64>,
    /// First stop reason.
    pub stop: PilotStop,
}

/// Adapter seam preserves identical empty/work dispatch and boundary consumption.
pub(crate) trait PilotOperation {
    /// Check outer admission without altering the monotonic clock.
    fn stopped(&self) -> bool;
    /// Restore input state outside each timed item; dry-check a new length.
    fn prepare(&mut self, length: u64) -> Result<(), RecipeError>;
    /// Execute the empty protocol or one workload item.
    fn execute(&mut self, length: Option<u64>) -> Result<(), RecipeError>;
}

/// Explicit pilot limits supplied by the already validated request/context.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PilotPolicy {
    /// Initial positive item length.
    pub(crate) length: u64,
    /// Maximum length under the checked operation/count caps.
    pub(crate) maximum_length: u64,
    /// Maximum diagnostic and pilot attempts remaining in the aggregate cap.
    pub(crate) attempts: u64,
    /// Declared uninterrupted item bound, including empty items.
    pub(crate) item_bound_ns: u64,
    /// Optional platform-declared positive clock resolution.
    pub(crate) resolution_ns: Option<u64>,
    /// Whether the selected scope permits length doubling.
    pub(crate) grow: bool,
}

/// Duration conversion refuses clock reversal and wire overflow.
fn elapsed(start: Duration, end: Duration) -> Result<u64, PilotStop> {
    let duration = end.checked_sub(start).ok_or(PilotStop::Clock)?;
    u64::try_from(duration.as_nanos())
        .ok()
        .filter(|n| *n <= crate::decimal::MAX_INTEGER)
        .ok_or(PilotStop::Clock)
}

/// Measure one independent item with the ordinary timer-pair boundary.
fn item(
    clock: &dyn SamplingClock,
    operation: &mut impl PilotOperation,
    length: Option<u64>,
    report: &mut PilotReport,
) -> Result<u64, PilotStop> {
    let attempt = report.attempted;
    report.attempted += 1;
    let result = timed_item(clock, operation, length);
    if let Err((stop, duration)) = result {
        report.failures.push(PilotFailure {
            attempt,
            elapsed_ns: duration,
            stop,
        });
    }
    result.map_err(|(stop, _)| stop)
}

/// Preserve elapsed time even when a timed operation returns an error.
fn timed_item(
    clock: &dyn SamplingClock,
    operation: &mut impl PilotOperation,
    length: Option<u64>,
) -> Result<u64, (PilotStop, Option<u64>)> {
    if operation.stopped() {
        return Err((PilotStop::Stopped, None));
    }
    operation
        .prepare(length.unwrap_or(0))
        .map_err(|error| (operation_stop(error), None))?;
    if operation.stopped() {
        return Err((PilotStop::Stopped, None));
    }
    let start = clock.now();
    let result = operation.execute(length);
    let end = clock.now();
    let duration = elapsed(start, end).map_err(|stop| (stop, None))?;
    result.map_err(|error| (operation_stop(error), Some(duration)))?;
    Ok(duration)
}

/// Retain worker failures separately so the collector can stop later requested cases.
const fn operation_stop(error: RecipeError) -> PilotStop {
    match error {
        RecipeError::Worker => PilotStop::Worker,
        RecipeError::Cancelled => PilotStop::Stopped,
        RecipeError::Allocation | RecipeError::Overflow => PilotStop::Capacity,
        RecipeError::Domain => PilotStop::Operation,
    }
}

/// Compute twice the median exactly, retaining half-nanosecond central pairs.
fn median_twice(values: &[u64]) -> Result<u64, PilotStop> {
    if values.is_empty() || values.len() > 256 || values.len() % 2 != 0 {
        return Err(PilotStop::Clock);
    }
    let mut copy = [0_u64; 256];
    copy[..values.len()].copy_from_slice(values);
    copy[..values.len()].sort_unstable();
    let middle = values.len() / 2;
    copy[middle - 1]
        .checked_add(copy[middle])
        .ok_or(PilotStop::Clock)
}

/// Collect fixed-count controls before admitting workload timing.
fn controls(
    clock: &dyn SamplingClock,
    operation: &mut impl PilotOperation,
    policy: PilotPolicy,
    report: &mut PilotReport,
) -> Result<(), PilotStop> {
    if operation.stopped() {
        return Err(PilotStop::Stopped);
    }
    let mut previous = clock.now();
    for _ in 1..4096 {
        let now = clock.now();
        let tick = elapsed(previous, now)?;
        if tick > 0 {
            report.observed_tick_ns =
                Some(report.observed_tick_ns.map_or(tick, |old| old.min(tick)));
        }
        previous = now;
    }
    if operation.stopped() {
        return Err(PilotStop::Stopped);
    }
    let observed = report.observed_tick_ns.ok_or(PilotStop::NoPositiveTick)?;
    if policy.attempts < 256 {
        return Err(PilotStop::Capacity);
    }
    for _ in 0..256 {
        let duration = item(clock, operation, None, report)?;
        report.empty_ns.push(duration);
        if duration > policy.item_bound_ns {
            return Err(PilotStop::ItemExceedsBound);
        }
    }
    let resolution = observed.max(policy.resolution_ns.unwrap_or(0));
    let target = resolution
        .checked_mul(100)
        .ok_or(PilotStop::Clock)?
        .max(
            median_twice(&report.empty_ns)?
                .checked_mul(50)
                .ok_or(PilotStop::Clock)?,
        )
        .max(100_000);
    report.target_ns = Some(target);
    if target > policy.item_bound_ns {
        return Err(PilotStop::ItemTooShort);
    }
    Ok(())
}

/// Perform bounded sizing, checking prediction before any doubled dispatch.
fn sizing(
    clock: &dyn SamplingClock,
    operation: &mut impl PilotOperation,
    policy: PilotPolicy,
    report: &mut PilotReport,
) -> Result<(), PilotStop> {
    let target = report.target_ns.ok_or(PilotStop::Clock)?;
    let mut length = policy.length;
    let mut attempts = 256;
    for doubling in 0..=20 {
        if length > policy.maximum_length {
            return Err(PilotStop::ItemTooShort);
        }
        let mut batch = PilotBatch {
            length,
            elapsed_ns: Vec::new(),
        };
        batch
            .elapsed_ns
            .try_reserve_exact(8)
            .map_err(|_| PilotStop::Capacity)?;
        if batch.elapsed_ns.capacity() != 8 {
            return Err(PilotStop::Capacity);
        }
        for _ in 0..8 {
            if attempts >= policy.attempts {
                report.batches.push(batch);
                return Err(PilotStop::Capacity);
            }
            attempts += 1;
            match item(clock, operation, Some(length), report) {
                Ok(duration) => {
                    batch.elapsed_ns.push(duration);
                    if duration > policy.item_bound_ns {
                        report.batches.push(batch);
                        return Err(PilotStop::ItemExceedsBound);
                    }
                }
                Err(stop) => {
                    report.batches.push(batch);
                    return Err(stop);
                }
            }
        }
        let median = median_twice(&batch.elapsed_ns)?;
        let maximum = batch
            .elapsed_ns
            .iter()
            .copied()
            .max()
            .ok_or(PilotStop::Clock)?;
        report.batches.push(batch);
        if median >= target.checked_mul(2).ok_or(PilotStop::Clock)? {
            report.accepted_length = Some(length);
            return Ok(());
        }
        let prediction = maximum
            .checked_mul(2)
            .filter(|n| *n <= policy.item_bound_ns);
        if prediction.is_none() || !policy.grow || doubling == 20 {
            return Err(PilotStop::ItemTooShort);
        }
        length = length.checked_mul(2).ok_or(PilotStop::ItemTooShort)?;
    }
    Err(PilotStop::ItemTooShort)
}

/// Empty evidence for a case that started preparation but never reached its pilot.
pub(crate) fn stopped(stop: PilotStop) -> PilotReport {
    PilotReport {
        observed_tick_ns: None,
        declared_resolution_ns: crate::Evidence::Unavailable(
            "safe std clock has no declared resolution".to_owned(),
        ),
        empty_ns: Vec::new(),
        target_ns: None,
        batches: Vec::new(),
        attempted: 0,
        failures: Vec::new(),
        accepted_length: None,
        stop,
    }
}

/// Run the fixed diagnostics and pilot while preserving every completed observation.
pub(crate) fn size(
    clock: &dyn SamplingClock,
    operation: &mut impl PilotOperation,
    policy: PilotPolicy,
) -> PilotReport {
    let mut report = stopped(PilotStop::Complete);
    report.declared_resolution_ns = policy.resolution_ns.map_or_else(
        || crate::Evidence::Unavailable("safe std clock has no declared resolution".to_owned()),
        crate::Evidence::Known,
    );
    if report.empty_ns.try_reserve_exact(256).is_err()
        || report.batches.try_reserve_exact(21).is_err()
        || report.failures.try_reserve_exact(1).is_err()
        || report.empty_ns.capacity() != 256
        || report.batches.capacity() != 21
        || report.failures.capacity() != 1
    {
        report.stop = PilotStop::Capacity;
        return report;
    }
    let result = controls(clock, operation, policy, &mut report)
        .and_then(|()| sizing(clock, operation, policy, &mut report));
    if let Err(stop) = result {
        report.stop = stop;
    }
    report
}

#[cfg(test)]
mod tests;
