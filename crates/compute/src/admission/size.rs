// SPDX-License-Identifier: AGPL-3.0-only
//! Recursive current-schema encoded size bound, independent of observation counts.

use super::{AdmissionError, AdmissionPolicy};

/// Maximum empty-report and per-case bytes under explicit metadata policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SizeBound {
    /// Envelope and shared metadata with an empty case array, including newline.
    pub header: u64,
    /// Largest complete or non-success case representation.
    pub case: u64,
}

impl SizeBound {
    /// Bound N independent case/repetition records, including separating commas.
    /// # Errors
    /// Refuses checked byte-count overflow.
    pub fn report(self, cases: u64) -> Result<u64, AdmissionError> {
        self.case
            .checked_mul(cases)
            .and_then(|n| n.checked_add(self.header))
            .and_then(|n| n.checked_add(cases.saturating_sub(1)))
            .ok_or(AdmissionError::Capacity("schema size"))
    }
}

/// Recompute literal keys, largest variants and bounded list/string contributions.
/// # Errors
/// Refuses missing representational space or checked byte-count overflow.
pub fn schema_size(policy: AdmissionPolicy) -> Result<SizeBound, AdmissionError> {
    if policy.metadata_string_bytes < 2 {
        return Err(AdmissionError::Capacity("metadata string"));
    }
    Ok(SizeBound {
        header: sum(&[summary_dataset(policy)?, 1])?,
        case: summary_case(policy)?,
    })
}

/// Checked recursive addition, with no allocated schema tree.
fn sum(values: &[u64]) -> Result<u64, AdmissionError> {
    values
        .iter()
        .try_fold(0_u64, |n, value| n.checked_add(*value))
        .ok_or(AdmissionError::Capacity("schema size"))
}

/// Empty or maximal list punctuation and payload.
fn list(item: u64, count: u64) -> Result<u64, AdmissionError> {
    let items = item
        .checked_mul(count)
        .ok_or(AdmissionError::Capacity("schema size"))?;
    sum(&[2, items, count.saturating_sub(1)])
}

/// Largest evidence object, including a bounded reason or the known value.
fn evidence(known: u64, policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    sum(&[21, 13, known.max(policy.metadata_string_bytes)])
}

/// Largest tagged completion variant; the alternatives are not added together.
fn outcome(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    sum(&[22, 12, summary_analysis(policy)?.max(32)])
}

/// Structured inspection evidence, with independently bounded symbol metadata.
fn lowering_record(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    sum(&[
        108,
        evidence(5, policy)?,
        evidence(raw_content_id(policy)?, policy)?,
        evidence(
            list(policy.metadata_string_bytes, policy.metadata_list_items)?,
            policy,
        )?,
        evidence(policy.metadata_string_bytes, policy)?,
        evidence(policy.metadata_string_bytes, policy)?,
        evidence(policy.metadata_string_bytes, policy)?,
        evidence(5, policy)?,
        evidence(policy.metadata_string_bytes, policy)?,
    ])
}

/// Literal fields and maximum encodings of `BuildRecord`.
fn build_record(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        142,
        evidence(policy.metadata_string_bytes, policy)?, // compiler
        evidence(policy.metadata_string_bytes, policy)?, // source_commit
        evidence(5, policy)?,                            // dirty
        evidence(policy.metadata_string_bytes, policy)?, // lockfile
        evidence(policy.metadata_string_bytes, policy)?, // target
        evidence(policy.metadata_string_bytes, policy)?, // profile
        evidence(policy.metadata_string_bytes, policy)?, // flags
        evidence(17, policy)?,                           // intent: largest quoted fixed enum
        list(policy.metadata_string_bytes, policy.metadata_list_items)?, // features
        evidence(policy.metadata_string_bytes, policy)?, // backend
        evidence(
            list(policy.metadata_string_bytes, policy.metadata_list_items)?,
            policy,
        )?, // dependencies
        lowering_record(policy)?,                        // lowering
    ])
}

