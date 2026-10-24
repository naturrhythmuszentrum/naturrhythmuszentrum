// SPDX-License-Identifier: AGPL-3.0-only
//! Public completed inventories retain original heterogeneous references and actual buffer owners.

use crate::reading_testing::TestResult;
use nrz_bindings::readings::{ReciprocalEntry, ReciprocalStage};
use nrz_matrix::{Binding, ExecutionLimits, Heap, HeapLimits, PrivateAllocation};
use nrz_scalar::Scalar;

/// Both formats account for every logical and primitive entry of the scoped numerical schedule.
/// ´test:integration:reciprocal-private-inventory-retains-exact-meanings-and-bytes´
#[test]
fn reciprocal_private_inventory_retains_exact_meanings_and_bytes() -> TestResult {
    inventory::<f32>(472, 424)?;
    inventory::<f64>(832, 736)
}

/// Each scalar format reuses the same checked declarations and existing independent numerical tests.
fn inventory<P: Scalar + 'static>(bytes: usize, peak: usize) -> TestResult {
    let (prepared, definition) =
        crate::reciprocal_testing::proved().map_err(|e| format!("inventory fixture: {e}"))?;
    let draft = crate::reciprocal_testing::draft(&prepared, &definition)
        .map_err(|e| format!("inventory draft: {e}"))?;
    let operation = draft
        .check(ExecutionLimits::default())
        .map_err(|e| format!("inventory operation: {e}"))?;
    let mut heap = Heap::new(HeapLimits::default()).map_err(|e| format!("inventory heap: {e}"))?;
    let values = [P::ONE / P::from_u8(4), P::ONE / P::from_u8(16)];
    let inputs = super::import(
        &mut heap,
        &definition,
        &values,
        &[P::ZERO; 2],
        &[P::ZERO; 2],
    )
    .map_err(|e| format!("inventory imports: {e}"))?;
    let bound = Binding::from_symbols(operation, inputs, ExecutionLimits::default())
        .validate(&mut heap)
        .map_err(|e| format!("inventory admission: {e}"))?;
    let execution = bound.execution();
    let views = bound
        .normalized_inputs()
        .map_err(|e| format!("inventory inputs: {e}"))?;
    let candidate = bound
        .evaluate(views)
        .map_err(|e| format!("inventory evaluation: {e}"))?;
    let outcome = bound
        .publish(candidate)
        .map_err(|e| format!("inventory publication: {e}"))?;
    let record = heap
        .operation_record(outcome.record())
        .map_err(|e| format!("inventory record: {e}"))?;
    let receipt = record
        .reciprocal_inventory()
        .ok_or("reciprocal inventory absent")?;
    assert_eq!(receipt.precision(), P::PRECISION);
    check(receipt, &definition, execution, bytes, peak)
}

/// Public receipts expose complete original meanings and independently counted allocation sizes.
fn check(
    receipt: &nrz_matrix::ReciprocalInventory,
    definition: &nrz_bindings::readings::PositiveReciprocalMap,
    execution: nrz_matrix::ExecutionId,
    bytes: usize,
    peak: usize,
) -> TestResult {
    assert_eq!(receipt.peak_payload_bytes(), peak);
    let entries = receipt.allocations();
    assert_eq!(
        entries.each_ref().map(PrivateAllocation::entries),
        [2, 8, 4, 2, 4, 4, 4, 4]
    );
    assert_eq!(
        entries
            .iter()
            .map(PrivateAllocation::float_entries)
            .sum::<usize>(),
        90
    );
    assert_eq!(
        entries
            .iter()
            .map(PrivateAllocation::owned_bytes)
            .sum::<usize>(),
        bytes
    );
    for (position, allocation) in entries.iter().enumerate() {
        assert_eq!(allocation.ticket().execution(), execution);
        assert!(
            entries[..position]
                .iter()
                .all(|other| other.ticket() != allocation.ticket())
        );
        let stage = ReciprocalStage::ALL[position];
        assert!(
            allocation
                .binding()
                .same_binding(&definition.allocation(stage))
        );
        assert!(allocation.binding().entry(allocation.entries()).is_none());
        meanings(allocation)?;
    }
    assert_eq!(
        (
            receipt.metadata_bytes(),
            receipt.planning_bytes(),
            receipt.candidate_metadata_bytes()
        ),
        (488, 624, 280),
        "aligned receipt, planning and candidate allocations"
    );
    Ok(())
}

