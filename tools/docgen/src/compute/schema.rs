// SPDX-License-Identifier: AGPL-3.0-only
//! Independent structural summary DTOs; no compute or matrix dependency.

use serde::{Deserialize, Serialize};

/// Named arithmetic, streaming, matrix or naive LC recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum Operation {
    /// Alternating signed additions.
    Add,
    /// Alternating near-one factors.
    Multiply,
    /// Fused a*b+x with addend dependence.
    FmaAddend,
    /// Fused a*x+b with multiplicand dependence.
    FmaMultiplicand,
    /// Separate multiplication and addition.
    MultiplyAdd,
    /// Alternating runtime division factors.
    Divide,
    /// Square root with timed range-maintenance addition.
    SquareRoot,
    /// Eight-accumulator contiguous reduction.
    Read,
    /// Contiguous scalar fill.
    Write,
    /// Contiguous disjoint copy.
    Copy,
    /// Fused out=a*x+y traversal.
    FusedStream,
    /// Serial raw binary64 matrix product.
    Gemm,
    /// Serial raw binary64 matrix-vector product.
    Gemv,
    /// Serial partial-pivot LU construction.
    Factor,
    /// Serial solve with a prepared partial-pivot LU factor.
    Solve,
    /// Factored-root forward LC phase with fused traversal.
    ForwardFused,
    /// Factored-root forward LC phase with separate rounding.
    ForwardSeparate,
    /// Known-product inductance query.
    Inductance,
    /// Known-product capacitance query.
    Capacitance,
    /// Finite inverse-time branch selection.
    InverseTime,
}

/// Source-level width; no native instruction promise.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum Width {
    /// Scalar binary64 source.
    Scalar,
    /// One binary64 SIMD lane.
    Simd1,
    /// Two binary64 SIMD lanes.
    Simd2,
    /// Four binary64 SIMD lanes.
    Simd4,
    /// Eight binary64 SIMD lanes.
    Simd8,
    /// Matrix-owned backend lowering.
    BackendSelectedBinary64,
}

/// Declared dependency and output-consumption structure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum Dependency {
    /// One evolving state per lane.
    Dependent,
    /// Eight evolving states per lane.
    Independent,
    /// Eight SIMD partial accumulators.
    MultiAccumulatorReduction,
    /// Independent streaming elements.
    IndependentElements,
    /// Matrix backend operation.
    BackendKernel,
    /// Naive LC output traversal.
    PreparedTraversal,
}

/// Explicit allocation and restoration boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum Scope {
    /// Preparation and reset outside the timed operation.
    Prepared,
    /// Named preparation and allocation inside the timer.
    PreparationInclusive,
    /// Repeated factors with timed restoration before each factor.
    RestorationInclusiveRepeated,
}

/// Requested host placement; observed placement is recorded separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum Placement {
    /// No requested CPU constraint.
    Unbound,
    /// Caller supplies external CPU-set policy and evidence.
    ExternallyConstrained,
    /// Requests an unavailable safe per-worker binding setter.
    ExactWorkerBinding,
}

/// Missing evidence never silently becomes a known or default setting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", deny_unknown_fields)]
pub(super) enum Evidence<T> {
    /// Captured value with its method retained by the containing field.
    Known(T),
    /// A fact could not be established, with a bounded reason.
    Unknown(String),
    /// Probe missing or denied, with its explicit reason.
    Unavailable(String),
    /// Explicit public projection, never a reversible private mapping.
    Redacted(String),
}

/// Compile-time target selection intent, distinct from observed machine-code lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum BuildIntent {
    /// No native CPU or explicit target-feature selection was requested.
    Baseline,
    /// The effective target CPU option requests the build machine's native CPU.
    NativeRequested,
    /// An explicit CPU or target-feature set was requested.
    ExplicitTarget,
}

/// Structured machine-code inspection evidence; build intent never substitutes for inspection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoweringRecord {
    /// Whether an explicit inspection occurred.
    pub(super) inspected: Evidence<bool>,
    /// Full content identity of the inspected executable or object.
    pub(super) artifact: Evidence<RawContentId>,
    /// Exact inspected symbol names.
    pub(super) symbols: Evidence<Vec<String>>,
    /// Evidence of scalar-source auto-vectorization.
    pub(super) scalar_vectorization: Evidence<String>,
    /// Evidence of splitting a logical vector width.
    pub(super) splitting: Evidence<String>,
    /// Evidence of out-of-line helpers.
    pub(super) helpers: Evidence<String>,
    /// Whether inspected FMA operations use native instructions.
    pub(super) native_fma: Evidence<bool>,
    /// Bounded inspection method and evidence notes.
    pub(super) notes: Evidence<String>,
}

