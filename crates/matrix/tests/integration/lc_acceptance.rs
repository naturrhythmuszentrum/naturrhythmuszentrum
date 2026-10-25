// SPDX-License-Identifier: AGPL-3.0-only
//! Shared original LC bindings exercise the operator and complete spectral publication together.

mod boundaries;
mod bridge;
mod companions;
mod construction_costs;
mod costs;
mod energy;
mod family_costs;
mod family_invalidation;
mod family_models;
mod family_native_costs;
mod family_refusals;
mod family_statistics;
mod family_support;
mod grid;
mod invariants;
mod oracle_regression;
mod reciprocal_costs;
mod reciprocal_refusals;
mod reciprocal_statistics;
mod reuse;
mod root_payload;
mod root_publication;
mod root_refusals;
mod root_statistics;
mod root_support;
mod root_work;
mod selected_joint;
mod state;
#[allow(
    dead_code,
    reason = "support also supplies standalone owner measurement entry points"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "private support is shared with the measurement example"
)]
mod support;
use support::{owners, state_fixture};

use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use crate::spectral_testing::Fixture;
use nrz_matrix::{
    Binding, DependencyPolicy, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft,
    ReadingBinding, ReadingRegion, SymbolState,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits};
use std::rc::Rc;

/// One original input pair drives both operations without an independent copied physical source.
/// ´test:integration:lc-acceptance-shared-operator-spectrum´
#[test]
fn lc_acceptance_shared_operator_spectrum() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let operation = operator(&fixture)?;
    let (graph, coverage) = operator_inventory()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let sources = represented(&fixture, &mut heap)?;
    let spectrum = fixture.plan(sources.clone(), &mut ledger()?)?;
    let publication = fixture.publication(spectrum, &mut ledger()?)?;
    let mut selection_budget = ledger()?;
    let selected = sources
        .iter()
        .map(|input| heap.select_reading(input, &mut selection_budget))
        .collect::<Result<Vec<_>, _>>()?;
    let mut bound = Binding::from_symbols(operation, selected, ExecutionLimits::default())
        .validate(&mut heap)?;
    let planned = bound.predeclare_result_readings(
        graph,
        coverage,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    )?;
    assert_eq!(planned[0].state(), SymbolState::Pending);
    assert!(bound.reading_entries(&planned[0]).is_err());
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let bundle = crate::kernel_testing::output(&outcome)?;
    operator_oracle(&heap, bundle)
        .map_err(|error| format!("independent operator oracle: {error}"))?;
    let context = ArithmeticContext::new()?;
    let roots = heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
    for (index, expected) in [(0, 8), (1, -8)] {
        let value = roots.reading(index, &heap, &context, &mut ledger()?)?[1];
        assert!(Rational::new(expected, 1)?.covered(value.center, value.error)?);
    }
    heap.replace_reading_entries(&sources[1], &[0.0625], &mut ledger()?)?;
    assert!(heap.operation_record(outcome.record()).is_err());
    assert!(heap.reading_entries(&planned[0]).is_err());
    assert!(roots.reading(0, &heap, &context, &mut ledger()?).is_err());
    Ok(())
}

/// Dyadic L/C coordinates have represented-input scope without declared physical uncertainty.
fn represented(fixture: &Fixture, heap: &mut Heap) -> TestResult<[ReadingBinding<f64>; 2]> {
    let regions = [0.25_f64, 0.0625]
        .into_iter()
        .map(|value| {
            Ok(ReadingRegion {
                extent: Extent::new(1, 1)?,
                values: vec![value],
            })
        })
        .collect::<TestResult<Vec<_>>>()?;
    let imported = heap.import_readings(
        Rc::clone(&fixture.graph),
        fixture.coverage.clone(),
        Vec::new(),
        &regions,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    )?;
    imported
        .bindings()
        .to_vec()
        .try_into()
        .map_err(|_| "two LC sources".into())
}

/// The original checked physical similarity is lowered independently of the spectrum executor.
fn operator(fixture: &Fixture) -> TestResult<nrz_matrix::CheckedOperation> {
    let recipe = fixture.law.operator();
    let (inputs, output) = crate::reciprocal_testing::shapes(recipe)?;
    Ok(OperationDraft::positive_reciprocal_map(
        &fixture.graph,
        recipe.clone(),
        inputs,
        output,
        LiteralLimits::default(),
        &mut ledger()?,
    )?
    .check(ExecutionLimits::default())?)
}

/// A fresh output declaration cannot change the already fixed physical proof header.
fn operator_inventory() -> TestResult<(
    Rc<nrz_bindings::readings::algebra::Cas>,
    nrz_matrix::ReadingCoverage<usize>,
)> {
    let mut output_graph = crate::reading_testing::Fixture::new(&[
        ("b", crate::reading_testing::matrix(2, 2)),
        ("B", crate::reading_testing::matrix(2, 2)),
    ])?;
    let coverage = crate::reciprocal_testing::readings::coverage(&mut output_graph)?;
    Ok((Rc::new(output_graph.cas), coverage))
}

/// Exact physical reciprocal ratios provide an independent normalized-operator oracle.
fn operator_oracle(heap: &Heap, bundle: nrz_matrix::PublishedBundle<f64>) -> TestResult {
    for (index, numerator, denominator) in [(0, 0, 1), (1, -1, 10), (2, 1, 6250), (3, 0, 1)] {
        let (center, _, error) = crate::kernel_testing::entry(heap, bundle, index)?;
        assert!(Rational::new(numerator, denominator)?.covered(center, error)?);
    }
    Ok(())
}
