// SPDX-License-Identifier: AGPL-3.0-only
//! Integrated refusal paths retain exact stage, costs and historical numerical evidence.

use super::{Case, ResultOf, capture, model, shapes};
use nrz_accuracy::{Budget, Limits, Promotion, RecoveryPolicy};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, ShapeLimits, SolveBudget,
    SolvePolicy,
};
use nrz_scalar::Scalar;
use std::time::Instant;

/// A nonsingular rational system succeeds near the inverse witness's precision boundary, then refuses.
pub(crate) fn edge() -> ResultOf<Case> {
    let started = Instant::now();
    let mut result = Case::new("certificate-edge");
    let mut heap = Heap::new(HeapLimits::default())?;
    let (_, [matrix, right, solution]) = shapes::normalized(2, 1)?;
    let draft = OperationDraft::solve(matrix, right, solution, ShapeLimits::default())?;
    let limits = ExecutionLimits::default();
    for (last, accepted) in [
        (1.0 + 1.0 / 1_048_576.0, true),
        (Scalar::next_up(1.0_f64), false),
    ] {
        let operator = heap.import_represented(
            Extent::new(2, 2)?,
            &Descriptor::ratio(),
            &[1.0, 1.0, 1.0, last],
        )?;
        let rhs = heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0, 1.0])?;
        result.inputs(&heap, &[operator, rhs])?;
        let outcome = heap.solve_recorded(Binding::new(
            draft.clone().check(limits)?,
            vec![operator.value(), rhs.value()],
            limits,
        ));
        if accepted {
            result
                .samples
                .push(capture::sample(&heap, &outcome?, "near-edge", 0)?);
            result.factors += 1;
        } else {
            let failure = outcome
                .err()
                .ok_or("certificate edge unexpectedly published")?;
            result.refusals.push(capture::refusal(&failure));
        }
    }
    result.finish(&heap);
    result
        .timings
        .push(("total", started.elapsed().as_secs_f64()));
    Ok(result)
}

/// Shared A and B sources require the negative cross terms of the implicit derivative law.
pub(crate) fn shared() -> ResultOf<Case> {
    let started = Instant::now();
    let mut result = Case::new("shared-a-b");
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0])?;
    let rhs = heap.import_represented(Extent::new(1, 2)?, &Descriptor::ratio(), &[2.0, 4.0])?;
    model::attach(
        &mut heap,
        [operator.value(), rhs.value()],
        &model::Law::Factor(&[1.0, 3.0, 1.0], 0.0625),
    )?;
    result.inputs(&heap, &[operator, rhs])?;
    let (_, [matrix, right, solution]) = shapes::normalized(1, 2)?;
    let draft = OperationDraft::solve(matrix, right, solution, ShapeLimits::default())?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let work = bound.statistical_work();
    let factor = bound.factor()?;
    result.factors += 1;
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let outcome = bound.publish(candidate)?;
    result.samples.push(capture::sample(
        &heap,
        &outcome,
        "shared-operator-rhs",
        work,
    )?);
    result.finish(&heap);
    result
        .timings
        .push(("total", started.elapsed().as_secs_f64()));
    Ok(result)
}

/// A changed operator invalidates both historical output authority and the factor's next use.
pub(crate) fn stale() -> ResultOf<Case> {
    let started = Instant::now();
    let mut result = Case::new("stale-factor");
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[3.0, 1.0, 1.0, 2.0],
    )?;
    let rhs = heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[5.0, 5.0])?;
    result.inputs(&heap, &[operator, rhs])?;
    let (_, [matrix, right, solution]) = shapes::normalized(2, 1)?;
    let limits = ExecutionLimits::default();
    let operation =
        OperationDraft::solve(matrix, right, solution, ShapeLimits::default())?.check(limits)?;
    let inputs = vec![operator.value(), rhs.value()];
    let bound = Binding::new(operation.clone(), inputs.clone(), limits).validate(&mut heap)?;
    let factor = bound.factor()?;
    result.factors += 1;
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let outcome = bound.publish(candidate)?;
    result.samples.push(capture::sample(
        &heap,
        &outcome,
        "before-operator-replacement",
        0,
    )?);
    let version = heap.value(operator.value())?.stamp().version();
    heap.replace_represented(operator.value(), version, &[4.0, 1.0, 1.0, 2.0])?;
    if heap.operation_record(outcome.record()).is_ok() {
        return Err("stale record retained current authority".into());
    }
    heap.historical_record(outcome.record())?;
    let failure = heap
        .solve_factored_recorded(Binding::new(operation, inputs, limits), &factor)
        .err()
        .ok_or("stale factor published")?;
    result.refusals.push(capture::refusal(&failure));
    result.finish(&heap);
    result
        .timings
        .push(("total", started.elapsed().as_secs_f64()));
    Ok(result)
}

/// A zero budget for 1/3 cannot be met by a binary candidate; bounded corrections must stop.
pub(crate) fn stagnation() -> ResultOf<Case> {
    let started = Instant::now();
    let mut result = Case::new("bounded-refinement");
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0])?;
    let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0])?;
    result.inputs(&heap, &[operator, rhs])?;
    let (_, [matrix, right, solution]) = shapes::normalized(1, 1)?;
    let policy = SolvePolicy::with_budget(SolveBudget::frobenius(
        Descriptor::ratio(),
        Budget::new(0.0, 0.0, 1.0)?,
    )?)
    .with_recovery(
        RecoveryPolicy::new(8, 1, Promotion::Never)?,
        Limits::new(9, 2304),
    );
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::solve(matrix, right, solution, ShapeLimits::default())?
        .with_solve_policy(policy)?
        .check(limits)?;
    let failure = heap
        .solve_recorded(Binding::new(
            operation,
            vec![operator.value(), rhs.value()],
            limits,
        ))
        .err()
        .ok_or("zero-budget last iterate published")?;
    result.factors = 1;
    result.refusals.push(capture::refusal(&failure));
    result.finish(&heap);
    result
        .timings
        .push(("total", started.elapsed().as_secs_f64()));
    Ok(result)
}
