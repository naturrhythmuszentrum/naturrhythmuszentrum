// SPDX-License-Identifier: AGPL-3.0-only
//! Deliberate private corruption cannot manufacture allocation authority or publish a prefix.

use super::{Run, State};
use crate::reading_testing::TestResult;
use crate::{
    Binding, ExecutionId, ExecutionLimits, Extent, Heap, HeapLimits, OperationError,
    PrivateAllocationError,
};
use nrz_bindings::readings::ReciprocalStage;
use nrz_scalar::DecodedLiteral;

/// A stage is admitted only for its original exact declaration and concrete representation.
/// ´test:unit:private-allocation-rejects-unplanned-meaning-and-representation´
#[test]
fn private_allocation_rejects_unplanned_meaning_and_representation() -> TestResult {
    let (_, definition) =
        crate::reciprocal_testing::proved().map_err(|e| format!("allocation definition: {e}"))?;
    let (_, foreign) = crate::reciprocal_testing::proved()
        .map_err(|e| format!("foreign allocation definition: {e}"))?;
    let run = Run::new::<f64>(ExecutionId(1), 2, &definition)
        .map_err(|e| format!("private plan: {e}"))?;
    assert!(matches!(
        run.grant::<DecodedLiteral<f64>>(&foreign.allocation(ReciprocalStage::References)),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::UnplannedMeaning
        ))
    ));
    assert!(matches!(
        run.buffer::<f64>(ReciprocalStage::References),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::Representation
        ))
    ));
    let original = run
        .buffer::<DecodedLiteral<f64>>(ReciprocalStage::References)
        .map_err(|e| format!("first allocation: {e}"))?;
    assert!(matches!(
        run.buffer::<DecodedLiteral<f64>>(ReciprocalStage::References),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::DuplicateTicket
        ))
    ));
    drop(original);
    assert!(matches!(
        run.buffer::<DecodedLiteral<f64>>(ReciprocalStage::References),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::DuplicateTicket
        ))
    ));
    Ok(())
}

/// A completely finite copied vector has no authority from the original vector's ticket.
/// ´test:unit:private-allocation-rejects-unticketed-candidate-array´
#[test]
fn private_allocation_rejects_unticketed_candidate_array() -> TestResult {
    let (mut heap, operation, inputs) = fixture()?;
    let bound = Binding::new(operation, inputs, ExecutionLimits::default())
        .validate(&mut heap)
        .map_err(|e| format!("unticketed admission: {e}"))?;
    let planned = bound.result_bindings().to_vec();
    let views = bound
        .normalized_inputs()
        .map_err(|e| format!("unticketed inputs: {e}"))?;
    let mut candidate = bound
        .evaluate(views)
        .map_err(|e| format!("unticketed evaluation: {e}"))?;
    let payload = candidate.outputs.first_mut().ok_or("unticketed payload")?;
    let replacement = Vec::from(payload.centers.as_slice());
    let original = std::mem::replace(&mut payload.centers, replacement.into());
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("unticketed publication succeeded")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(PrivateAllocationError::UnticketedBuffer)
    ));
    assert!(
        planned
            .iter()
            .all(|binding| binding.state() == crate::SymbolState::Unavailable)
    );
    assert_eq!(heap.descriptors.iter().count(), 2);
    drop(original);
    Ok(())
}

/// Both absent authority and an uncompleted planned stage refuse before the publication commit.
/// ´test:unit:private-allocation-requires-complete-publication-coverage´
#[test]
fn private_allocation_requires_complete_publication_coverage() -> TestResult {
    for absent in [false, true] {
        let (mut heap, operation, inputs) = fixture()?;
        let bound = Binding::new(operation, inputs, ExecutionLimits::default())
            .validate(&mut heap)
            .map_err(|e| format!("incomplete admission: {e}"))?;
        let views = bound
            .normalized_inputs()
            .map_err(|e| format!("incomplete inputs: {e}"))?;
        let mut candidate = bound
            .evaluate(views)
            .map_err(|e| format!("incomplete evaluation: {e}"))?;
        if absent {
            candidate.private = None;
        } else {
            candidate
                .private
                .as_ref()
                .ok_or("incomplete inventory")?
                .0
                .specifications[0]
                .state
                .set(State::Planned);
        }
        let failure = bound
            .publish(candidate)
            .err()
            .ok_or("incomplete publication succeeded")?;
        assert!(matches!(
            failure.cause(),
            OperationError::PrivateAllocation(PrivateAllocationError::IncompleteCoverage)
        ));
        assert_eq!(heap.descriptors.iter().count(), 2);
    }
    Ok(())
}