/// Build facts captured at compile time; runtime callers cannot replace them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuildRecord {
    /// Full compiler verbose version.
    pub(super) compiler: Evidence<String>,
    /// Source HEAD at build time.
    pub(super) source_commit: Evidence<String>,
    /// Whether tracked or untracked source was dirty at capture.
    pub(super) dirty: Evidence<bool>,
    /// Algorithm-qualified lockfile blob identity from checkout root.
    pub(super) lockfile: Evidence<String>,
    /// Cargo target triple.
    pub(super) target: Evidence<String>,
    /// Cargo profile.
    pub(super) profile: Evidence<String>,
    /// Effective inherited Cargo encoded rustflags.
    pub(super) flags: Evidence<String>,
    /// Captured target selection intent; runtime ISA never upgrades a baseline build.
    pub(super) intent: Evidence<BuildIntent>,
    /// Enabled Cargo features; distinct from ISA features.
    pub(super) features: Vec<String>,
    /// Backend package/version and explicit serial execution policy.
    pub(super) backend: Evidence<String>,
    /// Relevant package versions captured from the resolved checkout lockfile.
    pub(super) dependencies: Evidence<Vec<String>>,
    /// Inspection of actual lowered instructions, absent without explicit evidence.
    pub(super) lowering: LoweringRecord,
}

/// Explicit control request and observed state, without conflating them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Setting {
    /// Caller control declaration, not a verified probe.
    pub(super) requested: Evidence<String>,
    /// Observed setting or explicit unavailable state.
    pub(super) observed: Evidence<String>,
    /// Capture method, not executable instructions.
    pub(super) method: String,
    /// Capture offset in integer nanoseconds from run entry.
    pub(super) offset_ns: u64,
}

/// Bounded public machine environment; numeric telemetry uses typed source fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EnvironmentRecord {
    /// Opaque caller-assigned alias, not a hostname.
    pub(super) machine_alias: String,
    /// Public CPU model identity.
    pub(super) cpu_model: Evidence<String>,
    /// Runtime detected ISA names from a fixed revisioned list.
    pub(super) runtime_isa: Vec<String>,
    /// Compile-time ISA names from the same list.
    pub(super) compile_isa: Vec<String>,
    /// Topology policy and observed state.
    pub(super) topology: Setting,
    /// Private/shared cache configuration.
    pub(super) caches: Setting,
    /// Memory capacity/configuration.
    pub(super) memory: Setting,
    /// SMT state.
    pub(super) smt: Setting,
    /// Worker and coordinator placement.
    pub(super) placement: Setting,
    /// NUMA policy and observed state.
    pub(super) numa: Setting,
    /// Public operating-system/kernel version.
    pub(super) kernel: Setting,
    /// Distribution release fields, captured independently of the kernel version.
    pub(super) distribution: Setting,
    /// Container/VM limits with identifiers removed in public projection.
    pub(super) resource_limits: Setting,
    /// Frequency governor/power policy.
    pub(super) power: Setting,
    /// Turbo state.
    pub(super) turbo: Setting,
    /// Scoped tracing declaration; no subscriber is installed.
    pub(super) tracing: Setting,
    /// Concurrent workload declaration, explicitly unverified.
    pub(super) concurrent_load: Setting,
    /// Public core-class inventory; unavailable classes are not guessed.
    pub(super) core_classes: Setting,
    /// Observed coordinator CPU allowance, distinct from binding.
    pub(super) coordinator_placement: Setting,
    /// Observed worker placement, distinct from requested process constraints.
    pub(super) worker_placement: Setting,
    /// Observed process NUMA-node allowance, not page residency.
    pub(super) numa_allowed_nodes: Setting,
    /// Process resource limits without process identifiers.
    pub(super) process_limits: Setting,
    /// Resolved unified-cgroup CPU quota and period.
    pub(super) cgroup_cpu_limit: Setting,
    /// Resolved unified-cgroup memory limit.
    pub(super) cgroup_memory_limit: Setting,
    /// Resolved effective unified-cgroup CPU allowance.
    pub(super) cgroup_cpuset: Setting,
    /// Virtualization evidence without host or container identifiers.
    pub(super) virtual_machine: Setting,
    /// Observed CPU-zero minimum policy frequency in native kHz.
    pub(super) frequency_min: Setting,
    /// Observed CPU-zero maximum policy frequency in native kHz.
    pub(super) frequency_max: Setting,
}

