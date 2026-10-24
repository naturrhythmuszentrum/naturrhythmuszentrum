// SPDX-License-Identifier: AGPL-3.0-only
//! Factor-construction receipts preserve the original operator and distinguish backend allowances.

use crate::solve_testing::{TestResult, draft};
use nrz_bindings::Descriptor;
use nrz_matrix::{Binding, ExecutionLimits, Extent, FactorAllocationStage, Heap, HeapLimits};
use nrz_scalar::Scalar;

/// Both formats retain original operator bindings and exact matrix-owned inverse/defect bytes.
/// ´test:integration:factor-allocation-inventory-retains-original-binding-and-bytes´
#[test]
fn factor_allocation_inventory_retains_original_binding_and_bytes() -> TestResult {
    inventory::<f32>(16, 6144)?;
    inventory::<f64>(32, 12_288)
}

/// Logical backend rectangles have exact entries, while their private padding remains an allowance.
fn inventory<P: Scalar + 'static>(vector_bytes: usize, backend: usize) -> TestResult {
    let factor = fixture::<P>(None)?.map_err(|e| format!("factor inventory execution: {e}"))?;
    let receipt = factor.allocation_inventory();
    assert_eq!(receipt.backend_allowance_bytes(), backend);
    assert_eq!(receipt.inline_factor_bytes(), 144);
    for (slot, allocation) in receipt.allocations().iter().enumerate() {
        assert_eq!(allocation.stage(), FactorAllocationStage::ALL[slot]);
        assert_eq!(allocation.entries(), 4);
        assert_eq!(allocation.logical_bytes(), vector_bytes);
        assert_eq!(allocation.dependency(), factor.dependency());
        assert_eq!(allocation.ticket().execution(), factor.execution());
        assert_eq!(allocation.coordinate(3), Some([1, 1]));
        assert!(allocation.coordinate(4).is_none());
        let expected = if allocation.stage().is_backend() {
            None
        } else {
            Some(vector_bytes)
        };
        assert_eq!(allocation.owned_bytes(), expected);
        assert!(
            receipt.allocations()[..slot]
                .iter()
                .all(|other| other.ticket() != allocation.ticket())
        );
    }
    assert_eq!(
        (
            receipt.metadata_bytes(),
            receipt.planning_bytes(),
            receipt.receipt_bytes()
        ),
        (792, 432, 360),
        "aligned factor plan and inline receipt metadata"
    );
    Ok(())
}

/// Factor attempts preserve typed numerical refusals separately from fixture construction errors.
type FactorAttempt<P> = Result<nrz_matrix::FactorRef<P>, nrz_matrix::Failure<P>>;

/// Complete public factor construction keeps its exact binding alive after the origin lease ends.
fn fixture<P: Scalar + 'static>(
    budget: Option<&nrz_bindings::Budget>,
) -> TestResult<FactorAttempt<P>> {
    let mut heap =
        Heap::new(HeapLimits::default()).map_err(|e| format!("factor inventory heap: {e}"))?;
    let square = Extent::new(2, 2).map_err(|e| format!("factor inventory square: {e}"))?;
    let column = Extent::new(2, 1).map_err(|e| format!("factor inventory column: {e}"))?;
    let operator = heap
        .import_represented(
            square,
            &Descriptor::ratio(),
            &[P::from_u8(2), P::ZERO, P::ZERO, P::from_u8(3)],
        )
        .map_err(|e| format!("factor inventory operator: {e}"))?;
    let rhs = heap
        .import_represented(column, &Descriptor::ratio(), &[P::ONE, P::from_u8(2)])
        .map_err(|e| format!("factor inventory rhs: {e}"))?;
    let limits = ExecutionLimits::default();
    let operation = draft(2, 1)
        .map_err(|e| format!("factor inventory draft: {e}"))?
        .check(limits)
        .map_err(|e| format!("factor inventory check: {e}"))?;
    let bound = Binding::new(operation, vec![operator.value(), rhs.value()], limits)
        .validate(&mut heap)
        .map_err(|e| format!("factor inventory admission: {e}"))?;
    Ok(budget.map_or_else(
        || bound.factor(),
        |budget| bound.factor_with_inventory_budget(budget),
    ))
}

/// Both existing byte ceilings admit the exact new metadata and refuse one byte less before backend work.
/// ´test:integration:factor-allocation-inventory-metadata-boundaries´
#[test]
fn factor_allocation_inventory_metadata_boundaries() -> TestResult {
    for staged in [false, true] {
        for bytes in [792, 791] {
            let defaults = nrz_bindings::Limits::default();
            let limits = nrz_bindings::Limits {
                staged_payload_bytes: if staged {
                    bytes
                } else {
                    defaults.staged_payload_bytes
                },
                output_bytes: if staged { defaults.output_bytes } else { bytes },
                ..defaults
            };
            let budget = nrz_bindings::Budget::new(limits)
                .map_err(|e| format!("factor metadata policy: {e}"))?;
            let result = fixture::<f64>(Some(&budget))?;
            assert_eq!(result.is_ok(), bytes == 792);
            if let Err(failure) = result {
                assert!(matches!(
                    failure.cause(),
                    nrz_matrix::OperationError::Exact(_)
                ));
            }
        }
    }
    Ok(())
}
