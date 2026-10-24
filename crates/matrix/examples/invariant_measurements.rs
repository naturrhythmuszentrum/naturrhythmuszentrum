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
    let mode = std::env::args().nth(1).ok_or("expected cold or hit")?;
    if mode != "cold" && mode != "hit" {
        return Err("expected cold or hit".into());
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
    output.flush()?;
    Ok(())
}