/// Explicit aggregate capacity policy; no machine-derived defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResourcePolicy {
    /// Total admission duration from run entry.
    pub(super) budget_ns: u64,
    /// Maximum requested case count.
    pub(super) cases: u64,
    /// Maximum independent repetitions.
    pub(super) repetitions: u64,
    /// Total attempts, including controls, pilot and warm-up.
    pub(super) attempts: u64,
    /// Maximum iterations per row or warm-up group.
    pub(super) iterations_per_row: u64,
    /// Maximum named operations per item.
    pub(super) operations_per_item: u64,
    /// Aggregate input/output buffer payload cap.
    pub(super) buffer_bytes: u64,
    /// Aggregate scratch payload cap.
    pub(super) scratch_bytes: u64,
    /// Aggregate report payload cap.
    pub(super) report_bytes: u64,
    /// Active worker cap.
    pub(super) workers: u64,
    /// Stack reservation for each spawned worker.
    pub(super) worker_stack_bytes: u64,
}

/// Host-declared bounds; never inferred from pilot durations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "wire fields retain their explicit nanosecond units"
)]
pub(super) struct ExecutionBounds {
    /// Maximum uninterrupted outer execution interval U.
    pub(super) uninterrupted_ns: u64,
    /// Complete mandatory outer tail C, including every worker and report conversion.
    pub(super) finalization_ns: u64,
    /// Maximum uninterrupted item `U_item`, including diagnostic items.
    pub(super) item_ns: u64,
    /// Sampler-owned uninterrupted callback/checkpoint bound.
    pub(super) sampler_uninterrupted_ns: u64,
    /// Complete sampler finalization bound.
    pub(super) sampler_finalization_ns: u64,
    /// Complete receipt preservation cost.
    pub(super) receipt_ns: u64,
    /// Complete output disposal cost.
    pub(super) disposal_ns: u64,
    /// Complete adapter cleanup cost, including worker joins.
    pub(super) cleanup_ns: u64,
    /// Complete outer owned-report conversion cost.
    pub(super) report_ns: u64,
    /// Positive blocking receive interval between worker cancellation checkpoints.
    pub(super) wait_poll_ns: u64,
    /// Declared maximum cancellation read cost.
    pub(super) cancellation_read_ns: u64,
}

/// Raw object-format choice is independent of the rendering checkout.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum ObjectFormat {
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
pub(super) struct RawContentId {
    /// Algorithm chosen by the raw report.
    pub(super) algorithm: ObjectFormat,
    /// Full lowercase hexadecimal digest, excluding the algorithm prefix.
    pub(super) digest: String,
}

/// Shared recipe/estimator identity, with no pooled independent repetitions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunIdentity {
    /// Caller-assigned run slug.
    pub(super) run_id: String,
    /// Revision of input, workload and timer recipe.
    pub(super) recipe_revision: String,
    /// Actual matrix statistical source revision.
    pub(super) source_revision: String,
    /// Actual estimator and RNG extraction revision.
    pub(super) estimator_revision: String,
    /// Raw object format used for content binding.
    pub(super) object_format: ObjectFormat,
    /// Reviewed source attachment identity if the build was dirty.
    pub(super) source_attachment: Evidence<SourceAttachment>,
}

/// Fixed phase order in report durations, without per-observation metadata growth.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "wire fields retain their explicit nanosecond units"
)]
pub(super) struct PhaseDurations {
    /// Admission and schema/count validation.
    pub(super) validation_ns: u64,
    /// Input/worker preparation and dry validation.
    pub(super) preparation_ns: u64,
    /// Clock/empty control and length pilot.
    pub(super) pilot_ns: u64,
    /// Timed warm-up sum, distinct from wall preparation time.
    pub(super) warm_up_ns: u64,
    /// Timed measured item sum, including failed/partial items.
    pub(super) measurement_ns: u64,
    /// Observed worker release and owned-report finalization wall time.
    pub(super) finalization_ns: u64,
    /// Reset and dry-validation wall time outside item timers.
    pub(super) reset_ns: u64,
    /// Residual harness wall time, including unseparated sampler work.
    pub(super) harness_ns: u64,
}

/// Run-wide execution facts retained in both raw and summary projections.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Execution {
    /// Captured UTC run timestamp in RFC 3339 Z form.
    pub(super) timestamp: String,
    /// Actual public invocation arguments, scrubbed by public projection.
    pub(super) command: Vec<String>,
    /// Total wall duration including mandatory finalization; unavailable on clock-ledger failure.
    pub(super) elapsed_ns: Option<u64>,
    /// Observed excess over the admission budget; unavailable with total duration.
    pub(super) overshoot_ns: Option<u64>,
    /// Explicit outer execution assumptions, unavailable when unsupported.
    pub(super) bounds: Option<ExecutionBounds>,
    /// Fixed phase duration ledger.
    pub(super) phases: Option<PhaseDurations>,
    /// Every requested case/repetition completed with complete analysis and no run-wide failure.
    pub(super) complete: bool,
    /// Run-wide failure after admission; completed case evidence is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) stop_reason: Option<Reason>,
}

