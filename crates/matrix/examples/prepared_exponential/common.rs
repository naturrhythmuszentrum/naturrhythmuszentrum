// SPDX-License-Identifier: AGPL-3.0-only
//! Shared populations and complete result snapshots for prepared measurements.
use super::support::TestResult;
use nrz_scalar::{ExpFailure, ExpOutput, Expm1Failure, Expm1Output};

/// Exact represented result identity, including typed refusals.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Snapshot {
    /// Endpoints, centre, total radius and numerical error, or the expm1 cause.
    Expm1(Result<[u64; 5], Expm1Failure>),
    /// Endpoints, centre, total radius and numerical error, or the exp cause.
    Exp(Result<[u64; 5], ExpFailure>),
}

/// Preserve every expm1 field rather than a lossy centre checksum.
pub(super) fn snapshot_expm1(result: Result<Expm1Output, Expm1Failure>) -> Snapshot {
    Snapshot::Expm1(result.map(|value| {
        [
            value.endpoints[0],
            value.endpoints[1],
            value.center,
            value.total_radius,
            value.numerical_error,
        ]
        .map(f64::to_bits)
    }))
}

/// Preserve every exp field and its exact refusal variant.
pub(super) fn snapshot_exp(result: Result<ExpOutput, ExpFailure>) -> Snapshot {
    Snapshot::Exp(result.map(|value| {
        [
            value.endpoints[0],
            value.endpoints[1],
            value.center,
            value.total_radius,
            value.numerical_error,
        ]
        .map(f64::to_bits)
    }))
}

/// Bound supplied populations before allocation; the fixture is not a public numerical API.
pub(super) fn inputs(count: u32) -> TestResult<Vec<f64>> {
    admitted_inputs(count, |count| {
        // Multiplication by an odd number permutes a power-of-two population.
        // The last two entries are replaced by distinct cancellation-sensitive points.
        let mut values: Vec<_> = (0..count)
            .map(|i| -2.0 * f64::from((i * 13) % count) / f64::from(count))
            .collect();
        let end = values.len();
        values[end - 2] = -2.0_f64.powi(-40);
        values[end - 1] = -2.0_f64.powi(-10) / f64::from(count);
        values
    })
}

/// Admission precedes calling the allocation-producing population builder.
pub(super) fn admitted_inputs(
    count: u32,
    build: impl FnOnce(u32) -> Vec<f64>,
) -> TestResult<Vec<f64>> {
    if !(4..=4096).contains(&count) || !count.is_power_of_two() {
        return Err("cell count must be a power of two between 4 and 4096".into());
    }
    Ok(build(count))
}
