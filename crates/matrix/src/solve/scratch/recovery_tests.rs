// SPDX-License-Identifier: AGPL-3.0-only
//! A deliberately poor initial iterate exercises every finite correction allocation through publication.

use super::super::{Attempt, Run, Stage};
use crate::solve_testing::{TestResult, draft};
use crate::{Binding, Candidate, ExecutionLimits, Extent, Heap, HeapLimits, RecipeEvidence};
use nrz_bindings::Descriptor;

/// Correction RHS, backend copies, iterate update and every retained certificate copy stay covered.
/// ´test:unit:solve-correction-inventory-publishes-complete-attempt-prefix´
#[test]
fn solve_correction_inventory_publishes_complete_attempt_prefix() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let bound = binding(&mut heap)?;
    let candidate = corrected_candidate(&bound)?;
    let outcome = bound.publish(candidate)?;
    let receipt = heap
        .operation_record(outcome.record())?
        .solve_inventory()
        .ok_or("receipt")?;
    assert_eq!(receipt.completed_attempts(), 2);
    assert_eq!(receipt.allocations().len(), 23);
    assert_eq!(
        receipt
            .allocations()
            .iter()
            .filter_map(crate::SolveAllocation::owned_bytes)
            .sum::<usize>(),
        76
    );
    assert_eq!(receipt.peak_payload_bytes(), 16);
    assert!(
        receipt
            .allocations()
            .iter()
            .all(|entry| entry.iteration().is_none_or(|iteration| iteration < 2))
    );
    let value = heap
        .value(outcome.output()[0].value())?
        .region()
        .get(0, 0)?;
    let error = heap
        .companion(outcome.output()[0].evaluation().ok_or("error")?)?
        .region()
        .get(0, 0)?;
    assert!(
        crate::kernel_oracle::Rational::new(1, 3)?.covered(f64::from(value), f64::from(error))?
    );
    Ok(())
}
/// A genuine foreign correction inventory is refused even with equal final numerical values.
/// ´test:unit:solve-correction-publication-refuses-foreign-inventory´
#[test]
fn solve_correction_publication_refuses_foreign_inventory() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let bound = binding(&mut heap)?;
    let mut candidate = corrected_candidate(&bound)?;
    let mut foreign_heap = Heap::new(HeapLimits::default())?;
    let foreign = binding(&mut foreign_heap)?;
    let mut foreign_candidate = corrected_candidate(&foreign)?;
    assert_ne!(candidate.execution, foreign_candidate.execution);
    let output = candidate.outputs.first_mut().ok_or("output")?;
    let other = foreign_candidate
        .outputs
        .first_mut()
        .ok_or("foreign output")?;
    std::mem::swap(&mut output.centers, &mut other.centers);
    let planned = bound.result_bindings().to_vec();
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("foreign correction admitted")?;
    assert!(matches!(
        failure.cause(),
        crate::OperationError::PrivateAllocation(crate::PrivateAllocationError::ForeignInventory)
    ));
    assert!(
        planned
            .iter()
            .all(|binding| binding.state() == crate::SymbolState::Unavailable)
    );
    assert_eq!(heap.descriptors.iter().count(), 2);
    Ok(())
}
/// Construct the same finite correction policy over independent numerical inputs.
fn binding(heap: &mut Heap) -> TestResult<crate::Bound<'_, f32>> {
    let operator = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f32])?;
    let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f32])?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)?
        .with_solve_policy(crate::recovery_testing::policy(
            0.001,
            2,
            nrz_accuracy::Promotion::Never,
            nrz_accuracy::Limits::new(3, 1_000_000),
        )?)?
        .check(limits)?;
    Ok(Binding::new(operation, vec![operator.value(), rhs.value()], limits).validate(heap)?)
}
/// The deliberately altered iterate uses the production resource handoff before publication.
fn corrected_candidate(bound: &crate::Bound<'_, f32>) -> TestResult<Candidate<f32>> {
    let token = crate::heap::identity::identity()?;
    let budget = nrz_bindings::Budget::new(nrz_bindings::Limits::default())?;
    let run = Run::new(bound, token, &budget)?;
    let factor = bound.factor()?;
    let (restored, evidence) = corrected(bound, &factor, &run)?;
    let reservation = *bound.output_reservations().first().ok_or("reservation")?;
    let crate::binding::NativeRecipe::Solve(contract) = &bound.operation.recipe else {
        return Err("contract".into());
    };
    let candidate = Candidate {
        execution: bound.execution(),
        token,
        revision: bound.operation.revision(),
        outputs: vec![restored.payload(reservation, &run)?],
        evidence,
        statistics: Vec::new(),
        private: None,
    };
    bound.retain_consumer(&factor, contract, token)?;
    bound.issued.set(Some(token));
    Ok(candidate)
}
/// Two independent certificates use copied iterates; only the corrected complete proof may publish.
fn corrected(
    bound: &crate::Bound<'_, f32>,
    factor: &crate::FactorRef<f32>,
    run: &Run,
) -> TestResult<(
    crate::solve::restoration::Restored<f32>,
    Vec<RecipeEvidence>,
)> {
    let crate::binding::NativeRecipe::Solve(contract) = &bound.operation.recipe else {
        return Err("contract".into());
    };
    let inputs = bound.normalized_inputs()?;
    let reservation = *bound.output_reservations().first().ok_or("reservation")?;
    let mut centers = crate::solve::evaluate::initial(contract, inputs.views(), factor, run)?;
    centers[0] = 0.25;
    let mut evidence = Vec::new();
    let first = Attempt { run, index: 0 };
    let retained = copy(&centers, first)?;
    let (restored, decision) = crate::solve::evaluate::certify(
        contract,
        inputs.views(),
        factor,
        retained,
        reservation,
        &mut evidence,
        first,
    )?;
    assert_eq!(decision, nrz_accuracy::BudgetDecision::Exceeded);
    drop(restored);
    let next = Attempt { run, index: 1 };
    centers = crate::solve::recovery::correction(contract, inputs.views(), factor, &centers, next)?;
    let retained = copy(&centers, next)?;
    evidence.clear();
    let (restored, decision) = crate::solve::evaluate::certify(
        contract,
        inputs.views(),
        factor,
        retained,
        reservation,
        &mut evidence,
        next,
    )?;
    assert_eq!(decision, nrz_accuracy::BudgetDecision::Met);
    let Some(RecipeEvidence::Solve(claim)) = evidence.first_mut() else {
        return Err("proof".into());
    };
    claim.factor = Some(factor.used_by(bound.execution()));
    Ok((restored, evidence))
}
/// Copy into a distinct planned owner instead of duplicating a permit or detaching a vector.
fn copy(values: &[f32], attempt: Attempt<'_>) -> TestResult<crate::private::Buffer<f32>> {
    let mut retained = attempt.buffer(Stage::RetainedCenters)?;
    for &value in values {
        retained.push(value)?;
    }
    Ok(retained)
}
