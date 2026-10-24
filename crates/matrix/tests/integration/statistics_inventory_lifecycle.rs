// SPDX-License-Identifier: AGPL-3.0-only
//! Public inventories preserve exact selections, scalar formats and independent retirement history.

use crate::kernel_testing::{TestResult, axis, execute, output, owner, shape};
use crate::statistics_testing::{check, covariance, vector, worked_covariance};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, JointLimits, Layout, OperationDraft,
    PrecisionContract, StatisticsAllocationRole as Role,
};
use nrz_scalar::{Precision, Scalar};

/// Cross operands retain mixed formats, reordered repeated rows and the original common source model.
/// ´test:integration:statistical-inventory-preserves-mixed-repeated-selections´
#[test]
fn statistical_inventory_preserves_mixed_repeated_selections() -> TestResult {
    let declarations = owner()?;
    let rows = axis(2)?;
    let column = axis(1)?;
    let contract = shape(&declarations, &column, &rows)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<f64>(&mut heap, true)?;
    let conversion = PrecisionContract::new(contract, &nrz_bindings::Descriptor::ratio())?;
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::precision(vec![conversion], Precision::F64, Precision::F32)
        .check(limits)?;
    let bound = Binding::<f64, f32>::convert_precision(operation, vec![source.value()], limits)
        .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let narrow = output(&outcome)?;
    let left = heap.alias(
        narrow.value(),
        &Layout::gather(Extent::new(2, 1)?, 2, &[1, 1], 10)?,
    )?;
    let right = heap.alias(
        source.value(),
        &Layout::gather(Extent::new(3, 1)?, 2, &[1, 0, 1], 10)?,
    )?;
    let cross = heap.materialize_cross_covariance(left, right, JointLimits::default())?;
    let moments = heap.moments(cross)?;
    check(
        moments.covariance(),
        &covariance(
            &[0, 0, 1, 1],
            2,
            &[0, 1, 0, 1, 0, 1],
            3,
            2,
            &worked_covariance()?,
        )?,
    )?;
    let history = moments.allocation_inventory().clone();
    assert_eq!(history.model(), moments.model());
    assert_eq!(history.assurance(), moments.assurance());
    assert_eq!(history.input_rows(0)?, [1, 1]);
    assert_eq!(history.input_rows(1)?, [1, 0, 1]);
    assert_eq!(history.source_columns(), [0, 1]);
    for allocation in history
        .allocations()
        .iter()
        .filter(|item| item.role() == Role::InputSensitivity)
    {
        assert_eq!(
            allocation.precision(),
            if allocation.index() == 0 {
                Precision::F32
            } else {
                Precision::F64
            }
        );
        assert_eq!(
            allocation.extent().rows(),
            if allocation.index() == 0 { 2 } else { 3 }
        );
    }
    heap.retire_execution(outcome.record())?;
    assert!(heap.moments(cross).is_ok());
    let version = heap.value(source.value())?.stamp().version();
    heap.replace_represented(source.value(), version, &[2.0_f64, 3.0])?;
    assert!(heap.moments(cross).is_err());
    assert_eq!(history.input_rows(1)?, [1, 0, 1]);
    Ok(())
}

/// Generic statistics join the execution's common counter without inventing a second numerical owner.
/// ´test:integration:statistical-inventory-shares-execution-lifetimes´
#[test]
fn statistical_inventory_shares_execution_lifetimes() -> TestResult {
    common::<f32>()?;
    common::<f64>()
}
/// Observable statistical capacities are counted once and survive only as immutable history.
fn common<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let rows = axis(2)?;
    let column = axis(1)?;
    let contract = shape(&declarations, &column, &rows)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<P>(&mut heap, true)?;
    let outcome = execute(
        &mut heap,
        OperationDraft::copy(vec![contract]),
        vec![source.value()],
    )?;
    let value = output(&outcome)?;
    let moment = heap.moment_companion(value.value())?;
    let inventory = heap.moments(moment)?.allocation_inventory().clone();
    let execution = heap.execution_inventory(outcome.record())?;
    let owners = inventory
        .allocations()
        .iter()
        .filter_map(nrz_matrix::StatisticsAllocation::owned_bytes)
        .collect::<Vec<_>>();
    assert_eq!(execution.execution(), inventory.execution());
    assert_eq!(execution.observed_allocations(), owners.len());
    assert_eq!(
        execution.cumulative_payload_bytes(),
        owners.iter().sum::<usize>()
    );
    assert_eq!(
        execution.peak_payload_bytes(),
        inventory.peak_payload_bytes()
    );
    assert_eq!(
        execution.backend_allowance_bytes(),
        inventory.backend_allowance_bytes()
    );
    heap.retire_execution(outcome.record())?;
    check(heap.moments(moment)?.covariance(), &worked_covariance()?)?;
    assert_eq!(inventory.allocations().len(), 46);
    Ok(())
}

