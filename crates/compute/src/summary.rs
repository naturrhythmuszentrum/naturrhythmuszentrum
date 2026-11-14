// SPDX-License-Identifier: AGPL-3.0-only
//! Pure deterministic projection; never collection, clock access or command execution.

use crate::report::{
    CaseOutcome, Counts, Execution, PhaseDurations, RawContentId, RunIdentity, RunReport,
    SummaryRecipe,
};
use crate::statistics::{DecimalEstimate, RateEstimate};
use crate::{ResourcePolicy, Seed};
use serde::{Deserialize, Serialize};

/// Fixed statistical summary; no observation arrays or source-statistic bits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryAnalysis {
    /// Mean, median, sample standard deviation, scaled MAD and slope.
    pub estimates: [DecimalEstimate; 5],
    /// Rounded confidence; exact input lives at the declared raw pointer.
    pub confidence: String,
    /// Lower outer, lower inner, upper inner, upper outer fence decimals.
    pub fences: [String; 4],
    /// Shared fence decimal quantum in seconds.
    pub fence_grid: i32,
    /// Fixed-category row partition, never per-row labels.
    pub outlier_counts: [u64; 5],
    /// Aggregate items per second.
    pub items_per_second: RateEstimate,
    /// Scalar elements processed per second.
    pub elements_per_second: RateEstimate,
    /// Logical vector operations per second, absent for scalar sources.
    pub vector_operations_per_second: Option<RateEstimate>,
    /// Conventional arithmetic operations per second where defined by the recipe.
    pub floating_operations_per_second: Option<RateEstimate>,
    /// Logical operand bytes per second, absent for register arithmetic.
    pub logical_bytes_per_second: Option<RateEstimate>,
}

/// Summary outcome has fixed shape independent of attempted observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "value", deny_unknown_fields)]
pub enum SummaryOutcome {
    /// Complete requested schedule and bootstrap.
    Complete(Box<SummaryAnalysis>),
    /// Started but incomplete, with no complete estimates.
    Incomplete(crate::report::Reason),
    /// Valid but unsupported case.
    Unsupported(crate::report::Reason),
    /// Unvisited after a run-wide stop.
    NotStarted(crate::report::Reason),
}

/// Exactly one case/repetition record, without any raw observation arrays.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryCase {
    /// Per-worker startup storage evidence, never per-observation metadata.
    pub worker_layouts: Vec<crate::layout::WorkerLayout>,
    /// Original ordered case index.
    pub case_index: u64,
    /// Original independent repetition index.
    pub repetition: u64,
    /// Full recipe seed identity.
    pub seed: Seed,
    /// Effective input offset.
    pub effective_offset: u64,
    /// Fixed public recipe descriptor or raw input reference.
    pub recipe: SummaryRecipe,
    /// Accepted fixed per-worker item length.
    pub accepted_length: Option<u64>,
    /// Complete attempted/measured/discarded partition.
    pub counts: Counts,
    /// Fixed phase duration totals.
    pub phases: PhaseDurations,
    /// Fixed arithmetic byte/cycle identity and raw exact coefficient reference.
    pub arithmetic: Option<SummaryArithmetic>,
    /// Fixed diagnostic summaries; no chronological pilot arrays.
    pub pilot: Option<SummaryPilot>,
    /// Public before-case load projection.
    pub load_before: Option<crate::SummaryLoad>,
    /// Public after-case load projection.
    pub load_after: Option<crate::SummaryLoad>,
    /// Exact counter-derived interval fractions, absent without both snapshots.
    pub load_fractions: Option<crate::LoadFractions>,
    /// Typed completion and analysis status.
    pub outcome: SummaryOutcome,
}

/// Report-level request metadata cannot grow with case observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryRequest {
    /// Requested case count.
    pub cases: u64,
    /// Requested independent repetition count.
    pub repetitions: u64,
    /// Requested samples per case.
    pub samples: u32,
    /// Requested resamples per complete case.
    pub resamples: u32,
    /// Rounded confidence input; not an endpoint-valued sampler setting.
    pub confidence: String,
    /// Canonical JSON Pointer to exact raw confidence bits.
    pub confidence_input: crate::report::InputReference,
    /// Explicit aggregate resource policy.
    pub resources: ResourcePolicy,
}

