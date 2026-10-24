// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone statistics measurements separate captured owners, execution clocks and backend reservations.

use nrz_matrix as test_api;

#[path = "../src/testing/conversion.rs"]
#[allow(
    dead_code,
    reason = "measurement uses only successful fixture declarations"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "fixture support is private to this example"
)]
mod conversion_testing;
#[path = "../src/testing/kernel_oracle.rs"]
#[allow(
    dead_code,
    reason = "shared statistical fixtures retain their independent oracle helpers"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "fixture support is private to this example"
)]
mod kernel_oracle;
#[path = "../src/testing/kernels.rs"]
#[allow(
    dead_code,
    reason = "measurement uses only successful fixture declarations"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "fixture support is private to this example"
)]
mod kernel_testing;
#[path = "../src/testing/statistics_oracle.rs"]
#[allow(
    dead_code,
    reason = "shared statistical fixtures retain their independent oracle helpers"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "fixture support is private to this example"
)]
mod statistics_oracle;
#[path = "../src/testing/statistics.rs"]
#[allow(
    dead_code,
    reason = "measurement uses only successful statistical declarations"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "fixture support is private to this example"
)]
mod statistics_testing;

use kernel_testing::{TestResult, axis, execute, output, owner, shape};
use nrz_matrix::{
    Binding, CheckedOperation, ExecutionLimits, Handle, Heap, HeapLimits, JointLimits,
    OperationDraft, StatisticsInventory, Value,
};
use nrz_scalar::Scalar;
use std::{
    io::{self, Write},
    time::Instant,
};

/// One process measures one path and precision; fixture construction is outside every reported clock.
fn main() -> TestResult {
    let selector = std::env::args()
        .nth(1)
        .ok_or("expected copy32, reload32, cross32, copy64, reload64 or cross64")?;
    match selector.as_str() {
        "copy32" => measure::<f32>("copy"),
        "reload32" => measure::<f32>("reload"),
        "cross32" => measure::<f32>("cross"),
        "copy64" => measure::<f64>("copy"),
        "reload64" => measure::<f64>("reload"),
        "cross64" => measure::<f64>("cross"),
        _ => Err("unknown statistical measurement selector".into()),
    }
}
/// Capture, numerical evaluation and atomic publication have independent transaction clocks.
fn measure<P: Scalar + 'static>(kind: &str) -> TestResult {
    let declarations = owner()?;
    let rows = axis(2)?;
    let column = axis(1)?;
    let contract = shape(&declarations, &column, &rows)?;
    let draft = OperationDraft::copy(vec![contract]);
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = statistics_testing::vector::<P>(&mut heap, true)?;
    let first = if kind == "copy" {
        source
    } else {
        output(&execute(&mut heap, draft.clone(), vec![source.value()])?)?
    };
    let (inventory, work, seconds) = if kind == "cross" {
        let second = output(&execute(&mut heap, draft, vec![first.value()])?)?;
        let start = Instant::now();
        let result = heap.materialize_cross_covariance(
            first.value(),
            second.value(),
            JointLimits::default(),
        )?;
        let elapsed = start.elapsed().as_secs_f64();
        let inventory = heap.moments(result)?.allocation_inventory().clone();
        let work = inventory.backend_allowance_bytes() / size_of::<P>();
        (inventory, work, [0.0, elapsed, 0.0])
    } else {
        transaction(
            &mut heap,
            draft.check(ExecutionLimits::default())?,
            first.value(),
        )?
    };
    let mut writer = io::BufWriter::new(io::stdout().lock());
    writeln!(
        writer,
        "owner=general-statistics kind={kind} precision={:?} statistical_work={work} capture_seconds={:.9} evaluate_seconds={:.9} publish_seconds={:.9} operation_seconds={:.9}",
        P::PRECISION,
        seconds[0],
        seconds[1],
        seconds[2],
        seconds.iter().sum::<f64>()
    )?;
    write_inventory(&mut writer, &inventory)?;
    writer.flush()?;
    Ok(())
}
/// Public binding capture includes metadata admission and the selected parameter/operating-point reads.
fn transaction<P: Scalar + 'static>(
    heap: &mut Heap,
    operation: CheckedOperation,
    value: Handle<Value, P>,
) -> TestResult<(StatisticsInventory, usize, [f64; 3])> {
    let start = Instant::now();
    let bound = Binding::new(operation, vec![value], ExecutionLimits::default()).validate(heap)?;
    let captured = start.elapsed().as_secs_f64();
    let work = bound.statistical_work();
    let start = Instant::now();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let evaluated = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let result = bound.publish(candidate)?;
    let published = start.elapsed().as_secs_f64();
    let value = output(&result)?;
    let inventory = heap
        .moments(heap.moment_companion(value.value())?)?
        .allocation_inventory()
        .clone();
    Ok((inventory, work, [captured, evaluated, published]))
}
/// Every numerical owner keeps its original role; opaque requests never pretend to report capacity.
fn write_inventory(writer: &mut impl Write, inventory: &StatisticsInventory) -> TestResult {
    let mut vectors = 0;
    let mut bytes = 0;
    let mut floats = 0;
    for allocation in inventory.allocations() {
        let entries = allocation.float_entries()?;
        floats += entries;
        if let Some(owned) = allocation.owned_bytes() {
            vectors += 1;
            bytes += owned;
        }
        writeln!(
            writer,
            "role={:?} index={} channel={:?} extent={:?} precision={:?} float_coordinates={entries} observed_capacity_bytes={:?}",
            allocation.role(),
            allocation.index(),
            allocation.channel(),
            allocation.extent(),
            allocation.precision(),
            allocation.owned_bytes()
        )?;
    }
    writeln!(
        writer,
        "scope=general-statistics requests={} float_coordinates={floats} vector_requests={vectors} cumulative_vector_bytes={bytes} peak_payload_bytes={} metadata_bytes={} named_backend_allowance_bytes={}",
        inventory.allocations().len(),
        inventory.peak_payload_bytes(),
        inventory.metadata_bytes(),
        inventory.backend_allowance_bytes()
    )?;
    Ok(())
}
