// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic inspection transcripts retain environment and numeric bits, with stable local names.

use super::support::{self, Case, Parameter, ResultOf, Sample};
use nrz_matrix::{
    Binding, CheckedOperation, ExecutionLimits, Extent, Handle, Heap, HeapLimits, RecordId,
    SolveEvidence, Value,
};
use nrz_units::Descriptor;
use std::fmt::Write;

/// The committed physical and adversarial cases reproduce every inspected numerical bit on fresh heaps.
/// ´test:integration:integrated-lifecycles-have-identical-numerical-transcripts´
#[test]
fn integrated_lifecycles_have_identical_numerical_transcripts() -> ResultOf {
    let cases: [fn() -> ResultOf<Case>; 6] = [
        support::truss,
        support::resistor,
        support::edge,
        support::shared,
        support::stale,
        support::stagnation,
    ];
    for run in cases {
        let first = run()?;
        let second = run()?;
        assert_eq!(
            transcript(&first)?,
            transcript(&second)?,
            "case {}",
            first.name
        );
    }
    Ok(())
}

/// Re-execution has fresh scoped identities; stable public observations agree within and across heaps.
/// ´test:integration:repeated-and-fresh-heap-solves-preserve-transcript-determinism´
#[test]
fn repeated_and_fresh_heap_solves_preserve_transcript_determinism() -> ResultOf {
    let limits = ExecutionLimits::default();
    let operation = crate::solve_testing::draft(2, 1)?.check(limits)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = import(&mut heap)?;
    let first = recorded(&mut heap, &operation, &inputs)?;
    let second = recorded(&mut heap, &operation, &inputs)?;
    let mut fresh = Heap::new(HeapLimits::default())?;
    let copied = import(&mut fresh)?;
    let third = recorded(&mut fresh, &operation, &copied)?;
    assert_ne!(first.0, second.0);
    assert_ne!(second.0, third.0);
    assert_ne!(first.2, second.2);
    assert_ne!(second.2, third.2);
    assert_eq!(first.1, second.1);
    assert_eq!(second.1, third.1);
    Ok(())
}

/// Import exactly the same represented input bits, with new heap-owned identities.
fn import(heap: &mut Heap) -> ResultOf<Vec<Handle<Value, f64>>> {
    let operator = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[3.0, 1.0, 1.0, 2.0],
    )?;
    let rhs = heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[5.0, 5.0])?;
    Ok(vec![operator.value(), rhs.value()])
}

/// Raw debug records deliberately retain identities; deterministic observations use their stable relationships.
fn recorded(
    heap: &mut Heap,
    operation: &CheckedOperation,
    inputs: &[Handle<Value, f64>],
) -> ResultOf<(RecordId, Vec<u8>, String)> {
    let outcome = heap.solve_recorded(Binding::new(
        operation.clone(),
        inputs.to_vec(),
        ExecutionLimits::default(),
    ))?;
    let bundle = crate::kernel_testing::output(&outcome)?;
    crate::solve_public::check(heap, bundle, &[3.0, 1.0, 1.0, 2.0], &[5.0, 5.0], 2)?;
    let record = heap.operation_record(outcome.record())?;
    let mut output = String::new();
    writeln!(
        output,
        "{} {:?} {:?}",
        record.environment(),
        record.precision(),
        record.solve_stage()
    )?;
    solve(&mut output, record.solve_evidence().ok_or("solve proof")?)?;
    for index in 0..2 {
        let values: [f64; 3] = crate::kernel_testing::entry(heap, bundle, index)?.into();
        numbers(&mut output, &values)?;
    }
    Ok((outcome.record(), output.into_bytes(), format!("{record:?}")))
}

/// This is an evidence comparison format, not a production serialization API.
fn transcript(case: &Case) -> ResultOf<Vec<u8>> {
    let mut output = String::new();
    writeln!(
        output,
        "matrix-acceptance-observations-v1 {} {} {} {}",
        case.name,
        std::env::consts::ARCH,
        std::env::consts::OS,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    )?;
    writeln!(
        output,
        "inputs={:?} storage={:?} factors={} provenance={:?}",
        case.input_regions,
        case.storage(),
        case.factors,
        case.provenance
    )?;
    for sample in &case.samples {
        observation(&mut output, sample)?;
    }
    for cross in &case.cross {
        parameter(&mut output, cross)?;
    }
    for refusal in &case.refusals {
        writeln!(
            output,
            "{:?} {:?} {}",
            refusal.cause, refusal.stage, refusal.work
        )?;
        if let Some(proof) = &refusal.proof {
            solve(&mut output, proof)?;
        }
    }
    Ok(output.into_bytes())
}