/// Summary payload is a public projection from one exact raw report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryPayload {
    /// Caller-supplied full digest of canonical raw bytes.
    pub raw: RawContentId,
    /// Exact raw schema version bound to that digest.
    pub raw_version: [u64; 3],
    /// Recipe and estimator identity.
    pub identity: RunIdentity,
    /// Public build projection.
    pub build: crate::BuildRecord,
    /// Public environment projection.
    pub environment: crate::EnvironmentRecord,
    /// Shared request settings and raw exact-input reference.
    pub request: SummaryRequest,
    /// Public execution projection.
    pub execution: Execution,
    /// Chronological bounded case metadata, never observation arrays.
    pub cases: Vec<SummaryCase>,
}

/// Current summary envelope; repository admission is a separate policy decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryDataset {
    /// Current summary namespace.
    pub namespace: String,
    /// Exact summary schema revision.
    pub version: [u64; 3],
    /// Estate owner.
    pub owner: String,
    /// Exported summary license.
    pub license: String,
    /// Bounded source references.
    pub sources: Vec<String>,
    /// Design labels.
    pub labels: Vec<String>,
    /// Deterministic public projection.
    pub payload: SummaryPayload,
}

/// Invalid raw evidence, digest declaration or exact projection.
#[derive(Debug, thiserror::Error)]
pub enum SummaryError {
    /// Input failed the owned strict report contract.
    #[error("invalid raw report: {0}")]
    Report(String),
    /// Malformed or mismatched content identity.
    #[error("invalid raw content identity")]
    ContentId,
    /// Exact numeric source or projection failed.
    #[error(transparent)]
    Decimal(#[from] crate::decimal::DecimalError),
}

/// Deterministically summarize already captured evidence, without commands or timing.
/// # Errors
/// Refuses invalid raw evidence, digest syntax/format mismatch and exact projection errors.
pub fn summarize(raw: &RunReport, raw_id: &RawContentId) -> Result<SummaryDataset, SummaryError> {
    crate::codec::validate_report(raw).map_err(|error| SummaryError::Report(error.to_string()))?;
    if !raw_id.is_valid() || raw_id.algorithm != raw.payload.identity.object_format {
        return Err(SummaryError::ContentId);
    }
    let request = &raw.payload.request;
    let cases = raw
        .payload
        .cases
        .iter()
        .map(|record| project_case(record, request, raw_id))
        .collect::<Result<Vec<_>, _>>()?;
    let build = raw.payload.publication.build.clone();
    let mut execution = raw.payload.execution.clone();
    execution
        .command
        .clone_from(&raw.payload.publication.command);
    Ok(SummaryDataset {
        namespace: "org.naturrhythmuszentrum.compute.summary".to_owned(),
        version: [1, 1, 0],
        owner: raw.owner.clone(),
        license: raw.license.clone(),
        sources: raw.sources.clone(),
        labels: raw.labels.clone(),
        payload: SummaryPayload {
            raw: raw_id.clone(),
            raw_version: raw.version,
            identity: raw.payload.identity.clone(),
            build,
            environment: raw.payload.publication.environment.clone(),
            request: SummaryRequest {
                cases: u64::try_from(request.cases.len()).map_err(|_| SummaryError::ContentId)?,
                repetitions: u64::try_from(request.seeds.len())
                    .map_err(|_| SummaryError::ContentId)?,
                samples: request.sampling.samples,
                resamples: request.sampling.resamples,
                confidence: crate::decimal::fixed(
                    crate::decimal::Exact::from_bits(request.sampling.confidence.0)?,
                    -6,
                    crate::decimal::Rounding::Nearest,
                )?,
                confidence_input: crate::report::InputReference {
                    raw: raw_id.clone(),
                    pointer: "/payload/request/sampling/confidence".to_owned(),
                },
                resources: request.resources,
            },
            execution,
            cases,
        },
    })
}

/// Project one complete fixed statistical shape or preserve its non-success outcome.
fn project_case(
    record: &crate::report::CaseRecord,
    request: &crate::RunRequest,
    raw_id: &RawContentId,
) -> Result<SummaryCase, SummaryError> {
    let index = usize::try_from(record.case_index).map_err(|_| SummaryError::ContentId)?;
    let case = request.cases.get(index).ok_or(SummaryError::ContentId)?;
    let (counts, phases, outcome) = match &record.outcome {
        CaseOutcome::Complete(measurement) => (
            measurement.observations.counts,
            measurement.observations.phases,
            SummaryOutcome::Complete(Box::new(project_analysis(measurement)?)),
        ),
        CaseOutcome::Incomplete(partial) => (
            partial.observations.counts,
            partial.observations.phases,
            SummaryOutcome::Incomplete(partial.reason),
        ),
        CaseOutcome::Unsupported(reason) => (
            Counts::default(),
            PhaseDurations::default(),
            SummaryOutcome::Unsupported(*reason),
        ),
        CaseOutcome::NotStarted(reason) => (
            Counts::default(),
            PhaseDurations::default(),
            SummaryOutcome::NotStarted(*reason),
        ),
    };
    Ok(SummaryCase {
        worker_layouts: match &record.outcome {
            CaseOutcome::Complete(value) => value.observations.worker_layouts.clone(),
            CaseOutcome::Incomplete(value) => value.observations.worker_layouts.clone(),
            CaseOutcome::Unsupported(_) | CaseOutcome::NotStarted(_) => Vec::new(),
        },
        case_index: record.case_index,
        repetition: record.repetition,
        seed: record.seed,
        effective_offset: record.effective_offset,
        recipe: SummaryRecipe::from_request(
            case,
            record.case_index,
            raw_id,
            record.lc_ledger.clone(),
        )
        .map_err(|_| SummaryError::Report("invalid case protocol".to_owned()))?,
        accepted_length: record.accepted_length,
        arithmetic: project_arithmetic(record, request, raw_id)?,
        pilot: match &record.outcome {
            CaseOutcome::Complete(value) => Some(project_pilot(&value.observations.pilot)?),
            CaseOutcome::Incomplete(value) => Some(project_pilot(&value.observations.pilot)?),
            CaseOutcome::Unsupported(_) | CaseOutcome::NotStarted(_) => None,
        },
        counts,
        phases,
        load_before: record
            .load_before
            .as_ref()
            .map(crate::load_summary::project)
            .transpose()?,
        load_after: record
            .load_after
            .as_ref()
            .map(crate::load_summary::project)
            .transpose()?,
        load_fractions: record
            .load_before
            .as_ref()
            .zip(record.load_after.as_ref())
            .map(|(a, b)| crate::load_summary::fractions(a, b)),
        outcome,
    })
}

/// Six-figure rates are exact rational projections of the source slope endpoints.
fn project_analysis(
    measurement: &crate::report::Measurement,
) -> Result<SummaryAnalysis, SummaryError> {
    let analysis = &measurement.analysis;
    let estimates = analysis
        .estimates
        .each_ref()
        .map(crate::statistics::RawEstimate::summary);
    let [mean, median, deviation, mad, slope_estimate] = estimates;
    let slope = &analysis.estimates[4];
    let n = measurement.numerators;
    Ok(SummaryAnalysis {
        estimates: [mean?, median?, deviation?, mad?, slope_estimate?],
        confidence: analysis.confidence.decimal.clone(),
        fences: analysis
            .fences
            .each_ref()
            .map(|fence| fence.decimal.clone()),
        fence_grid: analysis.fence_grid,
        outlier_counts: analysis.outlier_counts,
        items_per_second: slope.rate(n.items)?,
        elements_per_second: slope.rate(n.elements)?,
        vector_operations_per_second: n
            .vector_operations
            .map(|count| slope.rate(count))
            .transpose()?,
        floating_operations_per_second: n
            .floating_operations
            .map(|count| slope.rate(count))
            .transpose()?,
        logical_bytes_per_second: n.logical_bytes.map(|count| slope.rate(count)).transpose()?,
    })
}

/// Fixed pilot diagnostics, independent of the number of attempted lengths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryPilot {
    /// Smallest positive observed monotonic tick.
    pub observed_tick_ns: Option<u64>,
    /// Available platform-declared resolution.
    pub declared_resolution_ns: crate::Evidence<u64>,
    /// Exact fixed-grid control median in seconds, with ten fractional digits.
    pub control_median_seconds: Option<String>,
    /// Largest retained control observation.
    pub control_maximum_ns: Option<u64>,
    /// Exact accepted-workload median on the same fixed grid.
    pub accepted_median_seconds: Option<String>,
    /// Largest retained observation at the accepted length.
    pub accepted_maximum_ns: Option<u64>,
    /// Required timing target, absent before control completion.
    pub target_ns: Option<u64>,
    /// Original typed pilot completion/refusal.
    pub stop: crate::pilot::PilotStop,
}

