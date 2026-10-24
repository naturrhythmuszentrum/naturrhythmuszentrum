// SPDX-License-Identifier: AGPL-3.0-only
//! Resource survival, exact ticket association and rollback exercise actual shared factors.

use crate::solve_testing::{TestResult, draft};
use crate::{
    Binding, CheckedOperation, ExecutionLimits, Extent, Handle, Heap, HeapError, HeapLimits, Value,
};
use nrz_bindings::Descriptor;
use nrz_scalar::Scalar;
use std::rc::Rc;

/// One checked operator revision is reused across all independent executions.
struct Case<P: Scalar> {
    /// Numerical heap with stable original inputs.
    heap: Heap,
    /// Clones preserve the exact original operator revision.
    operation: CheckedOperation,
    /// Original operator and RHS handles in checked operand order.
    inputs: Vec<Handle<Value, P>>,
}
impl<P: Scalar + 'static> Case<P> {
    /// Small positive nonsingular system with exact represented entries in both formats.
    fn new() -> TestResult<Self> {
        let mut heap = Heap::new(HeapLimits::default())?;
        let operator = heap.import_represented(
            Extent::new(2, 2)?,
            &Descriptor::ratio(),
            &[P::from_u8(3), P::ONE, P::ONE, P::from_u8(2)],
        )?;
        let rhs = heap.import_represented(
            Extent::new(2, 1)?,
            &Descriptor::ratio(),
            &[P::from_u8(5), P::from_u8(5)],
        )?;
        Ok(Self {
            heap,
            operation: draft(2, 1)?.check(ExecutionLimits::default())?,
            inputs: vec![operator.value(), rhs.value()],
        })
    }
    /// Every lease has a fresh identity while preserving the original operator declaration.
    fn bound(&mut self) -> TestResult<crate::Bound<'_, P>> {
        Ok(Binding::new(
            self.operation.clone(),
            self.inputs.clone(),
            ExecutionLimits::default(),
        )
        .validate(&mut self.heap)?)
    }
}

/// An unpublished origin drops its share while the external factor remains independently usable.
/// ´test:unit:execution-abandonment-preserves-external-factor´
#[test]
fn execution_abandonment_preserves_external_factor() -> TestResult {
    abandonment::<f32>()?;
    abandonment::<f64>()
}
/// Both formats prove that historical receipts do not keep the final numerical owner alive.
fn abandonment<P: Scalar + 'static>() -> TestResult {
    let mut case = Case::<P>::new()?;
    let bound = case.bound()?;
    let factor = bound.factor()?;
    let historical = factor.allocation_inventory().clone();
    let weak = Rc::downgrade(&factor.resource);
    drop(bound);
    assert!(weak.upgrade().is_some());
    let bound = case.bound()?;
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let outcome = bound.publish(candidate)?;
    let receipt = case.heap.execution_inventory(outcome.record())?.clone();
    drop(factor);
    assert!(weak.upgrade().is_some());
    case.heap.retire_execution(outcome.record())?;
    assert!(weak.upgrade().is_none());
    assert_eq!(
        receipt.factors()[0].ticket(),
        historical.allocations()[1].ticket()
    );
    assert_eq!(
        receipt.factors()[0]
            .allocation_inventory()
            .allocations()
            .len(),
        6
    );
    Ok(())
}

/// Origin retirement preserves both an external factor and an independently completed consumer.
/// ´test:unit:execution-retirement-preserves-other-consumers´
#[test]
fn execution_retirement_preserves_other_consumers() -> TestResult {
    surviving_consumer::<f32>()?;
    surviving_consumer::<f64>()
}
/// Each execution releases only its own independent strong share.
fn surviving_consumer<P: Scalar + 'static>() -> TestResult {
    let mut case = Case::<P>::new()?;
    let bound = case.bound()?;
    let factor = bound.factor()?;
    let weak = Rc::downgrade(&factor.resource);
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let origin = bound.publish(candidate)?;
    let bound = case.bound()?;
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let consumer = bound.publish(candidate)?;
    let historical = case.heap.execution_inventory(origin.record())?.clone();
    let reused = case.heap.execution_inventory(consumer.record())?;
    assert_eq!(historical.factors().len(), 1);
    assert_eq!(reused.factors().len(), 1);
    assert_eq!(
        historical.factors()[0].ticket(),
        reused.factors()[0].ticket()
    );
    assert_eq!(
        reused.backend_allowance_bytes(),
        factor.allocation_inventory().backend_allowance_bytes()
    );
    case.heap.retire_execution(origin.record())?;
    assert!(weak.upgrade().is_some());
    let bound = case.bound()?;
    bound.validate_factor(&factor)?;
    drop(bound);
    drop(factor);
    assert!(weak.upgrade().is_some());
    case.heap.retire_execution(consumer.record())?;
    assert!(weak.upgrade().is_none());
    assert_eq!(historical.factors().len(), 1);
    Ok(())
}

