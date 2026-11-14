// SPDX-License-Identifier: AGPL-3.0-only
//! Checked aggregate report reservations before any workload preparation.

mod capture;
mod owned;

#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) use capture::bytes as capture_bytes;

#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) use owned::{Budget, clone_case, clone_request};

use crate::{RequestCause, RequestError, RunRequest};

/// A request capacity refusal precedes any benchmark or worker admission.
const fn capacity() -> RequestError {
    RequestError {
        field: "aggregate payload",
        cause: RequestCause::Capacity,
    }
}

/// Checked native byte arithmetic, also constrained to the exact wire count domain.
fn add(a: u64, b: u64) -> Result<u64, RequestError> {
    a.checked_add(b)
        .filter(|n| *n <= crate::decimal::MAX_INTEGER)
        .ok_or_else(capacity)
}

/// Checked native payload product.
fn multiply(a: u64, b: u64) -> Result<u64, RequestError> {
    a.checked_mul(b)
        .filter(|n| *n <= crate::decimal::MAX_INTEGER)
        .ok_or_else(capacity)
}

/// Size conversion cannot narrow an owned vector capacity.
fn size<T>() -> Result<u64, RequestError> {
    u64::try_from(size_of::<T>()).map_err(|_| capacity())
}

/// Account for the owned copy of caller request strings and numerical vectors.
fn request_bytes(request: &RunRequest) -> Result<u64, RequestError> {
    let mut bytes = u64::try_from(request.run_id.capacity()).map_err(|_| capacity())?;
    bytes = add(
        bytes,
        multiply(
            u64::try_from(request.seeds.capacity()).map_err(|_| capacity())?,
            size::<crate::Seed>()?,
        )?,
    )?;
    bytes = add(
        bytes,
        multiply(
            u64::try_from(request.cases.capacity()).map_err(|_| capacity())?,
            size::<crate::CaseRequest>()?,
        )?,
    )?;
    bytes = add(bytes, size::<RunRequest>()?)?;
    for case in &request.cases {
        let values = case
            .shape
            .capacity()
            .checked_add(case.allowed_cpus.capacity())
            .and_then(|n| n.checked_add(case.cycles.capacity()))
            .ok_or_else(capacity)?;
        bytes = add(
            bytes,
            multiply(u64::try_from(values).map_err(|_| capacity())?, 8)?,
        )?;
        bytes = add(
            bytes,
            multiply(
                u64::try_from(case.inputs.capacity()).map_err(|_| capacity())?,
                size::<Vec<crate::Binary64>>()?,
            )?,
        )?;
        for input in &case.inputs {
            bytes = add(
                bytes,
                multiply(u64::try_from(input.capacity()).map_err(|_| capacity())?, 8)?,
            )?;
        }
    }
    Ok(bytes)
}

/// The sampler gets only its own concurrent reservation, including its analysis report.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn sampler_bytes(request: &RunRequest, attempts: u64) -> Result<u64, RequestError> {
    use crate::{adapter::NoEngineWork, recipe::RecipeError};
    use nrz_matrix::sampling::{IterationRecord, SampleRow, SamplingReport};
    let fixed = add(
        size::<SamplingReport<'_, NoEngineWork, RecipeError>>()?,
        request.sampling.analysis_report_bytes,
    )?;
    let rows = multiply(u64::from(request.sampling.samples), size::<SampleRow>()?)?;
    let iterations = multiply(
        attempts,
        size::<IterationRecord<NoEngineWork, RecipeError>>()?,
    )?;
    add(add(fixed, rows)?, iterations)
}

/// Named fixed payloads replace an unstructured per-case padding estimate.
fn case_payload() -> Result<u64, RequestError> {
    // Every load record has at most sixteen dynamic strings, including nested reasons.
    // Each is bounded by the probe payload plus its truncation sentinel.
    let load = add(
        size::<crate::telemetry::LoadSnapshot>()?,
        multiply(16, 65_537)?,
    )?;
    // Arithmetic retains 64 seeds and two two-field coefficient tuples per seed.
    let arithmetic = multiply(64 * 5, size::<crate::Binary64>()?)?;
    // The fixed pilot admits 256 controls and at most 21 eight-item batches.
    let pilot = add(
        multiply(256 + 21 * 8, size::<u64>()?)?,
        add(
            multiply(21, size::<crate::pilot::PilotBatch>()?)?,
            size::<crate::pilot::PilotFailure>()?,
        )?,
    )?;
    // Packing evidence has four bounded strings; LC input populations have four entries.
    let recipe = add(multiply(4, 65_537)?, multiply(4, size::<u64>()?)?)?;
    [
        size::<crate::Measurement>()?,
        multiply(2, load)?,
        arithmetic,
        pilot,
        recipe,
    ]
    .into_iter()
    .try_fold(0, add)
}

/// Reserve retained evidence plus simultaneous host-sampler and conversion payloads.
///
/// String growth is covered by twice the schema's maximum encoded scalar width.
/// Native stack frames, allocator metadata and caller-owned inputs are exclusions.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn preflight(
    request: &RunRequest,
    capture_bytes: u64,
    outcome_capacity: usize,
) -> Result<(), RequestError> {
    let outcomes = u64::try_from(outcome_capacity).map_err(|_| capacity())?;
    let retained_attempt = add(
        size::<Option<u64>>()?,
        add(
            size::<crate::report::AttemptException>()?,
            size::<crate::pilot::PilotFailure>()?,
        )?,
    )?;
    let attempts = multiply(
        multiply(outcomes, request.resources.attempts)?,
        retained_attempt,
    )?;
    let rows = multiply(
        multiply(outcomes, u64::from(request.sampling.samples))?,
        add(size::<crate::report::RawRow>()?, 1278)?,
    )?;
    let scalar_strings = multiply(2, 24 * 639 + 16 * 15 + 10)?;
    let per_case = add(
        size::<crate::report::CaseRecord>()?,
        add(scalar_strings, case_payload()?)?,
    )?;
    let layouts = multiply(
        request.resources.workers,
        size::<crate::layout::WorkerLayout>()?,
    )?;
    let fixed = multiply(outcomes, add(per_case, layouts)?)?;
    let total = add(
        add(attempts, rows)?,
        add(add(fixed, request_bytes(request)?)?, capture_bytes)?,
    )?;
    let total = add(total, sampler_bytes(request, request.resources.attempts)?)?;
    if total > request.resources.report_bytes {
        return Err(capacity());
    }
    let stacks = if request.resources.workers > 1 {
        multiply(
            request.resources.workers,
            request.resources.worker_stack_bytes,
        )?
    } else {
        0
    };
    let transport = if request.resources.workers > 1 {
        crate::workers::transport_bytes(request.resources.workers).map_err(|_| capacity())?
    } else {
        0
    };
    let scratch = add(
        add(add(stacks, transport)?, 4097)?,
        request.sampling.analysis_scratch_bytes,
    )?;
    if scratch > request.resources.scratch_bytes {
        return Err(capacity());
    }
    Ok(())
}
