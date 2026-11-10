// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit host policy, exports and deterministic coexistence checks.

mod adapter;
mod fixture;
mod report;
mod runner;

use adapter::Native;
use nrz_matrix::sampling::{
    AnalysisCaps, AnalysisSettings, Representative, SamplerBounds, SamplingIdentity,
    SamplingReport, SamplingSettings, sample,
};
use std::{fmt::Debug, io::Write, time::Duration};

/// Run the prepared comparison through the executable's single entry point.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the benchmark entry point is visible only to its executable"
)]
pub(super) fn run() -> BenchResult {
    runner::run()
}

/// Preserve original fixture, sampling and output errors without panicking.
type BenchResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Fixed seed and minimum linear schedule make deterministic receipts comparable.
fn settings() -> BenchResult<SamplingSettings> {
    let caps = AnalysisCaps {
        report_bytes: 2_000_000,
        scratch_bytes: 1_000_000,
    };
    Ok(SamplingSettings {
        analysis: AnalysisSettings::new(
            100,
            2_000,
            0.95,
            901,
            AnalysisCaps {
                report_bytes: 65_536,
                scratch_bytes: 1_000_000,
            },
        )?,
        warm_up: Duration::from_nanos(1),
        measurement: Duration::from_nanos(1),
        wall_budget: Duration::from_secs(2),
        max_total_iterations: 5_051,
        max_iterations_per_sample: 100,
        caps,
        bounds: SamplerBounds {
            uninterrupted: Duration::from_secs(1),
            finalization: Duration::from_secs(1),
        },
    })
}

/// Phase markers are explicit host output, outside the sampler invocation.
/// A syscall diagnostic can inspect the interval between these flushed markers.
fn observe<O, E>(
    name: &'static str,
    phase: &str,
    operation: impl FnMut() -> Result<O, E>,
) -> BenchResult<SamplingReport<'static, u64, E>>
where
    E: Copy + Debug,
{
    let representative = Representative {
        item: &(),
        elements_per_item: Some(1),
        identity: SamplingIdentity {
            representative: name,
            shape: &[1],
            operation_revision: "native-bench-v1",
            workload: name,
            scope: "native operation only; zero exact Work",
            trace_configuration: "no subscriber installed",
        },
    };
    let policy = settings()?;
    let mut adapter = Native { operation };
    let mut output = std::io::stdout().lock();
    writeln!(output, "HOST-SAMPLE-BEGIN {name} {phase}")?;
    output.flush()?;
    let report = sample(&(), &representative, &mut adapter, policy);
    writeln!(output, "HOST-SAMPLE-END {name} {phase}")?;
    output.flush()?;
    Ok(report?)
}
