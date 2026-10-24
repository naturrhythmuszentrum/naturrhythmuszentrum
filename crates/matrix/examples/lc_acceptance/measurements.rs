// SPDX-License-Identifier: AGPL-3.0-only
//! Operation clocks exclude prerequisite owner setup and keep cumulative construction separate.

use super::{Observation, fixture};
use crate::reading_testing::{TestResult, ledger};
use crate::support::{owners, references, state_fixture};
use std::time::Instant;

/// Select one complete owner per process for independent server wall and RSS observations.
pub(crate) fn measure(name: &str) -> TestResult<Vec<Observation>> {
    match name {
        "vector-covariance" => vector(),
        "overlap-replay" => replay(),
        name => owner(name),
    }
}

/// Measure exactly the operation used by the owner boundary tests.
fn owner(name: &str) -> TestResult<Vec<Observation>> {
    let operation = [
        "inventory",
        "entries",
        "whole-base",
        "normalize",
        "restore",
        "norm",
    ]
    .iter()
    .position(|candidate| *candidate == name)
    .ok_or("unknown LC owner")?;
    let case = owners::Case::new()?;
    let mut budget = ledger()?;
    let start = Instant::now();
    case.run(operation, &mut budget)?;
    Ok(vec![Observation {
        phase: name.into(),
        work: budget.usage().work,
        seconds: start.elapsed().as_secs_f64(),
        accounting: "input_payload_bytes=48".into(),
    }])
}

/// Export each original overlap, then inspect it in an independent default receiving store.
fn replay() -> TestResult<Vec<Observation>> {
    let plan = state_fixture::plan(&references()?)?;
    let mut result = Vec::new();
    for (index, relation) in plan.coverage.overlaps().iter().enumerate() {
        let mut budget = ledger()?;
        let start = Instant::now();
        let bytes = relation.export(&plan.graph, &mut budget)?;
        result.push(Observation {
            phase: format!("overlap-{index}-export"),
            work: budget.usage().work,
            seconds: start.elapsed().as_secs_f64(),
            accounting: format!("wire_bytes={}", bytes.len()),
        });
        let mut receiver =
            nrz_bindings::readings::algebra::Cas::new(nrz_bindings::Limits::default())?;
        let mut budget = ledger()?;
        let start = Instant::now();
        let witness = receiver.inspect(&bytes, &mut budget)?;
        result.push(Observation {
            phase: format!("overlap-{index}-replay"),
            work: budget.usage().work,
            seconds: start.elapsed().as_secs_f64(),
            accounting: format!("unconditional={}", witness.is_unconditional()),
        });
    }
    Ok(result)
}

/// Construct both complete current targets and evaluate all three original-source covariance maps.
fn vector() -> TestResult<Vec<Observation>> {
    let mut budget = super::budget()?;
    let start = Instant::now();
    let mut case = fixture::vector_case_with_budget::<f64>(&mut budget)?;
    let mut result = vec![Observation {
        phase: "vector-construction".into(),
        work: budget.usage().work,
        seconds: start.elapsed().as_secs_f64(),
        accounting: format!(
            "milestones={:?} caller_work_limit=16000000",
            case.construction
        ),
    }];
    let before = budget.usage().work;
    let start = Instant::now();
    let means = case.means(&mut budget)?;
    result.push(Observation {
        phase: "means".into(),
        work: budget.usage().work - before,
        seconds: start.elapsed().as_secs_f64(),
        accounting: "payload_bytes=192".into(),
    });
    let before = budget.usage().work;
    let start = Instant::now();
    let executions = case.executions(&means, &mut budget)?;
    result.push(Observation {
        phase: "planning".into(),
        work: budget.usage().work - before,
        seconds: start.elapsed().as_secs_f64(),
        accounting: "shared_definitions_counted_by_owner".into(),
    });
    evaluate(&case, &executions, &mut budget, &mut result)?;
    Ok(result)
}

/// Preserve separate lowering/arithmetic usage without treating retained allowances as float payloads.
fn evaluate(
    case: &fixture::Case<f64>,
    executions: &[nrz_matrix::CompanionExecution<f64>; 3],
    budget: &mut nrz_bindings::Budget,
    result: &mut Vec<Observation>,
) -> TestResult {
    let context = nrz_scalar::ArithmeticContext::new()?;
    for (index, execution) in executions.iter().enumerate() {
        let start = Instant::now();
        let value = case.heap.evaluate_companion(execution, &context, budget)?;
        let seconds = start.elapsed().as_secs_f64();
        let usage = value.usage();
        result.push(Observation { phase: format!("covariance-{index}"),
            work: usage.after.work - usage.before_lowering.work, seconds,
            accounting: format!("lowering_work={} arithmetic_work={} retained_allowance_bytes={} payload_bytes=64 cumulative_work={}",
                usage.before_evaluation.work - usage.before_lowering.work,
                usage.after.work - usage.before_evaluation.work, execution.retained_payload_bytes(), budget.usage().work) });
    }
    Ok(())
}