/// Typed non-success case reason, never a zero-rate observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum Reason {
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

/// Attempt partitions; outlier labels never reduce the measured count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Counts {
    /// All attempts, including pilot, warm-up and untimed failures.
    pub(super) attempted: u64,
    /// Valid timed items in retained complete measurement rows.
    pub(super) measured: u64,
    /// Attempted minus measured, including unfinished rows.
    pub(super) discarded: u64,
    /// Pilot and control attempts.
    pub(super) pilot: u64,
    /// Sampler warm-up attempts.
    pub(super) warm_up: u64,
    /// Sampler measurement attempts, including incomplete rows.
    pub(super) measurement: u64,
    /// Complete measurement rows.
    pub(super) rows: u64,
    /// Achieved resamples, zero without complete analysis.
    pub(super) resamples: u64,
}

/// Public recipe descriptor without raw numerical input arrays.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryRecipe {
    /// Fixed reset, cache, start-wait, construction and restoration identity.
    pub(super) protocol: CaseProtocol,
    /// Catalog operation.
    pub(super) operation: Operation,
    /// Source width.
    pub(super) width: Width,
    /// Dependency form.
    pub(super) dependency: Dependency,
    /// Timing scope.
    pub(super) scope: Scope,
    /// Fixed shape descriptor.
    pub(super) shape: Vec<u64>,
    /// Requested active workers.
    pub(super) workers: u64,
    /// Requested placement.
    pub(super) placement: Placement,
    /// JSON Pointer to original exact recipe inputs, bound to summary's raw digest.
    pub(super) exact_input_pointer: InputReference,
    /// Complete checked LC population, storage and work ledger.
    pub(super) lc_ledger: Option<LcLedger>,
    /// Backend packing evidence, including unavailable threshold getter routes.
    pub(super) packing: Option<PackingEvidence>,
}

/// Exact-input reference binds a canonical JSON Pointer to the canonical raw blob.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InputReference {
    /// Algorithm-qualified full raw content identity.
    pub(super) raw: RawContentId,
    /// Canonical JSON Pointer, with exact escaping and canonical array indices.
    pub(super) pointer: String,
}

/// Source-free time estimate projected onto its shared exact decimal grid.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Estimate {
    /// Decimal quantum in seconds.
    pub(super) grid_power: i32,
    /// Nearest point estimate.
    pub(super) point: String,
    /// Nearest bootstrap standard error.
    pub(super) standard_error: String,
    /// Outward lower confidence endpoint.
    pub(super) lower: String,
    /// Outward upper confidence endpoint.
    pub(super) upper: String,
}

/// Reciprocal rate without an invented reciprocal standard error.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RateEstimate {
    /// Explicit positive integer work numerator.
    pub(super) numerator: u64,
    /// Six-significant-figure nearest rate.
    pub(super) point: String,
    /// Six-significant-figure outward lower endpoint.
    pub(super) lower: Option<String>,
    /// Six-significant-figure outward upper endpoint.
    pub(super) upper: Option<String>,
    /// Nonpositive source interval endpoints have no reciprocal interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) interval_unavailable: Option<String>,
}

/// Native cgroup CPU accounting, without cgroup path or container identifiers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CpuCounters {
    /// Total CPU usage microseconds.
    pub(super) usage_usec: u64,
    /// Scheduling periods.
    pub(super) periods: Evidence<u64>,
    /// Throttled scheduling periods.
    pub(super) throttled_periods: Evidence<u64>,
    /// Total throttled microseconds.
    pub(super) throttled_usec: Evidence<u64>,
}

/// Fixed native-unit decimal telemetry with its retained exact source resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoadValue {
    /// Canonical decimal in the declared native unit.
    pub(super) value: String,
    /// Decimal quantum exponent.
    pub(super) grid_power: i32,
    /// Native source unit, never silently converted.
    pub(super) unit: String,
    /// Exact source resolution numerator.
    pub(super) resolution_numerator: u64,
    /// Positive source resolution denominator.
    pub(super) resolution_denominator: u64,
}