/// Each public standalone inventory policy admits its exact charge and refuses one byte less.
/// ´test:integration:cross-inventory-metadata-byte-boundaries´
#[test]
fn cross_inventory_metadata_byte_boundaries() -> TestResult {
    for staged in [true, false] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = vector::<f64>(&mut heap, true)?;
        let original = heap.materialize_cross_covariance(
            source.value(),
            source.value(),
            JointLimits::default(),
        )?;
        let required = u64::try_from(
            heap.moments(original)?
                .allocation_inventory()
                .metadata_bytes(),
        )?;
        for allowance in [required - 1, required] {
            let mut limits = nrz_bindings::Limits::default();
            if staged {
                limits.staged_payload_bytes = allowance;
            } else {
                limits.output_bytes = allowance;
            }
            let budget = nrz_bindings::Budget::new(limits)?;
            let result = heap.materialize_cross_covariance_with_inventory_budget(
                source.value(),
                source.value(),
                JointLimits::default(),
                &budget,
            );
            if allowance < required {
                assert!(result.is_err());
            } else {
                check(heap.moments(result?)?.covariance(), &worked_covariance()?)?;
            }
            assert!(heap.moments(original).is_ok());
        }
    }
    Ok(())
}

/// Public binding admission applies both inventory byte ceilings before captured numerical parameters exist.
/// ´test:integration:statistical-binding-inventory-byte-boundaries´
#[test]
fn statistical_binding_inventory_byte_boundaries() -> TestResult {
    binding_boundaries::<f32>()?;
    binding_boundaries::<f64>()
}
/// Original declarations keep the same exact metadata charge across successes and refused preparations.
fn binding_boundaries<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let rows = axis(2)?;
    let column = axis(1)?;
    let contract = shape(&declarations, &column, &rows)?;
    let draft = OperationDraft::copy(vec![contract]);
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<P>(&mut heap, true)?;
    let probe = output(&execute(&mut heap, draft.clone(), vec![source.value()])?)?;
    let required = u64::try_from(
        heap.moments(heap.moment_companion(probe.value())?)?
            .allocation_inventory()
            .metadata_bytes(),
    )?;
    let operation = draft.check(ExecutionLimits::default())?;
    for staged in [true, false] {
        for available in [required - 1, required] {
            let mut limits = nrz_bindings::Limits::default();
            if staged {
                limits.staged_payload_bytes = available;
            } else {
                limits.output_bytes = available;
            }
            let budget = nrz_bindings::Budget::new(limits)?;
            let records = heap.provenance_usage().records();
            let result = Binding::new(
                operation.clone(),
                vec![source.value()],
                ExecutionLimits::default(),
            )
            .validate_with_resource_budget(&mut heap, &budget);
            if available < required {
                assert!(result.is_err());
                assert_eq!(heap.provenance_usage().records(), records);
            } else {
                let bound = result?;
                let candidate = bound.evaluate(bound.normalized_inputs()?)?;
                let outcome = bound.publish(candidate)?;
                let value = output(&outcome)?;
                check(
                    heap.moments(heap.moment_companion(value.value())?)?
                        .covariance(),
                    &worked_covariance()?,
                )?;
            }
        }
    }
    Ok(())
}