/// Literal fields and maximum encodings of `Setting`.
fn setting(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        49,
        evidence(policy.metadata_string_bytes, policy)?, // requested
        evidence(policy.metadata_string_bytes, policy)?, // observed
        policy.metadata_string_bytes,                    // method
        16,                                              // offset_ns
    ])
}

/// Literal fields and maximum encodings of `EnvironmentRecord`.
fn environment_record(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        428,
        policy.metadata_string_bytes,                    // machine_alias
        evidence(policy.metadata_string_bytes, policy)?, // cpu_model
        list(policy.metadata_string_bytes, policy.metadata_list_items)?, // runtime_isa
        list(policy.metadata_string_bytes, policy.metadata_list_items)?, // compile_isa
        setting(policy)?,                                // topology
        setting(policy)?,                                // caches
        setting(policy)?,                                // memory
        setting(policy)?,                                // smt
        setting(policy)?,                                // placement
        setting(policy)?,                                // numa
        setting(policy)?,                                // distribution
        setting(policy)?,                                // kernel
        setting(policy)?,                                // resource_limits
        setting(policy)?,                                // power
        setting(policy)?,                                // turbo
        setting(policy)?,                                // tracing
        setting(policy)?,                                // concurrent_load
        setting(policy)?,                                // core_classes
        setting(policy)?,                                // coordinator_placement
        setting(policy)?,                                // worker_placement
        setting(policy)?,                                // numa_allowed_nodes
        setting(policy)?,                                // process_limits
        setting(policy)?,                                // cgroup_cpu_limit
        setting(policy)?,                                // cgroup_memory_limit
        setting(policy)?,                                // cgroup_cpuset
        setting(policy)?,                                // virtual_machine
        setting(policy)?,                                // frequency_min
        setting(policy)?,                                // frequency_max
    ])
}

/// Literal fields and maximum encodings of `ResourcePolicy`.
fn resource_policy(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        177, 16, // budget_ns
        16, // cases
        16, // repetitions
        16, // attempts
        16, // iterations_per_row
        16, // operations_per_item
        16, // buffer_bytes
        16, // scratch_bytes
        16, // report_bytes
        16, // workers
        16, // worker_stack_bytes
    ])
}

/// Literal fields and maximum encodings of `ExecutionBounds`.
fn execution_bounds(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        202, 16, // uninterrupted_ns
        16, // finalization_ns
        16, // item_ns
        16, // sampler_uninterrupted_ns
        16, // sampler_finalization_ns
        16, // receipt_ns
        16, // disposal_ns
        16, // cleanup_ns
        16, // report_ns
        16, // wait_poll_ns
        16, // cancellation_read_ns
    ])
}

/// Literal fields and maximum encodings of `RawContentId`.
fn raw_content_id(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        24,
        8,                            // algorithm
        policy.metadata_string_bytes, // digest
    ])
}

/// Literal fields and maximum encodings of `RunIdentity`.
fn run_identity(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        109,
        policy.metadata_string_bytes, // run_id
        policy.metadata_string_bytes, // recipe_revision
        policy.metadata_string_bytes, // source_revision
        policy.metadata_string_bytes, // estimator_revision
        8,                            // object_format
        evidence(
            sum(&[17, policy.metadata_string_bytes, raw_content_id(policy)?])?,
            policy,
        )?, // source_attachment
    ])
}

/// Literal fields and maximum encodings of `PhaseDurations`.
fn phase_durations(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        125, 16, // validation_ns
        16, // preparation_ns
        16, // pilot_ns
        16, // warm_up_ns
        16, // measurement_ns
        16, // finalization_ns
        16, // reset_ns
        16, // harness_ns
    ])
}

/// Literal fields and maximum encodings of `Execution`.
fn execution(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        87,
        policy.metadata_string_bytes, // timestamp
        list(policy.metadata_string_bytes, policy.metadata_list_items)?, // command
        16,                           // elapsed_ns
        16,                           // overshoot_ns
        (execution_bounds(policy)?).max(4), // bounds
        phase_durations(policy)?,     // phases
        5,                            // complete
        40,                           // optional run stop reason
    ])
}