/// Fixed PSI projection, without its source decimal coefficients.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryPressure {
    /// Native percent averages for ten, sixty and three hundred seconds.
    pub(super) averages_percent: [LoadValue; 3],
    /// Native cumulative microseconds.
    pub(super) total_usec: u64,
}

/// Fixed-size load snapshot; no source history or private membership path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryLoad {
    /// Named hwmon-zero sensor; no assumption that it measures the CPU package.
    pub(super) thermal_sensor: Evidence<String>,
    /// Native signed millidegrees Celsius from the named sensor.
    pub(super) temperature_millidegrees: Evidence<i64>,
    /// CPU-zero scaling frequency in native integer kHz, never wall-time cycles.
    pub(super) scaling_frequency_khz: Evidence<u64>,
    /// CPU-zero frequency in native decimal MHz.
    pub(super) cpu_mhz: Evidence<LoadValue>,

    /// Capture offset from run entry.
    pub(super) offset_ns: u64,
    /// Native runnable-task averages.
    pub(super) load_tasks: Evidence<[LoadValue; 3]>,
    /// Runnable task population.
    pub(super) runnable_tasks: Evidence<u64>,
    /// Total task population.
    pub(super) total_tasks: Evidence<u64>,
    /// System some pressure.
    pub(super) system_some: Evidence<SummaryPressure>,
    /// System full pressure.
    pub(super) system_full: Evidence<SummaryPressure>,
    /// Applicable native cgroup counters, retained for interval reconstruction.
    pub(super) cpu: Evidence<CpuCounters>,
    /// Applicable some pressure.
    pub(super) cgroup_some: Evidence<SummaryPressure>,
    /// Applicable full pressure.
    pub(super) cgroup_full: Evidence<SummaryPressure>,
    /// Recorded allowed CPU population.
    pub(super) allowed_cpus: Evidence<u64>,
    /// Capture method without private paths.
    pub(super) method: String,
}

/// Exact interval fractions, unavailable when their retained sources do not suffice.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoadFractions {
    /// CPU usage divided by elapsed wall time and recorded allowed population.
    pub(super) utilization: Evidence<LoadValue>,
    /// Throttled periods divided by scheduling periods.
    pub(super) throttling: Evidence<LoadValue>,
}

/// Explicit pilot stop; a failed prefix never supplies estimates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum PilotStop {
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
    /// Explicit report reservation failed.
    Capacity,
}

/// Fixed arithmetic storage identity; coefficient/state arrays remain in the raw artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryArithmetic {
    /// Two alternating coefficient tuples per active state and lane.
    pub(super) cycle: u64,
    /// Logical active binary64 seed/state bytes, excluding compiler spills.
    pub(super) state_bytes: u64,
    /// Exact active coefficient-pair bytes, excluding compiler spills.
    pub(super) coefficient_bytes: u64,
    /// Digest-bound location of all exact active coefficient and seed bits.
    pub(super) exact_inputs: InputReference,
}

/// Fixed pilot diagnostics, independent of the number of attempted lengths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryPilot {
    /// Smallest positive observed monotonic tick.
    pub(super) observed_tick_ns: Option<u64>,
    /// Available platform-declared resolution.
    pub(super) declared_resolution_ns: Evidence<u64>,
    /// Exact fixed-grid control median in seconds, with ten fractional digits.
    pub(super) control_median_seconds: Option<String>,
    /// Largest retained control observation.
    pub(super) control_maximum_ns: Option<u64>,
    /// Exact accepted-workload median on the same fixed grid.
    pub(super) accepted_median_seconds: Option<String>,
    /// Largest retained observation at the accepted length.
    pub(super) accepted_maximum_ns: Option<u64>,
    /// Required timing target, absent before control completion.
    pub(super) target_ns: Option<u64>,
    /// Original typed pilot completion/refusal.
    pub(super) stop: PilotStop,
}

/// Fixed statistical summary; no observation arrays or source-statistic bits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryAnalysis {
    /// Mean, median, sample standard deviation, scaled MAD and slope.
    pub(super) estimates: [Estimate; 5],
    /// Rounded confidence; exact input lives at the declared raw pointer.
    pub(super) confidence: String,
    /// Lower outer, lower inner, upper inner, upper outer fence decimals.
    pub(super) fences: [String; 4],
    /// Shared fence decimal quantum in seconds.
    pub(super) fence_grid: i32,
    /// Fixed-category row partition, never per-row labels.
    pub(super) outlier_counts: [u64; 5],
    /// Aggregate items per second.
    pub(super) items_per_second: RateEstimate,
    /// Scalar elements processed per second.
    pub(super) elements_per_second: RateEstimate,
    /// Logical vector operations per second, absent for scalar sources.
    pub(super) vector_operations_per_second: Option<RateEstimate>,
    /// Conventional arithmetic operations per second where defined by the recipe.
    pub(super) floating_operations_per_second: Option<RateEstimate>,
    /// Logical operand bytes per second, absent for register arithmetic.
    pub(super) logical_bytes_per_second: Option<RateEstimate>,
}

