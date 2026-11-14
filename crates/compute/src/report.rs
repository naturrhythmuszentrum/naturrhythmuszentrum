// SPDX-License-Identifier: AGPL-3.0-only
//! Owned raw evidence and fixed-shape summary records.

use crate::{BuildRecord, CaseRequest, EnvironmentRecord, ExecutionBounds, RunRequest, Seed};
use crate::{pilot::PilotReport, recipe::Numerators, statistics::RawAnalysis};
use serde::{Deserialize, Serialize};

/// Raw object-format choice is independent of the rendering checkout.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ObjectFormat {
    /// Full SHA-1 Git blob content identity.
    #[serde(rename = "sha1")]
    Sha1,
    /// Full SHA-256 Git blob content identity.
    #[serde(rename = "sha256")]
    Sha256,
}

/// Caller-supplied full content digest of canonical raw bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawContentId {
    /// Algorithm chosen by the raw report.
    pub algorithm: ObjectFormat,
    /// Full lowercase hexadecimal digest, excluding the algorithm prefix.
    pub digest: String,
}

impl RawContentId {
    /// Check algorithm-specific full digest syntax, not unseen content binding.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let length = match self.algorithm {
            ObjectFormat::Sha1 => 40,
            ObjectFormat::Sha256 => 64,
        };
        let width = self.digest.len() == length;
        let digits = self
            .digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        width && digits
    }
}

/// Sanitized repository source attachment, bound by the summary admission receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAttachment {
    /// Repository-relative reviewed patch path, never a live URL.
    pub path: String,
    /// Full content identity of the exact reviewed attachment bytes.
    pub blob: RawContentId,
}

/// Shared recipe/estimator identity, with no pooled independent repetitions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunIdentity {
    /// Caller-assigned run slug.
    pub run_id: String,
    /// Revision of input, workload and timer recipe.
    pub recipe_revision: String,
    /// Actual matrix statistical source revision.
    pub source_revision: String,
    /// Actual estimator and RNG extraction revision.
    pub estimator_revision: String,
    /// Raw object format used for content binding.
    pub object_format: ObjectFormat,
    /// Reviewed source attachment identity if the build was dirty.
    pub source_attachment: crate::Evidence<SourceAttachment>,
}

/// Fixed phase order in report durations, without per-observation metadata growth.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseDurations {
    /// Admission and schema/count validation.
    pub validation_ns: u64,
    /// Input/worker preparation and dry validation.
    pub preparation_ns: u64,
    /// Clock/empty control and length pilot.
    pub pilot_ns: u64,
    /// Timed warm-up sum, distinct from wall preparation time.
    pub warm_up_ns: u64,
    /// Timed measured item sum, including failed/partial items.
    pub measurement_ns: u64,
    /// Observed worker release and owned-report finalization wall time.
    pub finalization_ns: u64,
    /// Reset and dry-validation wall time outside item timers.
    pub reset_ns: u64,
    /// Residual harness wall time, including sampler work without a separate phase clock.
    pub harness_ns: u64,
}

/// Run-wide execution facts retained in both raw and summary projections.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    /// Captured UTC run timestamp in RFC 3339 Z form.
    pub timestamp: String,
    /// Actual public invocation arguments, scrubbed by public projection.
    pub command: Vec<String>,
    /// Total wall duration including mandatory finalization; unavailable on clock-ledger failure.
    pub elapsed_ns: Option<u64>,
    /// Observed excess over the admission budget; unavailable with total duration.
    pub overshoot_ns: Option<u64>,
    /// Explicit outer execution assumptions, unavailable when unsupported.
    pub bounds: Option<ExecutionBounds>,
    /// Fixed phase duration ledger.
    pub phases: Option<PhaseDurations>,
    /// Every requested case/repetition completed with complete analysis and no run-wide failure.
    pub complete: bool,
    /// Run-wide failure after admission; completed case evidence is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<Reason>,
}