/// Project only complete diagnostic populations; stopped prefixes do not masquerade as full controls.
fn project_pilot(raw: &crate::pilot::PilotReport) -> Result<SummaryPilot, SummaryError> {
    let accepted = raw
        .accepted_length
        .and_then(|length| raw.batches.iter().find(|batch| batch.length == length));
    let control_median_seconds = if raw.empty_ns.len() == 256 {
        Some(pilot_median(&raw.empty_ns)?)
    } else {
        None
    };
    let accepted_median_seconds = accepted
        .map(|batch| pilot_median(&batch.elapsed_ns))
        .transpose()?;
    Ok(SummaryPilot {
        observed_tick_ns: raw.observed_tick_ns,
        declared_resolution_ns: raw.declared_resolution_ns.clone(),
        control_median_seconds,
        control_maximum_ns: raw.empty_ns.iter().copied().max(),
        accepted_median_seconds,
        accepted_maximum_ns: accepted.and_then(|batch| batch.elapsed_ns.iter().copied().max()),
        target_ns: raw.target_ns,
        stop: raw.stop,
    })
}

/// Integer central-pair arithmetic preserves half nanoseconds, regardless of clock resolution.
fn pilot_median(values: &[u64]) -> Result<String, SummaryError> {
    if values.is_empty() || values.len() > 256 || values.len() % 2 != 0 {
        return Err(SummaryError::Report("invalid median population".to_owned()));
    }
    let mut sorted = [0_u64; 256];
    sorted[..values.len()].copy_from_slice(values);
    sorted[..values.len()].sort_unstable();
    let exact = crate::decimal::Exact::midpoint_nanos(
        sorted[values.len() / 2 - 1],
        sorted[values.len() / 2],
    )?;
    Ok(crate::decimal::fixed(
        exact,
        -10,
        crate::decimal::Rounding::Nearest,
    )?)
}

