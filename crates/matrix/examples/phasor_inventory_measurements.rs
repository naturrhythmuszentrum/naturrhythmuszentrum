// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone native LC solve and publication measurements use the public physical fixture.

use nrz_matrix as test_api;

#[path = "../src/testing/readings.rs"]
#[allow(
    dead_code,
    reason = "measurement uses the successful subset of shared acceptance support"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared fixture declarations remain private to this example"
)]
mod reading_testing;

#[path = "../src/testing/phasor.rs"]
#[allow(
    dead_code,
    reason = "measurement uses the successful subset of shared acceptance support"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared fixture declarations remain private to this example"
)]
mod phasor_testing;

#[path = "../src/testing/phasor_solve.rs"]
#[allow(
    dead_code,
    reason = "measurement uses the successful subset of shared acceptance support"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared fixture declarations remain private to this example"
)]
mod phasor_solve_testing;

use nrz_scalar::{ArithmeticContext, Scalar};
use std::io::{self, Write};

/// One process owns one fixture; only the requested native operation is clocked.
fn main() -> reading_testing::TestResult {
    let mode = std::env::args()
        .nth(1)
        .ok_or("expected evaluate32, publish32, evaluate64 or publish64")?;
    match mode.as_str() {
        "evaluate32" => measure::<f32>(false),
        "publish32" => measure::<f32>(true),
        "evaluate64" => measure::<f64>(false),
        "publish64" => measure::<f64>(true),
        _ => Err("expected evaluate32, publish32, evaluate64 or publish64".into()),
    }
}

/// Fixture declarations and publication planning are excluded from the native operation clock.
fn measure<P: Scalar + 'static>(publish: bool) -> reading_testing::TestResult {
    let mut case = phasor_solve_testing::case(
        -P::from_u8(75),
        P::ZERO,
        false,
        &mut phasor_solve_testing::budget()?,
    )?;
    let plan = case.publication(&mut reading_testing::ledger()?)?;
    let context = ArithmeticContext::new()?;
    let mut budget = reading_testing::ledger()?;
    let start = std::time::Instant::now();
    let result = if publish {
        case.heap
            .publish_phasor_solve(&plan, &context, &mut budget)?
            .evaluation()
            .clone()
    } else {
        case.heap
            .evaluate_phasor_solve(plan.solve(), &context, &mut budget)?
    };
    let seconds = start.elapsed().as_secs_f64();
    let usage = result.usage();
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "owner=native-phasor precision={:?} publication={publish} lowering_work={} numerical_work={} operation_work={} seconds={seconds:.9}",
        P::PRECISION,
        usage[1].work - usage[0].work,
        usage[2].work - usage[1].work,
        budget.usage().work
    )?;
    write_inventory(&mut output, result.inventory())?;
    output.flush()?;
    Ok(())
}

/// Report fixed owners and vector capacities separately from whole-process RSS and shared definitions.
fn write_inventory<P: Scalar>(
    output: &mut impl Write,
    inventory: &nrz_matrix::PhasorInventory<P>,
) -> reading_testing::TestResult {
    let mut bytes = 0;
    let mut floats = 0;
    for allocation in inventory.allocations() {
        let stage = allocation.stage()?;
        let owned = allocation.owned_bytes();
        let entries = allocation.float_entries()?;
        bytes += owned;
        floats += entries;
        writeln!(
            output,
            "stage={stage:?} vector={} logical_entries={} float_entries={entries} owned_bytes={owned}",
            stage.is_vector(),
            allocation.entries()?
        )?;
    }
    writeln!(
        output,
        "scope=native-phasor allocations={} float_entries={floats} cumulative_owner_bytes={bytes} peak_payload_bytes={} planning_bytes={} metadata_bytes={} backend_allowance=not-applicable",
        inventory.allocations().count(),
        inventory.peak_payload_bytes(),
        inventory.planning_bytes(),
        inventory.metadata_bytes()
    )?;
    Ok(())
}