/// Summary outcome has fixed shape independent of attempted observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "value", deny_unknown_fields)]
pub(super) enum SummaryOutcome {
    /// Complete requested schedule and bootstrap.
    Complete(Box<SummaryAnalysis>),
    /// Started but incomplete, with no complete estimates.
    Incomplete(Reason),
    /// Valid but unsupported case.
    Unsupported(Reason),
    /// Unvisited after a run-wide stop.
    NotStarted(Reason),
}

/// Exactly one case/repetition record, without any raw observation arrays.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryCase {
    /// Bounded startup storage and placement evidence for each worker.
    pub(super) worker_layouts: Vec<WorkerLayout>,
    /// Original ordered case index.
    pub(super) case_index: u64,
    /// Original independent repetition index.
    pub(super) repetition: u64,
    /// Full recipe seed identity.
    pub(super) seed: String,
    /// Effective input offset.
    pub(super) effective_offset: u64,
    /// Fixed public recipe descriptor or raw input reference.
    pub(super) recipe: SummaryRecipe,
    /// Accepted fixed per-worker item length.
    pub(super) accepted_length: Option<u64>,
    /// Complete attempted/measured/discarded partition.
    pub(super) counts: Counts,
    /// Fixed phase duration totals.
    pub(super) phases: PhaseDurations,
    /// Fixed arithmetic byte/cycle identity and raw exact coefficient reference.
    pub(super) arithmetic: Option<SummaryArithmetic>,
    /// Fixed diagnostic summaries; no chronological pilot arrays.
    pub(super) pilot: Option<SummaryPilot>,
    /// Public before-case load projection.
    pub(super) load_before: Option<SummaryLoad>,
    /// Public after-case load projection.
    pub(super) load_after: Option<SummaryLoad>,
    /// Exact counter-derived interval fractions, absent without both snapshots.
    pub(super) load_fractions: Option<LoadFractions>,
    /// Typed completion and analysis status.
    pub(super) outcome: SummaryOutcome,
}

/// Report-level request metadata cannot grow with case observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryRequest {
    /// Requested case count.
    pub(super) cases: u64,
    /// Requested independent repetition count.
    pub(super) repetitions: u64,
    /// Requested samples per case.
    pub(super) samples: u32,
    /// Requested resamples per complete case.
    pub(super) resamples: u32,
    /// Rounded confidence input; not an endpoint-valued sampler setting.
    pub(super) confidence: String,
    /// Canonical JSON Pointer to exact raw confidence bits.
    pub(super) confidence_input: InputReference,
    /// Explicit aggregate resource policy.
    pub(super) resources: ResourcePolicy,
}

/// Summary payload is a public projection from one exact raw report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryPayload {
    /// Caller-supplied full digest of canonical raw bytes.
    pub(super) raw: RawContentId,
    /// Exact raw schema version bound to that digest.
    pub(super) raw_version: [u64; 3],
    /// Recipe and estimator identity.
    pub(super) identity: RunIdentity,
    /// Public build projection.
    pub(super) build: BuildRecord,
    /// Public environment projection.
    pub(super) environment: EnvironmentRecord,
    /// Shared request settings and raw exact-input reference.
    pub(super) request: SummaryRequest,
    /// Public execution projection.
    pub(super) execution: Execution,
    /// Chronological bounded case metadata, never observation arrays.
    pub(super) cases: Vec<SummaryCase>,
}

/// Current summary envelope; repository admission is a separate policy decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryDataset {
    /// Current summary namespace.
    pub(super) namespace: String,
    /// Exact summary schema revision.
    pub(super) version: [u64; 3],
    /// Estate owner.
    pub(super) owner: String,
    /// Exported summary license.
    pub(super) license: String,
    /// Bounded source references.
    pub(super) sources: Vec<String>,
    /// Design labels.
    pub(super) labels: Vec<String>,
    /// Deterministic public projection.
    pub(super) payload: SummaryPayload,
}

/// Independent structural LC ledger component.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationCounts {
    /// Square roots.
    pub(super) roots: u64,
    /// Divisions, including reciprocal preparation.
    pub(super) divisions: u64,
    /// Separately rounded multiplications.
    pub(super) multiplications: u64,
    /// Separately rounded additions.
    pub(super) additions: u64,
    /// Fused multiply-add evaluations.
    pub(super) fmas: u64,
}