/// Literal fields and maximum encodings of `Counts`.
fn counts(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        95, 16, // attempted
        16, // measured
        16, // discarded
        16, // pilot
        16, // warm_up
        16, // measurement
        16, // rows
        16, // resamples
    ])
}

/// Packing capability evidence never invents inaccessible tuning values.
fn packing_evidence(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    sum(&[
        66,
        evidence(policy.metadata_string_bytes, policy)?,
        evidence(16, policy)?,
        evidence(16, policy)?,
        evidence(16, policy)?,
    ])
}

/// Fixed LC ledger fields, bounded populations and two five-counter operation records.
fn lc_ledger(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    sum(&[
        215,
        list(16, policy.metadata_list_items.min(4))?,
        8 * 16,
        2 * (63 + 5 * 16),
        2 + 2 * 16 + 1,
    ])
}

/// Literal fields and maximum encodings of `SummaryRecipe`.
fn summary_recipe(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        138,
        575,                                   // protocol
        17,                                    // operation
        25,                                    // width
        27,                                    // dependency
        30,                                    // scope
        list(16, policy.metadata_list_items)?, // shape
        16,                                    // workers
        23,                                    // placement
        input_reference(policy)?,              // exact_input_pointer
        lc_ledger(policy)?,                    // lc_ledger, including null
        packing_evidence(policy)?,             // packing, including null
    ])
}

/// Literal fields and maximum encodings of `InputReference`.
fn input_reference(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        19,
        raw_content_id(policy)?,      // raw
        policy.metadata_string_bytes, // pointer
    ])
}

/// Literal fields and maximum encodings of `Estimate`.
fn estimate(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        60, 17,  // grid_power
        639, // point
        639, // standard_error
        639, // lower
        639, // upper
    ])
}

/// Literal fields and maximum encodings of `RateEstimate`.
fn rate_estimate(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        41, 16, // numerator
        15, // point
        15, // lower
        15, // upper
        57, // optional NonpositiveTimeEndpoint reason
    ])
}

/// Literal fields and maximum encodings of `CpuCounters`.
fn cpu_counters(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        65,
        16,                    // usage_usec
        evidence(16, policy)?, // periods
        evidence(16, policy)?, // throttled_periods
        evidence(16, policy)?, // throttled_usec
    ])
}

/// Literal fields and maximum encodings of `LoadValue`.
fn load_value(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        82,
        639,                          // value
        17,                           // grid_power
        policy.metadata_string_bytes, // unit
        16,                           // resolution_numerator
        16,                           // resolution_denominator
    ])
}

/// Literal fields and maximum encodings of `SummaryPressure`.
fn summary_pressure(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        35,
        list(load_value(policy)?, 3)?, // averages_percent
        16,                            // total_usec
    ])
}

/// Literal fields and maximum encodings of `SummaryLoad`.
fn summary_load(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        236,
        evidence(policy.metadata_string_bytes, policy)?, // thermal_sensor
        evidence(17, policy)?,                           // temperature_millidegrees
        evidence(16, policy)?,                           // scaling_frequency_khz
        evidence(load_value(policy)?, policy)?,          // cpu_mhz
        16,                                              // offset_ns
        evidence(list(load_value(policy)?, 3)?, policy)?, // load_tasks
        evidence(16, policy)?,                           // runnable_tasks
        evidence(16, policy)?,                           // total_tasks
        evidence(summary_pressure(policy)?, policy)?,    // system_some
        evidence(summary_pressure(policy)?, policy)?,    // system_full
        evidence(cpu_counters(policy)?, policy)?,        // cpu
        evidence(summary_pressure(policy)?, policy)?,    // cgroup_some
        evidence(summary_pressure(policy)?, policy)?,    // cgroup_full
        evidence(16, policy)?,                           // allowed_cpus
        policy.metadata_string_bytes,                    // method
    ])
}

