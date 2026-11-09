// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Criterion 0.8.2 linear scheduling and geometric warm-up, source revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream algorithm: `src/routine.rs`, `Function::warm_up` and `Routine::sample`;
//! `src/lib.rs`, linear `ActualSamplingMode::iteration_counts`.
//! Modifications: exact checked nanosecond ratios, explicit caps, per-iteration
//! timing/admission, no harness or output. Copyright (c) 2014 Jorge Aparicio.
//! MIT notice: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

use crate::sampling::{SamplingError, SamplingSettings};
use std::time::Duration;

/// Admit `k_j = j d` using exact integer nanoseconds, avoiding rounded-down ceilings.
/// The warm-up mean is elapsed / iterations, so d = ceil(T iterations / elapsed / triangular).
pub(in crate::sampling) fn linear_step(
    settings: SamplingSettings,
    iterations: u64,
    elapsed: Duration,
) -> Result<u64, SamplingError> {
    let samples = u128::from(settings.analysis.samples());
    let triangular = samples
        .checked_mul(samples + 1)
        .map(|n| n / 2)
        .ok_or(SamplingError::Arithmetic)?;
    let numerator = settings
        .measurement
        .as_nanos()
        .checked_mul(u128::from(iterations))
        .ok_or(SamplingError::Arithmetic)?;
    let denominator = elapsed
        .as_nanos()
        .checked_mul(triangular)
        .filter(|n| *n != 0)
        .ok_or(SamplingError::Arithmetic)?;
    let step = numerator
        .checked_div(denominator)
        .and_then(|n| n.checked_add(u128::from(!numerator.is_multiple_of(denominator))))
        .map(|n| n.max(1))
        .ok_or(SamplingError::Arithmetic)?;
    let largest = samples.checked_mul(step).ok_or(SamplingError::Arithmetic)?;
    let total = triangular
        .checked_mul(step)
        .and_then(|n| n.checked_add(u128::from(iterations)))
        .ok_or(SamplingError::Arithmetic)?;
    if largest > u128::from(settings.max_iterations_per_sample)
        || total > u128::from(settings.max_total_iterations)
    {
        return Err(SamplingError::IterationCap);
    }
    u64::try_from(step).map_err(|_| SamplingError::Arithmetic)
}

/// Only whole doubled groups enter warm-up, with checked total and group caps.
pub(in crate::sampling) fn next_group(
    current: u64,
    completed: u64,
    settings: SamplingSettings,
) -> Result<u64, SamplingError> {
    let next = current.checked_mul(2).ok_or(SamplingError::Arithmetic)?;
    let total = completed
        .checked_add(next)
        .ok_or(SamplingError::Arithmetic)?;
    if next > settings.max_iterations_per_sample || total > settings.max_total_iterations {
        return Err(SamplingError::IterationCap);
    }
    Ok(next)
}
