// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone cold and reused LC coefficients share the established public reciprocal fixtures.

use nrz_matrix as test_api;

#[path = "../src/testing/conversion.rs"]
#[allow(
    dead_code,
    reason = "measurement uses the successful subset of shared acceptance support"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared fixture declarations remain private to this example"
)]
mod conversion_testing;

#[path = "../src/testing/kernels.rs"]
#[allow(
    dead_code,
    reason = "measurement uses the successful subset of shared acceptance support"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared fixture declarations remain private to this example"
)]
mod kernel_testing;

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

#[path = "../src/testing/reciprocal.rs"]
#[allow(
    dead_code,
    reason = "measurement uses the successful subset of shared acceptance support"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared fixture declarations remain private to this example"
)]
mod reciprocal_testing;

use nrz_matrix::{Binding, ExecutionLimits, Extent, Heap, HeapLimits, InvariantCache};
use nrz_scalar::ArithmeticContext;
use std::io::{self, Write};

/// Each process owns one complete LC fixture; only the requested numerical phase is clocked.
fn main() -> reading_testing::TestResult {
    let mode = std::env::args()
        .nth(1)
        .ok_or("expected cold, hit or grid")?;
    if mode == "grid" {
        return grid();
    }
    if mode != "cold" && mode != "hit" {
        return Err("expected cold, hit or grid".into());
    }
    let (prepared, recipe) = reciprocal_testing::proved()?;
    let operation =
        reciprocal_testing::draft(&prepared, &recipe)?.check(ExecutionLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut inputs = Vec::new();
    for (entry, value) in recipe.frame().entries().iter().zip([0.25_f64, 0.0625]) {
        let bundle =
            heap.import_declared_bounds(Extent::new(1, 1)?, entry.descriptor(), &[value], &[0.0])?;
        inputs.push(heap.symbol_binding(bundle.value())?);
    }
    let context = ArithmeticContext::new()?;
    let mut cache = InvariantCache::new(&heap, 1);
    let mut budget = reading_testing::ledger()?;
    if mode == "hit" {
        cache.materialize(
            &mut heap,
            Binding::from_symbols(
                operation.clone(),
                inputs.clone(),
                ExecutionLimits::default(),
            ),
            &context,
            &mut budget,
        )?;
    }
    let request = Binding::from_symbols(operation, inputs, ExecutionLimits::default());
    let started = std::time::Instant::now();
    let result = cache.materialize(&mut heap, request, &context, &mut budget)?;
    let seconds = started.elapsed().as_secs_f64();
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "owner=invariant-{mode} precision=binary64 work={} cumulative_work={} owned_bytes={} avoided={} seconds={seconds:.9}",
        result.work(),
        budget.usage().work,
        result.value().owned_bytes(),
        result.avoided_evaluation_work()
    )?;
    write_inventory(&mut output, &heap, result.value().record(), result.reused())?;
    output.flush()?;
    Ok(())
}

/// Scratch receipts are historical allocation facts of the original materialization on either path.
fn write_inventory(
    output: &mut impl Write,
    heap: &Heap,
    record: nrz_matrix::RecordId,
    reused: bool,
) -> reading_testing::TestResult {
    let inventory = heap
        .operation_record(record)?
        .reciprocal_inventory()
        .ok_or("missing reciprocal inventory")?;
    let source = if reused { "cached" } else { "fresh" };
    for allocation in inventory.allocations() {
        writeln!(
            output,
            "owner=reciprocal-kernel source={source} stage={:?} logical_entries={} float_entries={} owned_bytes={}",
            allocation.binding().stage(),
            allocation.entries(),
            allocation.float_entries(),
            allocation.owned_bytes(),
        )?;
    }
    writeln!(
        output,
        "scope=reciprocal-kernel source={source} peak_payload_bytes={} planning_bytes={} receipt_bytes={} candidate_metadata_bytes={} backend_allowance=not-applicable",
        inventory.peak_payload_bytes(),
        inventory.planning_bytes(),
        inventory.metadata_bytes(),
        inventory.candidate_metadata_bytes(),
    )?;
    Ok(())
}