/// Literal fields and maximum encodings of `LoadFractions`.
fn load_fractions(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        30,
        evidence(load_value(policy)?, policy)?, // utilization
        evidence(load_value(policy)?, policy)?, // throttling
    ])
}

/// Literal fields and maximum encodings of `SummaryArithmetic`.
fn summary_arithmetic(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        62,
        16,                       // cycle
        16,                       // state_bytes
        16,                       // coefficient_bytes
        input_reference(policy)?, // exact_inputs
    ])
}

/// Literal fields and maximum encodings of `SummaryPilot`.
fn summary_pilot(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        166,
        16,                    // observed_tick_ns
        evidence(16, policy)?, // declared_resolution_ns
        639,                   // control_median_seconds
        16,                    // control_maximum_ns
        639,                   // accepted_median_seconds
        16,                    // accepted_maximum_ns
        16,                    // target_ns
        18,                    // stop
    ])
}

/// Literal fields and maximum encodings of `SummaryAnalysis`.
fn summary_analysis(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        207,
        list(estimate(policy)?, 5)?,     // estimates
        10,                              // confidence
        list(639, 4)?,                   // fences
        17,                              // fence_grid
        list(16, 5)?,                    // outlier_counts
        rate_estimate(policy)?,          // items_per_second
        rate_estimate(policy)?,          // elements_per_second
        (rate_estimate(policy)?).max(4), // vector_operations_per_second
        (rate_estimate(policy)?).max(4), // floating_operations_per_second
        (rate_estimate(policy)?).max(4), // logical_bytes_per_second
    ])
}

/// Fixed eight-slot worker storage evidence, bounded by the metadata list allowance.
fn worker_layout(_policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let buffer = sum(&[105, 13, 5 * 16])?;
    sum(&[152, 5 * 16, 26, list(buffer, 8)?, 2 * (26 + list(10, 32)?)])
}

/// Literal fields and maximum encodings of `SummaryCase`.
fn summary_case(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        205,
        list(worker_layout(policy)?, policy.metadata_list_items)?,
        16,                                   // case_index
        16,                                   // repetition
        34,                                   // seed
        16,                                   // effective_offset
        summary_recipe(policy)?,              // recipe
        16,                                   // accepted_length
        counts(policy)?,                      // counts
        phase_durations(policy)?,             // phases
        (summary_arithmetic(policy)?).max(4), // arithmetic
        (summary_pilot(policy)?).max(4),      // pilot
        (summary_load(policy)?).max(4),       // load_before
        (summary_load(policy)?).max(4),       // load_after
        (load_fractions(policy)?).max(4),     // load_fractions
        outcome(policy)?,                     // outcome
    ])
}

/// Literal fields and maximum encodings of `SummaryRequest`.
fn summary_request(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        96,
        16,                       // cases
        16,                       // repetitions
        16,                       // samples
        16,                       // resamples
        10,                       // confidence
        input_reference(policy)?, // confidence_input
        resource_policy(policy)?, // resources
    ])
}

/// Literal fields and maximum encodings of `SummaryPayload`.
fn summary_payload(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        92,
        raw_content_id(policy)?,     // raw
        list(16, 3)?,                // raw_version
        run_identity(policy)?,       // identity
        build_record(policy)?,       // build
        environment_record(policy)?, // environment
        summary_request(policy)?,    // request
        execution(policy)?,          // execution
        2,                           // cases
    ])
}

/// Literal fields and maximum encodings of `SummaryDataset`.
fn summary_dataset(policy: AdmissionPolicy) -> Result<u64, AdmissionError> {
    let _ = policy;
    sum(&[
        77,
        policy.metadata_string_bytes, // namespace
        list(16, 3)?,                 // version
        policy.metadata_string_bytes, // owner
        policy.metadata_string_bytes, // license
        list(policy.metadata_string_bytes, policy.source_references)?, // sources
        list(policy.metadata_string_bytes, policy.metadata_list_items)?, // labels
        summary_payload(policy)?,     // payload
    ])
}