/// Fixed arithmetic storage identity; coefficient/state arrays remain in the raw artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryArithmetic {
    /// Two alternating coefficient tuples per active state and lane.
    pub cycle: u64,
    /// Logical active binary64 seed/state bytes, excluding compiler spills.
    pub state_bytes: u64,
    /// Exact active coefficient-pair bytes, excluding compiler spills.
    pub coefficient_bytes: u64,
    /// Digest-bound location of all exact active coefficient and seed bits.
    pub exact_inputs: crate::report::InputReference,
}

/// Derive logical active bytes and an ordered-record pointer without copying coefficient arrays.
fn project_arithmetic(
    record: &crate::report::CaseRecord,
    request: &crate::RunRequest,
    raw: &RawContentId,
) -> Result<Option<SummaryArithmetic>, SummaryError> {
    let Some(inputs) = &record.arithmetic_inputs else {
        return Ok(None);
    };
    let state_bytes = u64::try_from(inputs.states.len())
        .ok()
        .and_then(|n| n.checked_mul(8))
        .ok_or(SummaryError::ContentId)?;
    let coefficient_bytes = state_bytes
        .checked_mul(inputs.tuple_fields)
        .and_then(|n| n.checked_mul(2))
        .ok_or(SummaryError::ContentId)?;
    let count = u64::try_from(request.cases.len()).map_err(|_| SummaryError::ContentId)?;
    let index = record
        .repetition
        .checked_mul(count)
        .and_then(|n| n.checked_add(record.case_index))
        .ok_or(SummaryError::ContentId)?;
    Ok(Some(SummaryArithmetic {
        cycle: 2,
        state_bytes,
        coefficient_bytes,
        exact_inputs: crate::report::InputReference {
            raw: raw.clone(),
            pointer: format!("/payload/cases/{index}/arithmetic_inputs"),
        },
    }))
}
