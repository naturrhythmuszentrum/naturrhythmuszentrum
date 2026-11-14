// SPDX-License-Identifier: AGPL-3.0-only
//! Write an explicit small collector request; this example performs no measurement.

use nrz_compute::{
    Binary64, CaseRequest, Dependency, ExecutionBounds, Operation, Placement, ResourcePolicy,
    RunRequest, SamplingPolicy, Scope, Seed, Width,
};
use std::{env, fs::File, io::Write};

/// Emit an explicit arithmetic observation recipe to a caller-selected path.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args_os()
        .nth(1)
        .ok_or("supply an external request output path")?;
    let case = CaseRequest {
        operation: Operation::Add,
        width: Width::Simd2,
        dependency: Dependency::Independent,
        scope: Scope::Prepared,
        shape: Vec::new(),
        item_length: 1024,
        workers: 1,
        placement: Placement::Unbound,
        allowed_cpus: Vec::new(),
        budget_ns: 30_000_000_000,
        diagonal_margin: None,
        inputs: Vec::new(),
        cycles: Vec::new(),
    };
    let request = RunRequest {
        version: [1, 0, 0],
        run_id: "node-smoke".to_owned(),
        cases: vec![case],
        seeds: vec![Seed(1)],
        sampling: SamplingPolicy {
            samples: 10,
            resamples: 1000,
            confidence: Binary64(0.95_f64.to_bits()),
            warm_up_ns: 10_000_000,
            measurement_ns: 50_000_000,
            analysis_report_bytes: 1_048_576,
            analysis_scratch_bytes: 4_194_304,
        },
        resources: ResourcePolicy {
            budget_ns: 60_000_000_000,
            cases: 1,
            repetitions: 1,
            attempts: 8192,
            iterations_per_row: 1024,
            operations_per_item: 16_777_216,
            buffer_bytes: 8_388_608,
            scratch_bytes: 16_777_216,
            report_bytes: 33_554_432,
            workers: 1,
            worker_stack_bytes: 2_097_152,
        },
    };
    let bounds = ExecutionBounds {
        uninterrupted_ns: 2_000_000_000,
        finalization_ns: 5_000_000_000,
        item_ns: 500_000_000,
        sampler_uninterrupted_ns: 1_000_000_000,
        sampler_finalization_ns: 1_000_000_000,
        receipt_ns: 100_000_000,
        disposal_ns: 100_000_000,
        cleanup_ns: 1_000_000_000,
        report_ns: 1_000_000_000,
        wait_poll_ns: 1_000_000,
        cancellation_read_ns: 1_000_000,
    };
    let value = serde_json::json!({"request":request,"bounds":bounds,"machine_alias":"node-observation",
        "object_format":"sha1","export_bytes":67_108_864_u64,"export_ns":30_000_000_000_u64});
    let mut file = File::create(path)?;
    serde_json::to_writer(&mut file, &value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