/// Bounded initialization cannot grow a buffer beyond its exact planned entry universe.
/// ´test:unit:private-allocation-appends-cannot-grow-unplanned-storage´
#[test]
fn private_allocation_appends_cannot_grow_unplanned_storage() -> TestResult {
    let (_, definition) =
        crate::reciprocal_testing::proved().map_err(|e| format!("append definition: {e}"))?;
    let run =
        Run::new::<f64>(ExecutionId(3), 4, &definition).map_err(|e| format!("append plan: {e}"))?;
    let mut values = run
        .buffer::<f64>(ReciprocalStage::Centers)
        .map_err(|e| format!("append allocation: {e}"))?;
    for _ in 0..4 {
        values
            .push(0.0)
            .map_err(|e| format!("planned append: {e}"))?;
    }
    assert!(matches!(
        values.push(0.0),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::IncompleteCoverage
        ))
    ));
    assert_eq!(values.len(), 4);
    Ok(())
}

/// An inventory from a different attempt cannot authorize otherwise valid equal-shaped payloads.
/// ´test:unit:private-allocation-rejects-foreign-inventory´
#[test]
fn private_allocation_rejects_foreign_inventory() -> TestResult {
    let (mut heap, operation, inputs) = fixture()?;
    let definition = operation
        .reciprocal_map()
        .ok_or("foreign inventory definition")?
        .clone();
    let bound = Binding::new(operation, inputs, ExecutionLimits::default())
        .validate(&mut heap)
        .map_err(|e| format!("foreign inventory admission: {e}"))?;
    let views = bound
        .normalized_inputs()
        .map_err(|e| format!("foreign inventory inputs: {e}"))?;
    let mut candidate = bound
        .evaluate(views)
        .map_err(|e| format!("foreign inventory evaluation: {e}"))?;
    let token = candidate
        .token
        .checked_add(1)
        .ok_or("foreign inventory token")?;
    candidate.private = Some(
        Run::new::<f64>(bound.execution(), token, &definition)
            .map_err(|e| format!("foreign inventory plan: {e}"))?,
    );
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("foreign inventory publication succeeded")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(PrivateAllocationError::ForeignInventory)
    ));
    assert_eq!(heap.descriptors.iter().count(), 2);
    Ok(())
}

/// Equal-sized channels still have distinct allocating bindings and cannot exchange permits.
/// ´test:unit:private-allocation-rejects-swapped-channel-permits´
#[test]
fn private_allocation_rejects_swapped_channel_permits() -> TestResult {
    let (mut heap, operation, inputs) = fixture()?;
    let bound = Binding::new(operation, inputs, ExecutionLimits::default())
        .validate(&mut heap)
        .map_err(|e| format!("swapped admission: {e}"))?;
    let views = bound
        .normalized_inputs()
        .map_err(|e| format!("swapped inputs: {e}"))?;
    let mut candidate = bound
        .evaluate(views)
        .map_err(|e| format!("swapped evaluation: {e}"))?;
    let payload = candidate.outputs.first_mut().ok_or("swapped payload")?;
    std::mem::swap(&mut payload.bounds, &mut payload.evaluation);
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("swapped publication succeeded")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(PrivateAllocationError::UnticketedBuffer)
    ));
    assert_eq!(heap.descriptors.iter().count(), 2);
    Ok(())
}

/// Complete shared fixture output without a borrowed execution lease.
type Fixture = (
    Heap,
    crate::CheckedOperation,
    Vec<crate::Handle<crate::Value, f64>>,
);

/// Shared public declarations build both the unit corruption cases and the integration lifecycle.
fn fixture() -> TestResult<Fixture> {
    let (prepared, definition) =
        crate::reciprocal_testing::proved().map_err(|e| format!("private fixture: {e}"))?;
    let draft = crate::reciprocal_testing::draft(&prepared, &definition)
        .map_err(|e| format!("private draft: {e}"))?;
    let operation = draft
        .check(ExecutionLimits::default())
        .map_err(|e| format!("private operation: {e}"))?;
    let mut heap = Heap::new(HeapLimits::default()).map_err(|e| format!("private heap: {e}"))?;
    let mut inputs = Vec::new();
    for (meaning, center) in definition.frame().entries().iter().zip([0.25_f64, 0.0625]) {
        inputs.push(
            heap.import_represented(Extent::new(1, 1)?, meaning.descriptor(), &[center])
                .map_err(|e| format!("private import: {e}"))?
                .value(),
        );
    }
    Ok((heap, operation, inputs))
}
