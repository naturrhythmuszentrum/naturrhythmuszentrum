// SPDX-License-Identifier: AGPL-3.0-only
//! Every native input, verifier and final numerical owner is admitted before construction.

use super::PhasorAllocationStage as Stage;
use super::{AT, Contents, Heap, PhasorError, PhasorSolve, PhasorSolveEvaluation};
use super::{constants, inventory::Run, numeric, residual, transfer};
use nrz_bindings::{
    Budget,
    readings::algebra::limits::{BudgetKind, BudgetUsage},
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Complete the numerical prefix before publishing an immutable evaluation or any heap reading.
pub(super) fn evaluate<P: Scalar + 'static>(
    heap: &Heap,
    plan: &PhasorSolve<P>,
    run: &Run<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<PhasorSolveEvaluation<P>, PhasorError> {
    plan.validate(heap)?;
    let before = budget.usage();
    let bytes = plan
        .retained_payload_bytes()
        .checked_add(4096)
        .ok_or(crate::HeapError::ExtentOverflow)?;
    let (contents, coverage, lowered) = budget.with_retained_payload(bytes, AT, |budget| {
        let result = compute(heap, plan, run, context, budget)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok::<_, PhasorError>(result)
    })?;
    let inventory = run.complete(&contents)?;
    Ok(PhasorSolveEvaluation {
        plan: plan.clone(),
        contents: Rc::new(contents),
        inventory,
        coverage,
        revision: context.revision(),
        usage: [before, lowered, budget.usage()],
    })
}
/// Inputs and transient arrays leave scope before the complete live-owner coverage check.
fn compute<P: Scalar + 'static>(
    heap: &Heap,
    plan: &PhasorSolve<P>,
    run: &Run<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(Contents<P>, [crate::Coverage; 2], BudgetUsage), PhasorError> {
    let references = run.inline(Stage::References, || transfer::lower(plan, context, budget))?;
    let fixed = run.vector(Stage::FixedInputs, || {
        constants::lower(plan, context, budget)
    })?;
    let lowered = budget.usage();
    let (impedance, operator_scope) = run.capture(heap, 0, budget)?;
    let (voltage, voltage_scope) = run.capture(heap, 1, budget)?;
    let present = plan
        .definition()
        .fixed_inputs()
        .each_ref()
        .map(|operand| operand.each_ref().map(Option::is_some));
    constants::check(&[&impedance, &voltage], &fixed, present, context, budget)?;
    let reactance = numeric::domain(&impedance, context, budget)?;
    let transfer = run.inline(Stage::Transfer, || {
        transfer::evaluate(plan, references[0], reactance, context, budget)
    })?;
    let (payload, certificate) = numeric::solve(&impedance, &voltage, run, context, budget)?;
    let residual = residual::evaluate(
        plan,
        &[&impedance, &voltage],
        &payload.centers,
        &references,
        run,
        context,
        budget,
    )?;
    Ok((
        Contents {
            payload,
            certificate,
            transfer,
            residual,
        },
        [operator_scope, voltage_scope],
        lowered,
    ))
}
