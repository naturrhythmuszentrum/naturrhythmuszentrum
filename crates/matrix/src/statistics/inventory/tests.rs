// SPDX-License-Identifier: AGPL-3.0-only
//! Private substitutions cannot replace original statistical allocation authority.

use super::{Channel, Role, Run, plan::State};
use crate::kernel_testing::{TestResult, axis, owner, shape};
use crate::{
    Binding, Bound, CheckedOperation, ExecutionLimits, Handle, Heap, HeapLimits, OperationDraft,
    OperationError, PrivateAllocationError as Error, Value,
};
use nrz_scalar::Scalar;

/// Original declared two-row source operand and one immutable copy recipe.
struct Case<P: Scalar> {
    /// Independent owner scope.
    heap: Heap,
    /// Exact selected value.
    value: Handle<Value, P>,
    /// Closed original law.
    operation: CheckedOperation,
}
impl<P: Scalar + 'static> Case<P> {
    /// Rank-one model retains both selected coordinates and their common source correlation.
    fn new() -> TestResult<Self> {
        let declarations = owner()?;
        let rows = axis(1)?;
        let columns = axis(2)?;
        let contract = shape(&declarations, &columns, &rows)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let inputs = crate::solve_statistics_testing::common(&mut heap, &[P::ONE; 3], true)?;
        Ok(Self {
            heap,
            value: inputs[1].value(),
            operation: OperationDraft::copy(vec![contract]).check(ExecutionLimits::default())?,
        })
    }
    /// Use the production capture, planning and transaction entry point.
    fn bind(&mut self) -> TestResult<Bound<'_, P>> {
        Ok(Binding::new(
            self.operation.clone(),
            vec![self.value],
            ExecutionLimits::default(),
        )
        .validate(&mut self.heap)?)
    }
}

/// Concrete representation, finite role and one-use authority are checked before allocation.
/// ´test:unit:statistical-permits-refuse-unplanned-types-and-reissuance´
#[test]
fn statistical_permits_refuse_unplanned_types_and_reissuance() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let bound = case.bind()?;
    let run = bound
        .statistics
        .inventory
        .as_ref()
        .ok_or("statistical plan")?;
    assert!(matches!(
        run.buffer::<f64>(Role::Aligned, 0, Channel::Centers),
        Err(OperationError::PrivateAllocation(Error::UnplannedMeaning))
    ));
    assert!(matches!(
        run.buffer::<f32>(Role::Jacobian, 0, Channel::Centers),
        Err(OperationError::PrivateAllocation(Error::Representation))
    ));
    assert!(matches!(
        run.buffer::<f64>(Role::Jacobian, 0, Channel::Truth),
        Err(OperationError::PrivateAllocation(Error::Representation))
    ));
    assert!(matches!(
        run.buffer::<f64>(Role::InputSensitivity, 0, Channel::Centers),
        Err(OperationError::PrivateAllocation(Error::DuplicateTicket))
    ));
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    bound.publish(candidate)?;
    Ok(())
}

/// Dropping the covered final owner cannot turn an untracked replacement into a completed intermediate.
/// ´test:unit:statistical-publication-refuses-unticketed-final-owner´
#[test]
fn statistical_publication_refuses_unticketed_final_owner() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let count = case.heap.descriptors.iter().count();
    let bound = case.bind()?;
    let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let payload = candidate.statistics.first_mut().ok_or("moments")?;
    payload.covariance.centers = payload.covariance.centers.to_vec().into();
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("unticketed owner published")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(Error::UnticketedBuffer)
    ));
    assert_eq!(case.heap.descriptors.iter().count(), count);
    assert_eq!(case.heap.statistics.records.len(), 0);
    Ok(())
}