/// Typed non-success case reason, never a zero-rate observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Reason {
    /// Collection is claimed only on x86_64 Linux.
    Platform,
    /// Missing cooperative execution declarations.
    MissingBounds,
    /// Exact per-worker affinity cannot be set through the safe standard library.
    Affinity,
    /// Scaled matrix cases are outside the serial backend contract.
    ParallelMatrix,
    /// Selected source width is unavailable for this operation.
    Width,
    /// A matrix-owned adapter is unavailable.
    MatrixAdapter,
    /// Unsupported opaque packing under ``open:nrz:compute-backend-packing-requirement``.
    BackendPacking,
    /// Explicit cancellation was observed.
    Cancelled,
    /// Outer admission duration expired.
    Deadline,
    /// Explicit report, input or worker capacity failed.
    Capacity,
    /// Deterministic preparation or finite-domain validation failed.
    Preparation,
    /// Pilot target was unmet without an observed bound overrun.
    ItemTooShort,
    /// Executed pilot duration exceeded the declared bound.
    ItemExceedsBound,
    /// No useful monotonic clock observation was available.
    Clock,
    /// Host sampling stopped before complete analysis.
    Sampling,
    /// Worker execution or joining failed.
    Worker,
}

/// Completed row evidence with exact integer time and host seconds source bits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRow {
    /// Exact positive iteration count.
    pub iterations: u64,
    /// Sum of independently timed items in nanoseconds.
    pub elapsed_ns: u64,
    /// Actual sampler integer-to-seconds source and its nanosecond-grid view.
    pub seconds: crate::statistics::RawValue,
}

/// Sparse exceptional attempt; ordinary successful attempts need no extra record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptException {
    /// Zero-based index in the chronological item array.
    pub attempt: u64,
    /// Original sampler outcome tag.
    pub outcome: crate::sampling_evidence::IterationOutcome,
    /// Separate receipt/finalization failures, without replacing the first stop.
    pub finalization: [Option<crate::recipe::RecipeError>; 2],
}

/// Attempt partitions; outlier labels never reduce the measured count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    /// All attempts, including pilot, warm-up and untimed failures.
    pub attempted: u64,
    /// Valid timed items in retained complete measurement rows.
    pub measured: u64,
    /// Attempted minus measured, including unfinished rows.
    pub discarded: u64,
    /// Pilot and control attempts.
    pub pilot: u64,
    /// Sampler warm-up attempts.
    pub warm_up: u64,
    /// Sampler measurement attempts, including incomplete rows.
    pub measurement: u64,
    /// Complete measurement rows.
    pub rows: u64,
    /// Achieved resamples, zero without complete analysis.
    pub resamples: u64,
}

/// Raw measurement data shared by complete and partial outcomes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observations {
    /// Per-worker storage and startup evidence, empty if preparation did not finish.
    pub worker_layouts: Vec<crate::layout::WorkerLayout>,
    /// Diagnostic and length-sizing observations.
    pub pilot: PilotReport,
    /// Chronological sampler item times; null denotes an untimed attempt.
    pub item_ns: Vec<Option<u64>>,
    /// Sparse failures, indexed into `item_ns`.
    pub exceptions: Vec<AttemptException>,
    /// Complete measurement rows in chronological order.
    pub rows: Vec<RawRow>,
    /// Selected linear schedule step, absent before schedule admission.
    pub linear_step: Option<u64>,
    /// Original host stop enum, retained separately from outer stop.
    pub sampler_stop: crate::sampling_evidence::SamplingStop,
    /// Fixed attempt/count partition.
    pub counts: Counts,
    /// Fixed phase durations.
    pub phases: PhaseDurations,
}

/// Complete analysis is structurally absent from every partial outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "value", deny_unknown_fields)]
pub enum CaseOutcome {
    /// Entire requested row/resample schedule finished.
    Complete(Box<Measurement>),
    /// A started case stopped with preserved observations and no estimates.
    Incomplete(Box<PartialMeasurement>),
    /// Valid request could not be executed under available capabilities.
    Unsupported(Reason),
    /// Run-wide stop prevented this requested case from starting.
    NotStarted(Reason),
}

/// Owned complete measurement with unchanged host statistical evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    /// All observations and exact counts.
    pub observations: Observations,
    /// Complete fixed statistical shape with original bits.
    pub analysis: RawAnalysis,
    /// Exact reciprocal rate views in items, elements, vectors, floating operations and bytes order.
    pub rates: [Option<crate::statistics::RawRate>; 5],
    /// Explicit positive work units per aggregate item.
    pub numerators: Numerators,
}

/// Partial measurement has no field capable of carrying complete estimates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialMeasurement {
    /// First outer stop, separate from retained sampler and finalization failures.
    pub reason: Reason,
    /// All completed observations and unfinished-row evidence.
    pub observations: Observations,
}

