// SPDX-License-Identifier: AGPL-3.0-only
//! A normalized operator retains both physical input versions through the ordinary provenance graph.

use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::{draft, proved};
use nrz_matrix::{Binding, DependencyPolicy, ExecutionLimits, Extent, Heap, HeapLimits};
use std::rc::Rc;

/// Replacing C invalidates the complete operator and every attached result reading, even at equal centers.
/// ´test:integration:normalized-lc-operator-retains-capacitance-version´
#[test]
fn normalized_lc_operator_retains_capacitance_version() -> TestResult {
    let (mut prepared, recipe) = proved()?;
    let coverage = crate::reciprocal_testing::coverage(&mut prepared.case)?;
    let operation = draft(&prepared, &recipe)?.check(ExecutionLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut inputs = Vec::new();
    for (entry, value) in recipe.frame().entries().iter().zip([0.025_f64, 0.000_01]) {
        inputs.push(
            heap.import_declared_bounds(Extent::new(1, 1)?, entry.descriptor(), &[value], &[0.0])?
                .value(),
        );
    }
    let capacitance = inputs[1];
    let version = heap.value(capacitance)?.stamp().version();
    let mut bound =
        Binding::new(operation, inputs, ExecutionLimits::default()).validate(&mut heap)?;
    let readings = bound.predeclare_result_readings(
        Rc::new(prepared.case.cas),
        coverage,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    )?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let bundle = crate::kernel_testing::output(&outcome)?;
    let record = heap.operation_record(outcome.record())?;
    assert_eq!(
        record
            .operation()
            .reciprocal_map()
            .ok_or("recipe")?
            .reuse_key(),
        recipe.reuse_key()
    );
    heap.replace_declared_bounds(capacitance, version, &[0.000_01], &[0.0])?;
    assert!(heap.value(bundle.value()).is_err());
    assert!(heap.reading_entries(&readings[0]).is_err());
    assert!(heap.operation_record(outcome.record()).is_err());
    Ok(())
}