/// Retain all returned values, scoped bounds, parameter regions, dimensions, modes and costs.
fn observation(output: &mut String, sample: &Sample) -> ResultOf {
    writeln!(
        output,
        "{} {} {:?} {:?} {:?} {}",
        sample.name,
        sample.environment,
        sample.quantity,
        sample.stage,
        sample.assurance,
        sample.statistical_work
    )?;
    numbers(output, &sample.centers)?;
    numbers(output, &sample.totals)?;
    numbers(output, &sample.errors)?;
    for region in &sample.parameters {
        parameter(output, region)?;
    }
    writeln!(output, "kernel={:?}", sample.kernel)?;
    if let Some(proof) = &sample.solve {
        solve(output, proof)?;
    }
    Ok(())
}

/// A fixed column order and exact binary encoding avoid locale and decimal display ambiguity.
fn parameter(output: &mut String, region: &Parameter) -> ResultOf {
    writeln!(output, "{:?} {:?}", region.shape, region.dimensions)?;
    numbers(output, &region.centers)?;
    numbers(output, &region.errors)
}

/// Every optional theorem stage is encoded distinctly from a numeric zero.
fn solve(output: &mut String, proof: &SolveEvidence) -> ResultOf {
    writeln!(
        output,
        "{:?} {} {} complete={} inverse={}",
        proof.precision(),
        proof.arithmetic_revision(),
        proof.work(),
        proof.is_complete(),
        proof.is_inverse()
    )?;
    numbers(
        output,
        &[
            proof.inverse_defect(),
            proof.witness_norm(),
            proof.inverse_margin(),
            proof.inverse_norm(),
            proof.residual_norm(),
            proof.candidate_norm(),
            proof.numerical_radius(),
        ],
    )?;
    for value in [
        proof.operator_radius(),
        proof.rhs_radius(),
        proof.perturbation_product(),
        proof.perturbation_margin(),
        proof.total_radius(),
        proof.output_radius(),
        proof.output_numerical_radius(),
    ] {
        match value {
            Some(value) => numbers(output, std::slice::from_ref(&value))?,
            None => writeln!(output, "absent")?,
        }
    }
    writeln!(output, "budget={:?}", proof.budget())?;
    for direction in proof.sensitivities() {
        writeln!(output, "direction={}", direction.direction())?;
        numbers(output, &[direction.residual_norm(), direction.radius()])?;
    }
    factor_and_recovery(output, proof)
}

/// Factor reuse and attempt relationships remain in order while fresh owner IDs are excluded.
fn factor_and_recovery(output: &mut String, proof: &SolveEvidence) -> ResultOf {
    if let Some(factor) = proof.factor_use() {
        let key = factor.dependency();
        writeln!(
            output,
            "factor reused={} {:?} {:?} {} {}",
            factor.reused(),
            key.layout(),
            key.precision(),
            key.algorithm_revision(),
            key.arithmetic_revision()
        )?;
    }
    if let Some(recovery) = proof.recovery() {
        writeln!(
            output,
            "recovery {:?} {} {}",
            recovery.stop(),
            recovery.work(),
            recovery.reserved_attempts()
        )?;
        for attempt in recovery.attempts() {
            writeln!(
                output,
                "attempt {} {:?} {:?} {:?}",
                attempt.iteration(),
                attempt.precision(),
                attempt.residual_precision(),
                attempt.budget()
            )?;
            numbers(output, &[attempt.residual_norm(), attempt.radius()])?;
        }
    }
    Ok(())
}

/// Encode all finite floating observations as exactly sixteen hexadecimal digits per scalar.
fn numbers(output: &mut String, values: &[f64]) -> ResultOf {
    for value in values {
        write!(output, "{:016x} ", value.to_bits())?;
    }
    output.push('\n');
    Ok(())
}
