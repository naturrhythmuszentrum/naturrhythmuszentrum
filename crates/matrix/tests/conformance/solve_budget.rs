// SPDX-License-Identifier: AGPL-3.0-only
//! Budget acceptance uses lower tolerance endpoints and preserves failed proof evidence.

use crate::kernel_testing::{execute, output};
use crate::solve_testing::{TestResult, draft};
use nrz_accuracy::{Budget, BudgetDecision};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError, RecipeEvidence,
    SolveBudget, SolveError, SolvePolicy,
};
use nrz_scalar::{Enclosure, Finite};
use nrz_units::Descriptor;

/// Absolute thresholds immediately below, at and above the available radius have distinct outcomes.
/// ´test:integration:solve-budget-boundaries-retain-over-budget-certificates´
#[test]
fn solve_budget_boundaries_retain_over_budget_certificates() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.125],
    )?;
    let rhs = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    let handles = vec![operator.value(), rhs.value()];
    let baseline = execute(&mut heap, draft(1, 1)?, handles.clone())?;
    crate::solve_public::check(&heap, output(&baseline)?, &[2.0], &[2.0], 1)?;
    let available = proof(&heap, baseline.record())?
        .output_radius()
        .ok_or("complete solve proof")?;
    for (threshold, decision) in [
        (
            f64::from_bits(available.to_bits() - 1),
            BudgetDecision::Exceeded,
        ),
        (available, BudgetDecision::Met),
        (f64::from_bits(available.to_bits() + 1), BudgetDecision::Met),
    ] {
        budget_attempt(&mut heap, &handles, available, threshold, decision)?;
    }
    Ok(())
}

/// An overlapping tolerance interval remains unresolved and cannot create a success handle.
/// ´test:integration:solve-budget-overlap-refuses-with-retained-evidence´
#[test]
fn solve_budget_overlap_refuses_with_retained_evidence() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.125],
    )?;
    let budget = Budget::from_enclosures(
        Enclosure::new(0.0, 10.0)?,
        Enclosure::singleton(Finite::ZERO),
        Enclosure::singleton(Finite::ONE),
    )?;
    let policy = SolvePolicy::with_budget(SolveBudget::frobenius(Descriptor::ratio(), budget)?);
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(1, 1)?.with_solve_policy(policy)?.check(limits)?,
        vec![source.value(); 2],
        limits,
    )
    .validate(&mut heap)?;
    let failure = bound
        .evaluate(bound.normalized_inputs()?)
        .err()
        .ok_or("overlap accepted")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Solve(SolveError::BudgetUnresolved)
    ));
    let RecipeEvidence::Solve(claim) = failure
        .record()
        .historical_evidence()
        .first()
        .ok_or("proof")?
    else {
        return Err("solve proof".into());
    };
    assert_eq!(
        claim.budget().ok_or("budget assessment")?.decision(),
        BudgetDecision::Unresolved
    );
    assert!(claim.total_radius().ok_or("complete solve proof")? > 0.0);
    Ok(())
}

/// Inspect the scoped solve evidence only through the current operation record.
fn proof(heap: &Heap, id: nrz_matrix::RecordId) -> TestResult<&nrz_matrix::SolveEvidence> {
    let RecipeEvidence::Solve(claim) = heap
        .operation_record(id)?
        .evidence()
        .first()
        .ok_or("proof")?
    else {
        return Err("solve proof".into());
    };
    Ok(claim)
}

/// Execute one threshold request and preserve a complete proof when its comparison refuses.
fn budget_attempt(
    heap: &mut Heap,
    handles: &[nrz_matrix::Handle<nrz_matrix::Value, f64>],
    available: f64,
    threshold: f64,
    decision: BudgetDecision,
) -> TestResult {
    let policy = SolvePolicy::with_budget(SolveBudget::frobenius(
        Descriptor::ratio(),
        Budget::new(threshold, 0.0, 1.0)?,
    )?);
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)?.with_solve_policy(policy)?.check(limits)?;
    let bound = Binding::new(operation, handles.to_vec(), limits).validate(heap)?;
    let evaluated = bound.evaluate(bound.normalized_inputs()?);
    if decision == BudgetDecision::Met {
        let outcome = bound.publish(evaluated?)?;
        crate::solve_public::check(heap, output(&outcome)?, &[2.0], &[2.0], 1)?;
        assert_eq!(
            proof(heap, outcome.record())?
                .budget()
                .ok_or("budget assessment")?
                .decision(),
            decision
        );
    } else {
        let failure = evaluated.err().ok_or("budget unexpectedly met")?;
        assert!(matches!(
            failure.cause(),
            OperationError::Solve(SolveError::BudgetExceeded)
        ));
        let RecipeEvidence::Solve(claim) = failure
            .record()
            .historical_evidence()
            .first()
            .ok_or("retained proof")?
        else {
            return Err("solve proof".into());
        };
        assert_eq!(
            claim.budget().ok_or("budget assessment")?.decision(),
            decision
        );
        assert_eq!(
            claim
                .output_radius()
                .ok_or("complete solve proof")?
                .to_bits(),
            available.to_bits()
        );
    }
    Ok(())
}