/// Three separately certified RHS consumers read one physical coefficient under one cache ledger.
fn grid() -> reading_testing::TestResult {
    let (prepared, recipe) = reciprocal_testing::proved()?;
    let operation =
        reciprocal_testing::draft(&prepared, &recipe)?.check(ExecutionLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut inputs = Vec::new();
    for (entry, value) in recipe.frame().entries().iter().zip([0.25_f64, 0.0625]) {
        let imported =
            heap.import_declared_bounds(Extent::new(1, 1)?, entry.descriptor(), &[value], &[0.0])?;
        inputs.push(heap.symbol_binding(imported.value())?);
    }
    let context = ArithmeticContext::new()?;
    let mut cache = InvariantCache::new(&heap, 1);
    let mut budget = reading_testing::ledger()?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    for (index, values) in [[1.0, 2.0], [2.0, 3.0], [3.0, 4.0]].into_iter().enumerate() {
        let started = std::time::Instant::now();
        let usage = cache.materialize(
            &mut heap,
            Binding::from_symbols(
                operation.clone(),
                inputs.clone(),
                ExecutionLimits::default(),
            ),
            &context,
            &mut budget,
        )?;
        writeln!(
            output,
            "owner=grid-coefficient point={index} work={} reused={} owned_bytes={} seconds={:.9}",
            usage.work(),
            usage.reused(),
            usage.value().owned_bytes(),
            started.elapsed().as_secs_f64()
        )?;
        grid_consumer(
            &mut output,
            &mut heap,
            &operation,
            usage.value(),
            &context,
            index,
            values,
        )?;
    }
    writeln!(
        output,
        "owner=grid cache_materializations={} cache_work={} rhs_payload_bytes=96 result_payload_bytes=144",
        cache.materializations(),
        budget.usage().work
    )?;
    output.flush()?;
    Ok(())
}

/// Each consumer owns its independent RHS bundle and its numerical solve record.
fn grid_consumer(
    output: &mut impl Write,
    heap: &mut Heap,
    operation: &nrz_matrix::CheckedOperation,
    coefficient: &nrz_matrix::InvariantValue<f64>,
    context: &ArithmeticContext,
    index: usize,
    values: [f64; 2],
) -> reading_testing::TestResult {
    let solve = grid_operation(operation.outputs()[0].shape())?;
    let imported = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &nrz_bindings::Descriptor::ratio(),
        &values,
        &[0.0; 2],
    )?;
    let rhs = heap.symbol_binding(imported.value())?;
    let coefficients = coefficient.binding(heap, context)?.clone();
    let started = std::time::Instant::now();
    let bound = Binding::from_symbols(solve, vec![coefficients, rhs], ExecutionLimits::default())
        .validate(heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    grid_inventory(
        output,
        heap,
        outcome.record(),
        index,
        started.elapsed().as_secs_f64(),
    )
}

/// Preserve the original operator axes in every independent RHS and output shape.
fn grid_operation(
    operator: &nrz_matrix::CheckedShape,
) -> reading_testing::TestResult<nrz_matrix::CheckedOperation> {
    let limits = nrz_matrix::ShapeLimits::default();
    let rhs = operator.select(
        kernel_testing::axis(1)?,
        operator.codomain().clone(),
        nrz_matrix::EntryMap::new(operator.extent(), Extent::new(2, 1)?, vec![0, 1], limits)?,
        limits,
    )?;
    let solution = operator.select(
        rhs.domain().clone(),
        operator.domain().clone(),
        nrz_matrix::EntryMap::new(operator.extent(), Extent::new(2, 1)?, vec![0, 1], limits)?,
        limits,
    )?;
    Ok(
        nrz_matrix::OperationDraft::solve(operator.clone(), rhs, solution, limits)?
            .check(ExecutionLimits::default())?,
    )
}

/// Report certificate Work and covered execution storage without calling backend allowances owned bytes.
fn grid_inventory(
    output: &mut impl Write,
    heap: &Heap,
    record: nrz_matrix::RecordId,
    index: usize,
    seconds: f64,
) -> reading_testing::TestResult {
    let evidence = heap
        .operation_record(record)?
        .solve_evidence()
        .ok_or("grid solve evidence")?;
    let inventory = heap.execution_inventory(record)?;
    writeln!(
        output,
        "owner=grid-consumer point={index} certificate_work={} observed_allocations={} cumulative_owned_bytes={} peak_owned_bytes={} registry_metadata_bytes={} backend_allowance_bytes={} output_payload_bytes=48 seconds={seconds:.9}",
        evidence.work(),
        inventory.observed_allocations(),
        inventory.cumulative_payload_bytes(),
        inventory.peak_payload_bytes(),
        inventory.metadata_bytes(),
        inventory.backend_allowance_bytes()
    )?;
    Ok(())
}
