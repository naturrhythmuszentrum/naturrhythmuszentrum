// SPDX-License-Identifier: AGPL-3.0-only
//! A declared three-point grid reads one original invariant and retains each independent RHS.

use super::invariants::{mutate, request, selections};
use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use crate::spectral_testing::Fixture;
use nrz_matrix::{
    Binding, CheckedOperation, ExecutionLimits, Heap, InvariantCache, InvariantValue,
    ReadingBinding, RecordId, ValueBinding,
};
use nrz_scalar::ArithmeticContext;

struct Consumer {
    rhs: ValueBinding<f64>,
    rhs_handle: nrz_matrix::Handle<nrz_matrix::Value, f64>,
    output: ValueBinding<f64>,
    record: RecordId,
}

struct Grid {
    heap: Heap,
    sources: [ReadingBinding<f64>; 2],
    operation: CheckedOperation,
    cache: InvariantCache,
    coefficient: InvariantValue<f64>,
    consumers: Vec<Consumer>,
}

/// Three distinct RHS points retain the same cached source and independent published result versions.
/// ´test:integration:lc-grid-isolates-one-rhs-replacement´
#[test]
fn lc_grid_isolates_one_rhs_replacement() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut grid = prepare(&context)?;
    let versions = versions(&grid)?;
    let rhs = grid.consumers[1].rhs_handle;
    let revision = grid.heap.value(rhs)?.stamp().version();
    grid.heap.replace_represented(rhs, revision, &[2.0, 3.0])?;
    assert_ne!(grid.heap.value(rhs)?.stamp().version(), revision);
    for (index, consumer) in grid.consumers.iter().enumerate() {
        let result = grid
            .heap
            .value_by_symbol(consumer.output.symbol(), &consumer.output);
        assert_eq!(result.is_ok(), index != 1);
        assert_eq!(
            grid.heap.operation_record(consumer.record).is_ok(),
            index != 1
        );
        if index != 1 {
            assert_eq!(result?.stamp(), versions[index]);
        }
    }
    assert!(grid.coefficient.binding(&grid.heap, &context).is_ok());
    let selected = selections(&mut grid.heap, &grid.sources)?;
    let hit = grid.cache.materialize(
        &mut grid.heap,
        request(&grid.operation, &selected),
        &context,
        &mut ledger()?,
    )?;
    assert!(hit.reused());
    assert_eq!(hit.work(), 5);
    assert_eq!(grid.cache.materializations(), 1);
    Ok(())
}

/// Every original L/C or radius replacement invalidates all three consumers and the shared cache.
/// ´test:integration:lc-grid-invalidates-all-source-and-companion-consumers´
#[test]
fn lc_grid_invalidates_all_source_and_companion_consumers() -> TestResult {
    let context = ArithmeticContext::new()?;
    for change in 0..6 {
        let mut grid = prepare(&context)?;
        let selected = selections(&mut grid.heap, &grid.sources)?;
        mutate(&mut grid.heap, &grid.sources, change)?;
        assert!(grid.coefficient.binding(&grid.heap, &context).is_err());
        for consumer in &grid.consumers {
            assert!(
                grid.heap
                    .value_by_symbol(consumer.output.symbol(), &consumer.output)
                    .is_err()
            );
            assert!(grid.heap.operation_record(consumer.record).is_err());
            assert!(
                grid.heap
                    .value_by_symbol(consumer.rhs.symbol(), &consumer.rhs)
                    .is_ok()
            );
        }
        assert!(
            grid.cache
                .materialize(
                    &mut grid.heap,
                    request(&grid.operation, &selected),
                    &context,
                    &mut ledger()?
                )
                .is_err()
        );
        assert_eq!(grid.cache.materializations(), 1);
    }
    Ok(())
}