/// Independent structural LC ledger component.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LcLedger {
    /// Exact ordered populations of the caller's numerical arrays.
    pub(super) input_populations: Vec<u64>,
    /// Explicit inverse-query branch population, zero for other expressions.
    pub(super) cycle_population: u64,
    /// Primitive work in one shared preparation.
    pub(super) preparation: OperationCounts,
    /// Primitive work in one complete output traversal.
    pub(super) traversal: OperationCounts,
    /// Logical owned input payload, including integer cycles.
    pub(super) input_bytes: u64,
    /// Retained shared-frequency/reciprocal payload.
    pub(super) prepared_bytes: u64,
    /// Simultaneous per-axis root scratch, absent for non-forward expressions.
    pub(super) root_scratch_bytes: u64,
    /// Full consumed output population in bytes.
    pub(super) output_bytes: u64,
    /// Uses of each prepared root on the L and C axes, respectively.
    pub(super) root_reuse: [u64; 2],
    /// Output uses of each prepared reciprocal; zero when no reciprocal is prepared.
    pub(super) reciprocal_reuse: u64,
    /// Logical reads/writes in preparation, including output initialization.
    pub(super) preparation_bytes: u64,
    /// Logical reads/writes in one traversal, never physical bus traffic.
    pub(super) traversal_bytes: u64,
}

/// Independent structural packing capability record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PackingEvidence {
    /// Source-proven bypass path or explicit unavailable reason.
    pub(super) path: Evidence<String>,
    /// Packing bytes, never an inferred multiplier.
    pub(super) bytes: Evidence<u64>,
    /// Unavailable RHS threshold getter.
    pub(super) rhs_threshold: Evidence<u64>,
    /// Unavailable single-thread LHS threshold getter.
    pub(super) lhs_single_thread_threshold: Evidence<u64>,
}

/// Numerical storage role; no address or allocator identifier is exported.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum BufferRole {
    /// Arithmetic seed or coefficient state.
    State,
    /// Immutable numerical input.
    Input,
    /// Mutable consumed numerical output.
    Output,
    /// Shared prepared numerical values.
    Prepared,
    /// Explicit integer inverse-query cycles.
    Cycles,
    /// Backend index permutation storage.
    Permutation,
    /// Backend-declared byte scratch storage.
    Scratch,
}

/// Actual storage extent and address residue captured outside timing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BufferLayout {
    /// Stable role in the selected kernel.
    pub(super) role: BufferRole,
    /// Actual allocated payload capacity, not just the logical extent.
    pub(super) capacity_bytes: u64,
    /// Live logical payload bytes.
    pub(super) live_bytes: u64,
    /// Base address modulo 64, without exposing the address.
    pub(super) address_modulo_64: u64,
    /// Logical row stride in bytes.
    pub(super) row_stride_bytes: u64,
    /// Logical column stride in bytes, zero for a one-dimensional buffer.
    pub(super) column_stride_bytes: u64,
}

/// Page residence cannot be obtained from the safe standard library.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum NumaEvidence {
    /// The safe runtime has no physical page-residence query.
    UnavailableSafePageQuery,
}

/// Startup evidence is per worker and never claims execution remained on one CPU.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkerLayout {
    /// Worker index; zero is the direct caller for serial execution.
    pub(super) worker: u64,
    /// Sum of actual numerical buffer capacities, including scratch and prepared storage.
    pub(super) numerical_capacity_bytes: u64,
    /// Sum of live logical numerical payloads in those same buffers.
    pub(super) numerical_live_bytes: u64,
    /// Worker that allocated and initialized the numerical payload.
    pub(super) first_touch_worker: u64,
    /// Snapshot of the CPU reported by procfs, absent if the bounded probe failed.
    pub(super) startup_cpu: Option<u64>,
    /// Observed thread-local CPU allowance, distinct from binding.
    pub(super) allowed_cpus: Allowance,
    /// Observed thread-local NUMA-node allowance, distinct from page residence.
    pub(super) allowed_nodes: Allowance,
    /// Explicit inability to infer physical page residence from first touch.
    pub(super) numa: NumaEvidence,
    /// Ordered fixed storage slots; unused slots are null.
    pub(super) buffers: [Option<BufferLayout>; 8],
}

/// Reset identity, including dry validation outside item timing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum ResetPolicy {
    /// Reload fixed runtime seed state at each invocation.
    ReloadRuntimeSeeds,
    /// Clear mutable output outside each item timer.
    ZeroOutput,
    /// Restore matrix operands or outputs from their retained source.
    RestoreMatrixSource,
}