/// A foreign record or repeated retirement cannot release another execution's resource.
/// ´test:unit:execution-retirement-authenticates-record-owner´
#[test]
fn execution_retirement_authenticates_record_owner() -> TestResult {
    let mut first = Case::<f64>::new()?;
    let bound = first.bound()?;
    let factor = bound.factor()?;
    let weak = Rc::downgrade(&factor.resource);
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let outcome = bound.publish(candidate)?;
    drop(factor);
    let mut foreign = Case::<f64>::new()?;
    assert!(matches!(
        foreign.heap.retire_execution(outcome.record()),
        Err(HeapError::ForeignHandle)
    ));
    assert!(weak.upgrade().is_some());
    first.heap.retire_execution(outcome.record())?;
    assert!(weak.upgrade().is_none());
    assert!(matches!(
        first.heap.retire_execution(outcome.record()),
        Err(HeapError::StaleHandle)
    ));
    assert!(first.heap.historical_record(outcome.record()).is_ok());
    assert!(first.heap.value(outcome.output()[0].value()).is_ok());
    Ok(())
}

/// Publication failure drops its own origin share without changing a completed consumer.
/// ´test:unit:execution-failed-publication-rolls-back-only-its-share´
#[test]
fn execution_failed_publication_rolls_back_only_its_share() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let bound = case.bound()?;
    let surviving = bound.factor()?;
    let survivor = Rc::downgrade(&surviving.resource);
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &surviving)?;
    let completed = bound.publish(candidate)?;
    drop(surviving);
    let bound = case.bound()?;
    let discarded = bound.factor()?;
    let dead = Rc::downgrade(&discarded.resource);
    let mut candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &discarded)?;
    candidate.revision = candidate
        .revision
        .checked_add(1)
        .ok_or("revision exhausted")?;
    drop(discarded);
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("bad revision admitted")?;
    assert!(dead.upgrade().is_none());
    assert!(survivor.upgrade().is_some());
    case.heap.retire_execution(completed.record())?;
    assert!(survivor.upgrade().is_none());
    assert!(matches!(
        failure.cause(),
        crate::OperationError::CandidateContract
    ));
    Ok(())
}

/// Equal operator keys cannot substitute another original factor ticket at publication.
/// ´test:unit:execution-publication-refuses-substituted-factor-ticket´
#[test]
fn execution_publication_refuses_substituted_factor_ticket() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let bound = case.bound()?;
    let first = bound.factor()?;
    let second = bound.factor()?;
    assert_eq!(first.dependency(), second.dependency());
    let mut candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &first)?;
    let Some(crate::RecipeEvidence::Solve(claim)) = candidate.evidence.first_mut() else {
        return Err("missing solve evidence".into());
    };
    claim.factor = Some(second.used_by(bound.execution()));
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("substituted ticket admitted")?;
    assert!(matches!(
        failure.cause(),
        crate::OperationError::PrivateAllocation(crate::PrivateAllocationError::ForeignInventory)
    ));
    Ok(())
}