/// One chronological requested case and independent repetition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseRecord {
    /// Index in the explicit request's ordered case array.
    pub case_index: u64,
    /// Index in the explicit repetition seed array.
    pub repetition: u64,
    /// Full seed, even when different seeds would collide in effective offset.
    pub seed: Seed,
    /// Explicit effective input offset.
    pub effective_offset: u64,
    /// Selected fixed per-worker item length, absent without an accepted pilot.
    pub accepted_length: Option<u64>,
    /// Exact active arithmetic seeds and coefficient pairs; absent for other families.
    pub arithmetic_inputs: Option<crate::recipe::ArithmeticInputs>,
    /// Exact preparation, storage and amortization ledger for an LC recipe.
    pub lc_ledger: Option<crate::recipe::LcLedger>,
    /// Packing requirement and explicitly unavailable tuning thresholds for backend cases.
    pub packing: Option<crate::recipe::PackingEvidence>,
    /// Bounded load capture before a started case.
    pub load_before: Option<crate::telemetry::LoadSnapshot>,
    /// Bounded load capture after the case, retained separately from requested controls.
    pub load_after: Option<crate::telemetry::LoadSnapshot>,
    /// Case result, retained even when unsupported or unvisited.
    pub outcome: CaseOutcome,
}

/// Raw payload retains the complete request and chronological observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunPayload {
    /// Shared identity and source revisions.
    pub identity: RunIdentity,
    /// Instrument-captured build evidence.
    pub build: BuildRecord,
    /// Instrument-captured environment with tagged unavailable facts.
    pub environment: EnvironmentRecord,
    /// Explicit public-field and redaction declarations used by deterministic summarization.
    pub publication: crate::provenance::PublicProjection,
    /// Original exact request, never reconstructed from rounded views.
    pub request: RunRequest,
    /// Complete outer execution record.
    pub execution: Execution,
    /// Ordered result for every case/repetition.
    pub cases: Vec<CaseRecord>,
}

/// Canonical raw JSON envelope, distinct from the summary dataset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunReport {
    /// Current raw namespace.
    pub namespace: String,
    /// Exact raw schema revision.
    pub version: [u64; 3],
    /// Estate owner.
    pub owner: String,
    /// Exported data license.
    pub license: String,
    /// Bounded source identities.
    pub sources: Vec<String>,
    /// Stable design labels.
    pub labels: Vec<String>,
    /// Full raw invocation evidence.
    pub payload: RunPayload,
}

/// Public recipe descriptor without raw numerical input arrays.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryRecipe {
    /// Complete fixed reset, cache, start and conditioning protocol.
    pub protocol: crate::recipe::CaseProtocol,
    /// Catalog operation.
    pub operation: crate::Operation,
    /// Source width.
    pub width: crate::Width,
    /// Dependency form.
    pub dependency: crate::Dependency,
    /// Timing scope.
    pub scope: crate::Scope,
    /// Fixed shape descriptor.
    pub shape: Vec<u64>,
    /// Requested active workers.
    pub workers: u64,
    /// Requested placement.
    pub placement: crate::Placement,
    /// JSON Pointer to original exact recipe inputs, bound to summary's raw digest.
    pub exact_input_pointer: InputReference,
    /// Complete LC population and work ledger, absent for other families.
    pub lc_ledger: Option<crate::recipe::LcLedger>,
    /// Packing requirement and explicitly unavailable tuning thresholds for backend cases.
    pub packing: Option<crate::recipe::PackingEvidence>,
}

impl SummaryRecipe {
    /// Project only fixed identity fields; large input arrays stay raw.
    pub(crate) fn from_request(
        case: &CaseRequest,
        index: u64,
        raw: &RawContentId,
        lc_ledger: Option<crate::recipe::LcLedger>,
    ) -> Result<Self, crate::recipe::RecipeError> {
        Ok(Self {
            protocol: crate::recipe::CaseProtocol::from_case(case)?,
            operation: case.operation,
            width: case.width,
            dependency: case.dependency,
            scope: case.scope,
            shape: case.shape.clone(),
            workers: case.workers,
            placement: case.placement,
            lc_ledger,
            packing: (case.dependency == crate::Dependency::BackendKernel)
                .then(|| crate::recipe::PackingEvidence::from_case(case))
                .transpose()?,
            exact_input_pointer: InputReference {
                raw: raw.clone(),
                pointer: format!("/payload/request/cases/{index}"),
            },
        })
    }
}

/// Exact-input reference binds a canonical JSON Pointer to the canonical raw blob.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputReference {
    /// Algorithm-qualified full raw content identity.
    pub raw: RawContentId,
    /// Canonical JSON Pointer, with exact escaping and canonical array indices.
    pub pointer: String,
}