/// No cache flush or cold-cache claim is made by these recipes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum CachePolicy {
    /// Reuse allocated storage without flushing the cache.
    NoFlushWarmReuse,
}

/// The parallel item timer includes coordinator channel dispatch and collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum StartWaitPolicy {
    /// Execute on the calling thread without a channel protocol.
    DirectCall,
    /// Include generation-tagged worker dispatch and completion waits.
    BlockingChannel,
}

/// Matrix construction identity is distinct from its shape and observed performance.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum Conditioning {
    /// Use centered finite seeded entries without a condition estimate.
    SeededCentered,
    /// Replace each diagonal by its ascending off-diagonal absolute sum plus margin.
    RowAbsoluteSumPlusPositiveMargin,
}

/// Owned numerical allocation strategy, separate from physical page residence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum AllocationStrategy {
    /// Keep fixed arithmetic seed and coefficient arrays in one owned box.
    BoxedFixedState,
    /// Reserve each contiguous numerical vector fallibly.
    FallibleContiguousVectors,
    /// Keep numerical storage inside the matrix-owned raw handle.
    MatrixOwnedBuffers,
}

/// Fixed streaming summation order includes the scalar tail before final folding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum ReadOrder {
    /// Update eight vectors in order, add the scalar tail to matching partials, then fold.
    AccumulatorMajorLaneMinorTailThenFold,
}

/// Matrix seeded storage and diagonal construction order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum MatrixConstruction {
    /// Generate centered entries in column-major order.
    ColumnMajorSeed,
    /// Generate column-major entries, then form row diagonals in ascending column order.
    ColumnMajorSeedAscendingRowSums,
}

/// A structure guarantee is not a computed condition-number estimate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum ConditionEstimate {
    /// No condition-number estimate was computed.
    UnknownNotEstimated,
}

/// Timed restoration occurs at a stated boundary within the repeated item.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum RestorationSchedule {
    /// No copy between prepared traversals is timed.
    None,
    /// Restore before each traversal, including the first.
    BeforeEveryTraversal,
    /// Restore only between consecutive traversals.
    BetweenTraversals,
}

/// Fixed case protocol and preparation identity, independent of observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseProtocol {
    /// Restoring inputs happens outside each item except the explicit repeated-factor scope.
    pub(super) reset: ResetPolicy,
    /// Selected cache treatment.
    pub(super) cache: CachePolicy,
    /// Coordinator/worker start and completion protocol.
    pub(super) start_wait: StartWaitPolicy,
    /// Logical read partial sums; absent for other kernels.
    pub(super) read_partials: Option<u64>,
    /// Scalar read tail elements after complete eight-vector chunks.
    pub(super) read_tail: Option<u64>,
    /// Exact matrix construction recipe; margin is in the bound raw case inputs.
    pub(super) conditioning: Option<Conditioning>,
    /// Source elements per timed restoration; schedule determines its repetition count.
    pub(super) timed_restoration_elements: u64,
    /// Numerical allocation family.
    pub(super) allocation: AllocationStrategy,
    /// Fixed read reduction ordering, absent for other families.
    pub(super) read_order: Option<ReadOrder>,
    /// Explicit matrix generation order, absent for other families.
    pub(super) matrix_construction: Option<MatrixConstruction>,
    /// Explicit lack of a computed condition-number estimate.
    pub(super) condition_estimate: Option<ConditionEstimate>,
    /// Where each restoration occurs relative to repeated traversals.
    pub(super) restoration_schedule: RestorationSchedule,
    /// Copied source bytes for one timed restoration.
    pub(super) restoration_copied_bytes: u64,
    /// Logical read plus write traffic for one timed restoration.
    pub(super) restoration_logical_bytes: u64,
}

/// Reviewed sanitized source attachment; admission verifies the repository bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceAttachment {
    /// Reviewed repository-relative patch path, never a live URL.
    pub(super) path: String,
    /// Full content identity of the exact reviewed patch bytes.
    pub(super) blob: RawContentId,
}

/// Bounded observed affinity allowance; a mask never claims an exact binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", deny_unknown_fields)]
pub(super) enum Allowance {
    /// Least-significant word first; 32 words cover processor/node indices 0 through 1023.
    Known([u32; 32]),
    /// The bounded thread-local procfs probe was absent, denied or malformed.
    UnavailableProbe,
    /// The kernel mask exceeded the explicit representational extent.
    UnsupportedRange,
}