/// Actual factor and solve vectors use one event counter instead of summed scoped peaks.
/// ´test:unit:execution-common-peak-follows-actual-lifetimes´
#[test]
fn execution_common_peak_follows_actual_lifetimes() -> TestResult {
    common_peak::<f32>()?;
    common_peak::<f64>()
}
/// Factor witness vectors finish before RHS scratch starts; the retained backend stays opaque.
fn common_peak<P: Scalar + 'static>() -> TestResult {
    let mut case = Case::<P>::new()?;
    let bound = case.bound()?;
    let factor = bound.factor()?;
    let factor_vectors: usize = factor
        .allocation_inventory()
        .allocations()
        .iter()
        .filter_map(crate::FactorAllocation::owned_bytes)
        .sum();
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let result = bound.publish(candidate)?;
    let solved = case
        .heap
        .operation_record(result.record())?
        .solve_inventory()
        .ok_or("solve inventory")?;
    let execution = case.heap.execution_inventory(result.record())?;
    let solve_bytes: usize = solved
        .allocations()
        .iter()
        .filter_map(crate::SolveAllocation::owned_bytes)
        .sum();
    assert_eq!(
        execution.cumulative_payload_bytes(),
        factor_vectors + solve_bytes
    );
    assert_eq!(
        execution.peak_payload_bytes(),
        factor_vectors.max(solved.peak_payload_bytes())
    );
    assert!(execution.peak_payload_bytes() < factor_vectors + solved.peak_payload_bytes());
    Ok(())
}

/// Both formats pin aligned resource metadata, observed lifetimes and the original backend bound.
/// ´test:unit:execution-resource-costs-are-pinned´
#[test]
fn execution_resource_costs_are_pinned() -> TestResult {
    assert_eq!(
        [measurement::<f32>()?, measurement::<f64>()?],
        [
            [112, 56, 240, 792, 2352, 11, 112, 32, 6144],
            [112, 56, 240, 792, 2352, 11, 224, 64, 12288],
        ]
    );
    Ok(())
}
/// Keep common observations separate from scoped plans and opaque backend reservations.
fn measurement<P: Scalar + 'static>() -> TestResult<[usize; 9]> {
    let mut case = Case::<P>::new()?;
    let bound = case.bound()?;
    let factor = bound.factor()?;
    let factor_metadata = factor.allocation_inventory().metadata_bytes();
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let outcome = bound.publish(candidate)?;
    let inventory = case.heap.execution_inventory(outcome.record())?;
    let solved = case
        .heap
        .historical_record(outcome.record())?
        .solve_inventory()
        .ok_or("solve inventory")?;
    Ok([
        super::Registry::entry_bytes(),
        super::Trace::metadata_bytes(),
        inventory.metadata_bytes(),
        factor_metadata,
        solved.metadata_bytes(),
        inventory.observed_allocations(),
        inventory.cumulative_payload_bytes(),
        inventory.peak_payload_bytes(),
        inventory.backend_allowance_bytes(),
    ])
}

/// Exact staged and retained metadata ceilings refuse before the factor allocation begins.
/// ´test:unit:execution-resource-metadata-boundaries´
#[test]
fn execution_resource_metadata_boundaries() -> TestResult {
    metadata_boundaries::<f32>()?;
    metadata_boundaries::<f64>()
}
/// Both resource channels are checked at N and one byte below the actual complete owner size.
fn metadata_boundaries<P: Scalar + 'static>() -> TestResult {
    let measured = u64::try_from(measurement::<P>()?[2])?;
    for staged in [true, false] {
        for (limit, success) in [(measured, true), (measured - 1, false)] {
            let mut case = Case::<P>::new()?;
            let mut limits = nrz_bindings::Limits::default();
            if staged {
                limits.staged_payload_bytes = limit;
            } else {
                limits.output_bytes = limit;
            }
            let budget = nrz_bindings::Budget::new(limits)?;
            let bound = Binding::new(
                case.operation.clone(),
                case.inputs.clone(),
                ExecutionLimits::default(),
            )
            .validate_with_resource_budget(&mut case.heap, &budget)?;
            let factor = bound.factor();
            assert_eq!(factor.is_ok(), success);
            if let Ok(factor) = factor {
                let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
                let outcome = bound.publish(candidate)?;
                assert_eq!(
                    u64::try_from(
                        case.heap
                            .execution_inventory(outcome.record())?
                            .metadata_bytes()
                    )?,
                    measured
                );
            }
        }
    }
    Ok(())
}
