// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded caller metadata and simultaneous private/public capture ownership.

use super::{RequestError, RunContext, RunRequest, capacity};
use crate::{BuildRecord, EnvironmentRecord, PublicProjection, capture};

/// Complete provenance owners transferred into the final report without cloning.
pub(super) struct Captured {
    /// Upfront report identity and envelope allocations.
    pub(super) envelope: Envelope,
    /// Private immutable build evidence.
    pub(super) build: BuildRecord,
    /// Private retained environment evidence.
    pub(super) environment: EnvironmentRecord,
    /// Explicit bounded public projection.
    pub(super) publication: PublicProjection,
    /// UTC collection timestamp.
    pub(super) timestamp: String,
    /// Private caller invocation copied through the shared allowance.
    pub(super) command: Vec<String>,
}

/// Envelope storage is owned before workload admission, including the second run-ID copy.
pub(super) struct Envelope {
    /// Raw wire namespace.
    pub(super) namespace: String,
    /// Dataset owner.
    pub(super) owner: String,
    /// Dataset license.
    pub(super) license: String,
    /// Source revision population.
    pub(super) sources: Vec<String>,
    /// Dataset label population.
    pub(super) labels: Vec<String>,
    /// Full run and estimator identity.
    pub(super) identity: crate::report::RunIdentity,
}

impl Envelope {
    /// Reserve every fixed identity string and vector through the shared budget.
    fn reserve(
        request: &RunRequest,
        context: &RunContext,
        budget: &mut crate::memory::Budget,
    ) -> Result<Self, crate::recipe::RecipeError> {
        budget.charge(size_of::<Self>())?;
        let mut sources = budget.reserve(1)?;
        sources.push(budget.text(nrz_matrix::sampling::SOURCE_REVISION)?);
        let mut labels = budget.reserve(1)?;
        labels.push(budget.text("spec:nrz:compute-benchmarks")?);
        Ok(Self {
            namespace: budget.text("org.naturrhythmuszentrum.compute.raw")?,
            owner: budget.text("NRZ")?,
            license: budget.text("CC0-1.0")?,
            sources,
            labels,
            identity: crate::report::RunIdentity {
                run_id: budget.text(&request.run_id)?,
                recipe_revision: budget.text("compute-1")?,
                source_revision: budget.text(nrz_matrix::sampling::SOURCE_REVISION)?,
                estimator_revision: budget.text(nrz_matrix::sampling::ESTIMATOR_REVISION)?,
                object_format: context.object_format,
                source_attachment: crate::Evidence::Unknown(
                    budget.text("no reviewed source attachment supplied")?,
                ),
            },
        })
    }
}

/// Capture and reserve provenance before admitting numerical workload preparation.
pub(super) fn capture(
    request: &RunRequest,
    context: &RunContext,
    owned_request: &RunRequest,
    ownership: &mut crate::memory::Budget,
    outcome_capacity: usize,
    offset_ns: u64,
    timestamp: fn() -> Result<String, &'static str>,
) -> Result<Captured, RequestError> {
    let before_envelope = ownership.remaining();
    let envelope = Envelope::reserve(request, context, ownership).map_err(|_| capacity())?;
    let envelope_bytes = before_envelope - ownership.remaining();
    let mut scratch = crate::memory::Budget::new(request.resources.scratch_bytes);
    let alias = scratch
        .text(&context.machine_alias)
        .map_err(|_| capacity())?;
    let probe_cap = capture::probe_allowance(scratch.remaining()).map_err(|_| capacity())?;
    let build = capture::build(ownership).map_err(|_| capacity())?;
    let build_bytes = usize::try_from(build.owned_bytes()?).map_err(|_| capacity())?;
    let environment = capture::environment(alias, offset_ns, probe_cap);
    let timestamp = timestamp().map_err(|_| capacity())?;
    let mut command = ownership
        .reserve(context.command.len())
        .map_err(|_| capacity())?;
    for argument in &context.command {
        command.push(ownership.text(argument).map_err(|_| capacity())?);
    }
    let capture_bytes = crate::memory::capture_bytes(&build, &environment, &command, &timestamp)?;
    // Command capacities were charged individually before copying; all other capture owners
    // are temporary scratch until this checked transfer to retained report ownership.
    let command_bytes = command.iter().try_fold(
        command
            .capacity()
            .checked_mul(size_of::<String>())
            .ok_or_else(capacity)?,
        |sum, value| sum.checked_add(value.capacity()).ok_or_else(capacity),
    )?;
    ownership
        .charge(
            usize::try_from(capture_bytes)
                .map_err(|_| capacity())?
                .checked_sub(command_bytes)
                .and_then(|bytes| bytes.checked_sub(build_bytes))
                .ok_or_else(capacity)?,
        )
        .map_err(|_| capacity())?;
    let before_projection = ownership.remaining();
    let publication = crate::PublicProjection::captured(&build, &environment, &command, ownership)
        .map_err(|_| capacity())?;
    let capture_bytes = capture_bytes
        .checked_add(before_projection - ownership.remaining())
        .ok_or_else(capacity)?;
    crate::memory::preflight(
        owned_request,
        capture_bytes
            .checked_add(envelope_bytes)
            .ok_or_else(capacity)?,
        outcome_capacity,
    )?;
    Ok(Captured {
        envelope,
        build,
        environment,
        publication,
        timestamp,
        command,
    })
}
