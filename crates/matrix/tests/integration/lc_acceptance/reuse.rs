// SPDX-License-Identifier: AGPL-3.0-only
//! Reusing the original normalized LC operator cannot reuse an earlier solve certificate.

use super::{operator, represented};
use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use nrz_bindings::Descriptor;
use nrz_matrix::{Binding, ExecutionLimits, Extent, Heap, HeapLimits};

/// An unchanged LC operator reuses its factor across RHS versions while every solve is certified anew.
/// ´test:integration:lc-acceptance-factor-reuse-retains-original-dependencies´
#[test]
fn lc_acceptance_factor_reuse_retains_original_dependencies() -> TestResult {
    let Case {
        mut heap,
        sources,
        coefficients,
        solve,
        rhs,
    } = prepare()?;
    let limits = ExecutionLimits::default();
    let bound = stage(
        "first solve binding",
        Binding::new(solve.clone(), vec![coefficients, rhs], limits).validate(&mut heap),
    )?;
    let factor = stage("factor", bound.factor())?;
    let inputs = stage("first normalized inputs", bound.normalized_inputs())?;
    let candidate = stage("first candidate", bound.evaluate_factored(&inputs, &factor))?;
    let first = stage("first publication", bound.publish(candidate))?;
    let output = stage("first output", crate::kernel_testing::output(&first))?;
    stage(
        "first independent oracle",
        check(&heap, output, [-20, 6250]),
    )?;
    stage("compaction", heap.compact())?;
    let version = stage("RHS version", heap.value(rhs))?.stamp().version();
    stage(
        "RHS replacement",
        heap.replace_represented(rhs, version, &[2.0, 3.0]),
    )?;
    let second = stage(
        "reused solve",
        heap.solve_factored_recorded(
            Binding::new(solve.clone(), vec![coefficients, rhs], limits),
            &factor,
        ),
    )?;
    let output = stage("second output", crate::kernel_testing::output(&second))?;
    stage(
        "second independent oracle",
        check(&heap, output, [-30, 12_500]),
    )?;
    let record = stage("fresh record", heap.operation_record(second.record()))?;
    let usage = stage(
        "fresh solve evidence",
        record.solve_evidence().ok_or("solve evidence"),
    )?;
    let usage = stage("factor use", usage.factor_use().ok_or("factor use"))?;
    assert!(usage.reused());
    assert_eq!(usage.origin(), factor.execution());
    assert_ne!(usage.origin(), usage.execution());
    assert!(heap.operation_record(first.record()).is_err());
    assert!(
        stage("historical record", heap.historical_record(first.record()))?
            .solve_evidence()
            .is_some()
    );
    let mut budget = stage("source write budget", ledger())?;
    stage(
        "C replacement",
        heap.replace_reading_entries(&sources[1], &[0.0625], &mut budget),
    )?;
    assert!(heap.operation_record(second.record()).is_err());
    assert!(
        heap.solve_factored_recorded(
            Binding::new(solve, vec![coefficients, rhs], limits),
            &factor
        )
        .is_err()
    );
    Ok(())
}

type ValueHandle = nrz_matrix::Handle<nrz_matrix::Value, f64>;

struct Case {
    heap: Heap,
    sources: [nrz_matrix::ReadingBinding<f64>; 2],
    coefficients: ValueHandle,
    solve: nrz_matrix::CheckedOperation,
    rhs: ValueHandle,
}

fn prepare() -> TestResult<Case> {
    let fixture = stage(
        "physical fixture",
        crate::spectral_testing::Fixture::new(["5", "1/10", "1/2000"]),
    )?;
    let mut heap = stage("heap", Heap::new(HeapLimits::default()))?;
    let sources = stage("source import", represented(&fixture, &mut heap))?;
    let operation = stage("operator lowering", operator(&fixture))?;
    let shape = operation.outputs()[0].shape().clone();
    let coefficients = stage(
        "operator execution",
        execute_operator(operation, &mut heap, &sources),
    )?;
    let extent = stage("RHS extent", Extent::new(2, 1))?;
    let rhs = stage(
        "RHS import",
        heap.import_represented(extent, &Descriptor::ratio(), &[1.0_f64, 2.0]),
    )?;
    let solve = stage("solve scalar declaration", solve_from_operator(&shape))?;
    Ok(Case {
        heap,
        sources,
        coefficients,
        solve,
        rhs: rhs.value(),
    })
}

fn stage<T, E: std::fmt::Debug>(name: &str, result: Result<T, E>) -> TestResult<T> {
    result.map_err(|error| format!("{name}: {error:?}").into())
}

fn check(heap: &Heap, output: nrz_matrix::PublishedBundle<f64>, expected: [i64; 2]) -> TestResult {
    for (index, value) in expected.into_iter().enumerate() {
        let (center, total, error) = crate::kernel_testing::entry(heap, output, index)?;
        assert!(Rational::new(value, 1)?.covered(center, total)?);
        assert!(error <= total);
    }
    Ok(())
}

fn execute_operator(
    operation: nrz_matrix::CheckedOperation,
    heap: &mut Heap,
    sources: &[nrz_matrix::ReadingBinding<f64>; 2],
) -> TestResult<nrz_matrix::Handle<nrz_matrix::Value, f64>> {
    let selected = sources
        .iter()
        .map(|source| heap.select_reading(source))
        .collect::<Result<Vec<_>, _>>()?;
    let bound =
        Binding::from_symbols(operation, selected, ExecutionLimits::default()).validate(heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let output = bound.publish(candidate)?;
    Ok(crate::kernel_testing::output(&output)?.value())
}

/// Solve axes retain the actual published operator identities instead of equal fresh declarations.
fn solve_from_operator(
    operator: &nrz_matrix::CheckedShape,
) -> TestResult<nrz_matrix::CheckedOperation> {
    let limits = nrz_matrix::ShapeLimits::default();
    let rhs = operator.select(
        crate::kernel_testing::axis(1)?,
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