fn prepare(context: &ArithmeticContext) -> TestResult<Grid> {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let operation = super::operator(&fixture)?;
    let (mut heap, sources) = fixture.import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])?;
    let mut cache = InvariantCache::new(&heap, 1);
    let mut consumers = Vec::new();
    let mut resources = ledger()?;
    let mut coefficient: Option<InvariantValue<f64>> = None;
    for (index, values, expected) in [
        (0, [1.0, 2.0], [-20, 6250]),
        (1, [2.0, 3.0], [-30, 12_500]),
        (2, [3.0, 4.0], [-40, 18_750]),
    ] {
        let selected = selections(&mut heap, &sources)?;
        let usage = cache.materialize(
            &mut heap,
            request(&operation, &selected),
            context,
            &mut resources,
        )?;
        assert_eq!(usage.reused(), index != 0);
        assert_eq!(usage.work(), if index == 0 { 958 } else { 5 });
        if let Some(previous) = &coefficient {
            assert_eq!(usage.value().record(), previous.record());
        }
        consumers.push(consume(
            &mut heap,
            &operation,
            usage.value(),
            context,
            values,
            expected,
        )?);
        coefficient = Some(usage.value().clone());
    }
    assert_eq!(resources.usage().work, 968);
    assert_eq!(cache.materializations(), 1);
    heap.compact()?;
    Ok(Grid {
        heap,
        sources,
        operation,
        cache,
        coefficient: coefficient.ok_or("grid coefficient")?,
        consumers,
    })
}

fn consume(
    heap: &mut Heap,
    operation: &CheckedOperation,
    coefficient: &InvariantValue<f64>,
    context: &ArithmeticContext,
    values: [f64; 2],
    expected: [i64; 2],
) -> TestResult<Consumer> {
    let solve = super::reuse::solve_from_operator(operation.outputs()[0].shape())?;
    let original = coefficient.binding(heap, context)?.clone();
    let imported = heap.import_declared_bounds(
        nrz_matrix::Extent::new(2, 1)?,
        &nrz_bindings::Descriptor::ratio(),
        &values,
        &[0.0; 2],
    )?;
    let rhs = heap.symbol_binding(imported.value())?;
    let bound = Binding::from_symbols(
        solve,
        vec![original, rhs.clone()],
        ExecutionLimits::default(),
    )
    .validate(heap)?;
    let output = bound.result_bindings()[0].clone();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let bundle = crate::kernel_testing::output(&outcome)?;
    for (index, target) in expected.into_iter().enumerate() {
        let (center, total, _) = crate::kernel_testing::entry(heap, bundle, index)?;
        assert!(Rational::new(target, 1)?.covered(center, total)?);
    }
    assert!(heap.value_by_symbol(output.symbol(), &output).is_ok());
    Ok(Consumer {
        rhs_handle: imported.value(),
        rhs,
        output,
        record: outcome.record(),
    })
}

fn versions(grid: &Grid) -> TestResult<Vec<nrz_matrix::ValueStamp>> {
    grid.consumers
        .iter()
        .map(|consumer| {
            Ok(grid
                .heap
                .value_by_symbol(consumer.output.symbol(), &consumer.output)?
                .stamp())
        })
        .collect()
}

/// Each independent grid solve retains the measured certificate and scoped execution inventory.
/// ´test:integration:lc-grid-consumer-resource-owners´
#[test]
fn lc_grid_consumer_resource_owners() -> TestResult {
    let grid = prepare(&ArithmeticContext::new()?)?;
    let mut profiles = Vec::new();
    for consumer in &grid.consumers {
        let record = grid.heap.operation_record(consumer.record)?;
        let evidence = record.solve_evidence().ok_or("grid solve evidence")?;
        let inventory = grid.heap.execution_inventory(consumer.record)?;
        profiles.push([
            evidence.work(),
            inventory.observed_allocations(),
            inventory.cumulative_payload_bytes(),
            inventory.peak_payload_bytes(),
            inventory.metadata_bytes(),
            inventory.backend_allowance_bytes(),
        ]);
    }
    assert_eq!(profiles, [[1536, 11, 224, 64, 240, 12_288]; 3]);
    Ok(())
}