/// Equal shape and equal numbers from a different model execution cannot replace original authority.
/// ´test:unit:statistical-publication-refuses-foreign-final-owner´
#[test]
fn statistical_publication_refuses_foreign_final_owner() -> TestResult {
    foreign::<f32>()?;
    foreign::<f64>()
}
/// Both formats authenticate original plans independently of numerical equality.
fn foreign<P: Scalar + 'static>() -> TestResult {
    let mut first = Case::<P>::new()?;
    let mut second = Case::<P>::new()?;
    let first_bound = first.bind()?;
    let second_bound = second.bind()?;
    let mut first_candidate = first_bound.evaluate(first_bound.normalized_inputs()?)?;
    let mut second_candidate = second_bound.evaluate(second_bound.normalized_inputs()?)?;
    std::mem::swap(
        &mut first_candidate
            .statistics
            .first_mut()
            .ok_or("first moments")?
            .covariance,
        &mut second_candidate
            .statistics
            .first_mut()
            .ok_or("second moments")?
            .covariance,
    );
    let failure = first_bound
        .publish(first_candidate)
        .err()
        .ok_or("foreign owner published")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(Error::ForeignInventory)
    ));
    assert_eq!(first.heap.statistics.records.len(), 0);
    Ok(())
}

/// Same-shaped covariance and Jacobian owners still carry different original parameter roles.
/// ´test:unit:statistical-publication-refuses-parameter-role-substitution´
#[test]
fn statistical_publication_refuses_parameter_role_substitution() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let bound = case.bind()?;
    let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let payload = candidate.statistics.first_mut().ok_or("moments")?;
    assert_eq!(payload.covariance.extent, payload.jacobian.extent);
    std::mem::swap(&mut payload.covariance, &mut payload.jacobian);
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("wrong parameter role published")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(Error::ForeignInventory)
    ));
    Ok(())
}

/// A complete numerical payload cannot excuse a missing required intermediate in the original plan.
/// ´test:unit:statistical-publication-refuses-incomplete-plan´
#[test]
fn statistical_publication_refuses_incomplete_plan() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let bound = case.bind()?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let run = bound.statistics.inventory.as_ref().ok_or("plan")?;
    let slot = run.slot(Role::Inputs, 0, Channel::Centers)?;
    run.0.specifications[slot].state.set(State::Planned);
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("incomplete plan published")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(Error::IncompleteCoverage)
    ));
    assert_eq!(case.heap.statistics.records.len(), 0);
    Ok(())
}

/// Original source order and statistical law remain authoritative even with complete allocation permits.
/// ´test:unit:statistical-publication-refuses-source-and-law-substitution´
#[test]
fn statistical_publication_refuses_source_and_law_substitution() -> TestResult {
    for change_law in [false, true] {
        let mut case = Case::<f64>::new()?;
        let bound = case.bind()?;
        let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let payload = candidate.statistics.first_mut().ok_or("moments")?;
        if change_law {
            payload.lineage.assurance = crate::MomentAssurance::LinearizedMoments;
        } else {
            payload.lineage.columns.reverse();
        }
        let failure = bound
            .publish(candidate)
            .err()
            .ok_or("substituted source relation published")?;
        assert!(matches!(
            failure.cause(),
            OperationError::Statistics(crate::StatisticsError::Contract)
        ));
    }
    Ok(())
}

/// Both inventory ceilings refuse one byte below the actual aligned charge before any float allocation.
/// ´test:unit:statistical-metadata-boundaries-precede-numerical-capture´
#[test]
fn statistical_metadata_boundaries_precede_numerical_capture() -> TestResult {
    for staged in [true, false] {
        for admitted in [true, false] {
            let case = Case::<f64>::new()?;
            let trace = crate::execution::Trace::new(crate::ExecutionId(
                crate::heap::identity::identity()?,
            ));
            let binding = crate::statistics::capture::inspect(
                &case.heap,
                case.value,
                0,
                crate::JointLimits::default(),
            )?;
            let budget = nrz_bindings::Budget::new(nrz_bindings::Limits::default())?;
            let probe = Run::new::<f64>(vec![binding], Some(&case.operation), &trace, &budget)?;
            let required = u64::try_from(probe.0.metadata)?;
            let mut limits = nrz_bindings::Limits::default();
            let available = required - u64::from(!admitted);
            if staged {
                limits.staged_payload_bytes = available;
            } else {
                limits.output_bytes = available;
            }
            let binding = crate::statistics::capture::inspect(
                &case.heap,
                case.value,
                0,
                crate::JointLimits::default(),
            )?;
            let budget = nrz_bindings::Budget::new(limits)?;
            assert_eq!(
                Run::new::<f64>(vec![binding], Some(&case.operation), &trace, &budget).is_ok(),
                admitted
            );
            assert_eq!(trace.observations(), [0, 0, 0]);
        }
    }
    Ok(())
}

