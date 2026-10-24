// SPDX-License-Identifier: AGPL-3.0-only
//! Public deterministic receipts retain original input meanings and exact observable capacities.

use crate::solve_testing::{TestResult, draft};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, SolveAllocationStage as Stage,
};
use nrz_scalar::Scalar;

/// Both formats distinguish backend logical requests from independently observable vector bytes.
/// ´test:integration:solve-array-inventory-preserves-original-sources-and-capacities´
#[test]
fn solve_array_inventory_preserves_original_sources_and_capacities() -> TestResult {
    inventory::<f32>(80, 32)?;
    inventory::<f64>(160, 64)
}
/// The two-by-one physical solution has eleven requests and a four-entry operator-radius selection.
fn inventory<P: Scalar + 'static>(bytes: usize, peak: usize) -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[P::from_u8(2), P::ZERO, P::ZERO, P::from_u8(3)],
    )?;
    let rhs = heap.import_represented(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[P::ONE, P::from_u8(2)],
    )?;
    let original = heap.value(operator.value())?.stamp();
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(2, 1)?.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let execution = bound.execution();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let receipt = heap
        .operation_record(outcome.record())?
        .solve_inventory()
        .ok_or("solve inventory")?;
    assert_eq!(receipt.sources()[0].0.stamp(), original);
    assert_eq!(receipt.sources().len(), 2);
    assert_eq!(receipt.allocations().len(), 11);
    assert_eq!(
        receipt
            .allocations()
            .iter()
            .map(nrz_matrix::SolveAllocation::entries)
            .sum::<usize>(),
        24
    );
    assert_eq!(
        receipt
            .allocations()
            .iter()
            .filter_map(nrz_matrix::SolveAllocation::owned_bytes)
            .sum::<usize>(),
        bytes
    );
    assert_eq!(receipt.peak_payload_bytes(), peak);
    assert_eq!(receipt.precision(), P::PRECISION);
    for (slot, allocation) in receipt.allocations().iter().enumerate() {
        assert_eq!(allocation.ticket().execution(), execution);
        assert_eq!(allocation.entries(), allocation.float_entries());
        assert_eq!(
            allocation.owned_bytes().is_none(),
            allocation.stage().is_backend()
        );
        assert!(
            receipt.allocations()[..slot]
                .iter()
                .all(|prior| prior.ticket() != allocation.ticket())
        );
        assert_eq!(
            allocation.extent().columns(),
            if allocation.stage() == Stage::OperatorRadii {
                2
            } else {
                1
            }
        );
    }
    assert_eq!(
        (
            receipt.metadata_bytes(),
            receipt.planning_bytes(),
            receipt.source_bytes(),
            receipt.receipt_bytes(),
            receipt.candidate_metadata_bytes()
        ),
        (2_352, 1_080, 1_024, 248, 280)
    );
    Ok(())
}

/// Explicit caller byte ceilings admit the owned schedule and receipt exactly, before RHS allocation.
/// ´test:integration:solve-array-inventory-metadata-boundaries´
#[test]
fn solve_array_inventory_metadata_boundaries() -> TestResult {
    let bytes = budget_attempt(None)?.ok_or("default inventory")?;
    assert_eq!(bytes, 2_352);
    for staged in [false, true] {
        assert_eq!(budget_attempt(Some((staged, bytes)))?, Some(bytes));
        assert_eq!(budget_attempt(Some((staged, bytes - 1)))?, None);
    }
    Ok(())
}
/// Factor construction has its own existing budget; the supplied policy governs this RHS inventory.
fn budget_attempt(ceiling: Option<(bool, usize)>) -> TestResult<Option<usize>> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(draft(1, 1)?.check(limits)?, vec![source.value(); 2], limits)
        .validate(&mut heap)?;
    let factor = bound.factor()?;
    let mut policy = nrz_bindings::Limits::default();
    if let Some((staged, bytes)) = ceiling {
        if staged {
            policy.staged_payload_bytes = u64::try_from(bytes)?;
        } else {
            policy.output_bytes = u64::try_from(bytes)?;
        }
    }
    let budget = nrz_bindings::Budget::new(policy)?;
    let result = bound.evaluate_factored_with_inventory_budget(
        &bound.normalized_inputs()?,
        &factor,
        &budget,
    );
    match result {
        Ok(candidate) => {
            let outcome = bound.publish(candidate)?;
            Ok(Some(
                heap.operation_record(outcome.record())?
                    .solve_inventory()
                    .ok_or("receipt")?
                    .metadata_bytes(),
            ))
        }
        Err(failure) => {
            assert!(matches!(
                failure.cause(),
                nrz_matrix::OperationError::Exact(_)
            ));
            drop(bound);
            assert_eq!(heap.provenance_usage().records(), 0);
            Ok(None)
        }
    }
}