/// Component interpretations point to the exact retained graph, never one invented array unit.
fn meanings(allocation: &PrivateAllocation) -> TestResult {
    let binding = allocation.binding();
    let definition = binding.definition();
    for index in 0..allocation.entries() {
        match binding
            .entry(index)
            .ok_or("missing planned entry meaning")?
        {
            ReciprocalEntry::Reference(meaning) => {
                assert_eq!(meaning, &definition.frame().entries()[index]);
                assert_ne!(meaning.descriptor(), &nrz_bindings::Descriptor::ratio());
            }
            ReciprocalEntry::Constant(value) => {
                let expected = if binding.stage() == ReciprocalStage::Coefficients {
                    &definition.map().coefficients()[index]
                } else {
                    &definition.map().offsets()[index]
                };
                assert_eq!(value, expected);
            }
            ReciprocalEntry::Image {
                input,
                expression,
                reference,
            } => {
                assert_eq!(input, &definition.inputs()[index]);
                assert_eq!(expression, &definition.reciprocals()[index]);
                assert_eq!(reference, &definition.frame().entries()[index]);
            }
            ReciprocalEntry::Output {
                expression,
                index: position,
            } => {
                assert_eq!(expression, definition.image().expression());
                assert_eq!(position, index);
            }
        }
    }
    Ok(())
}

/// Retained receipt ownership is charged to provenance before any output becomes visible.
/// ´test:integration:reciprocal-private-inventory-provenance-boundary´
#[test]
fn reciprocal_private_inventory_provenance_boundary() -> TestResult {
    let (prepared, definition) =
        crate::reciprocal_testing::proved().map_err(|e| format!("receipt budget fixture: {e}"))?;
    let operation = crate::reciprocal_testing::draft(&prepared, &definition)
        .map_err(|e| format!("receipt budget draft: {e}"))?
        .check(ExecutionLimits::default())
        .map_err(|e| format!("receipt budget operation: {e}"))?;
    let bytes =
        budget_attempt(&operation, &definition, None)?.ok_or("unlimited receipt budget refused")?;
    assert_eq!(
        budget_attempt(&operation, &definition, Some(bytes))?,
        Some(bytes)
    );
    assert_eq!(
        budget_attempt(&operation, &definition, Some(bytes - 1))?,
        None
    );
    Ok(())
}

/// All non-byte limits remain at their existing default values for the exact N and N-minus-one pair.
fn budget_attempt(
    operation: &nrz_matrix::CheckedOperation,
    definition: &nrz_bindings::readings::PositiveReciprocalMap,
    ceiling: Option<usize>,
) -> TestResult<Option<usize>> {
    let mut heap =
        Heap::new(HeapLimits::default()).map_err(|e| format!("receipt budget heap: {e}"))?;
    if let Some(bytes) = ceiling {
        heap.set_provenance_limits(nrz_matrix::ProvenanceLimits::new(
            1024, 16_384, 65_536, bytes, 1_048_576,
        ))
        .map_err(|e| format!("receipt budget policy: {e}"))?;
    }
    let inputs = super::import(
        &mut heap,
        definition,
        &[0.25_f64, 0.0625],
        &[0.0; 2],
        &[0.0; 2],
    )
    .map_err(|e| format!("receipt budget imports: {e}"))?;
    let bound = Binding::from_symbols(operation.clone(), inputs, ExecutionLimits::default())
        .validate(&mut heap)
        .map_err(|e| format!("receipt budget admission: {e}"))?;
    let planned = bound.result_bindings().to_vec();
    let views = bound
        .normalized_inputs()
        .map_err(|e| format!("receipt budget views: {e}"))?;
    let candidate = bound
        .evaluate(views)
        .map_err(|e| format!("receipt budget evaluation: {e}"))?;
    match bound.publish(candidate) {
        Ok(_) => Ok(Some(heap.provenance_usage().bytes())),
        Err(failure) => {
            assert!(matches!(
                failure.cause(),
                nrz_matrix::OperationError::ProvenanceLimit
            ));
            assert_eq!(
                heap.provenance_usage(),
                nrz_matrix::ProvenanceUsage::default()
            );
            assert!(
                planned
                    .iter()
                    .all(|binding| binding.state() == nrz_matrix::SymbolState::Unavailable)
            );
            Ok(None)
        }
    }
}