/// Aligned owner widths and charge components retain their independently measured target layout.
/// ´test:unit:statistical-metadata-layout-is-exact´
#[test]
fn statistical_metadata_layout_is_exact() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let bound = case.bind()?;
    let run = bound.statistics.inventory.as_ref().ok_or("plan")?;
    let parts = super::metadata::parts(&run.0)?;
    let sizes = [
        size_of::<super::plan::Specification>(),
        size_of::<super::StatisticsAllocation>(),
        size_of::<crate::statistics::capture::InputBinding>(),
        size_of::<super::plan::RcLayout<crate::statistics::store::Lineage>>(),
        size_of::<crate::SourceId>(),
        size_of::<nrz_bindings::Descriptor>(),
        size_of::<nrz_bindings::QuantitySymbol>(),
        size_of::<crate::Dependency>(),
    ];
    assert_eq!(parts, [224, 64, 4_048, 576, 32, 0, 176, 16]);
    assert_eq!(sizes, [72, 16, 576, 128, 8, 736, 8, 488]);
    Ok(())
}

/// An equal-shaped replacement recipe is refused before any general contraction permit is consumed.
/// ´test:unit:statistical-inventory-refuses-replaced-original-recipe´
#[test]
fn statistical_inventory_refuses_replaced_original_recipe() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let count = case.heap.descriptors.iter().count();
    let mut bound = case.bind()?;
    let input = bound.operation.inputs()[0].shape().clone();
    let limits = crate::ShapeLimits::default();
    let selection = crate::EntryMap::new(input.extent(), input.extent(), vec![0, 1], limits)?;
    bound.operation = OperationDraft::transform_entries(
        input.clone(),
        input.domain().clone(),
        input.codomain().clone(),
        selection,
        limits,
    )?
    .check(ExecutionLimits::default())?;
    let run = bound.statistics.inventory.as_ref().ok_or("plan")?;
    let observed = run.0.trace.observations();
    let failure = bound
        .evaluate(bound.normalized_inputs()?)
        .err()
        .ok_or("replaced original recipe evaluated")?;
    assert!(
        matches!(
            failure.cause(),
            OperationError::PrivateAllocation(Error::ForeignInventory)
        ),
        "unexpected refusal: {:?}",
        failure.cause()
    );
    assert_eq!(run.0.trace.observations(), observed);
    drop(bound);
    assert_eq!(case.heap.descriptors.iter().count(), count);
    assert_eq!(case.heap.statistics.records.len(), 0);
    Ok(())
}

/// Altering the bound and candidate together cannot replace the original inventory's source order.
/// ´test:unit:statistical-inventory-refuses-replaced-bound-lineage´
#[test]
fn statistical_inventory_refuses_replaced_bound_lineage() -> TestResult {
    let mut case = Case::<f64>::new()?;
    let mut bound = case.bind()?;
    let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
    bound
        .statistics
        .lineage
        .as_mut()
        .ok_or("bound lineage")?
        .columns
        .reverse();
    candidate
        .statistics
        .first_mut()
        .ok_or("candidate lineage")?
        .lineage
        .columns
        .reverse();
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("replaced original lineage published")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(Error::ForeignInventory)
    ));
    Ok(())
}
