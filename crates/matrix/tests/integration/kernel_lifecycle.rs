// SPDX-License-Identifier: AGPL-3.0-only
//! Multi-input deterministic lineage, selected regions and atomic stale-evidence refusal.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use nrz_matrix::{
    Axis, EntryMap, Extent, Heap, HeapError, HeapLimits, OperationDraft, Partition, ShapeLimits,
};
use nrz_units::Descriptor;

/// A selected repeated output retains both sum parents, including the nonprimary dependency.
/// ´test:integration:sum-selection-invalidates-through-either-operand´
#[test]
fn sum_selection_invalidates_through_either_operand() -> TestResult {
    let declarations = owner()?;
    let column = axis(1)?;
    let coordinates = axis(2)?;
    let contract = shape(&declarations, &column, &coordinates)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let first = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64, 3.0],
        &[0.125, 0.25],
    )?;
    let second = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64, 4.0],
        &[0.125, 0.25],
    )?;
    let original_version = heap.value(second.value())?.stamp().version();
    let copied = execute(
        &mut heap,
        OperationDraft::copy(vec![contract.clone()]),
        vec![second.value()],
    )?;
    let summed = execute(
        &mut heap,
        OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?,
        vec![first.value(), output(&copied)?.value()],
    )?;
    let selected = select_sum(&mut heap, &summed, column, &coordinates)?;
    check_selection(&heap, selected)?;
    heap.compact()?;
    assert_eq!(heap.operation_record(summed.record())?.inputs().len(), 2);
    heap.replace_declared_bounds(second.value(), original_version, &[1.0, 4.0], &[0.25, 0.25])?;
    assert!(matches!(
        heap.value(output(&summed)?.value()),
        Err(HeapError::StaleEvidence)
    ));
    assert!(matches!(
        heap.value(selected.value()),
        Err(HeapError::StaleEvidence)
    ));
    assert!(heap.value(first.value()).is_ok());
    Ok(())
}

/// Algebraic transpose preserves physical entry units and maps each companion exactly.
/// ´test:integration:dual-transpose-and-reshape-enclose-permuted-values´
#[test]
fn dual_transpose_and_reshape_enclose_permuted_values() -> TestResult {
    let declarations = owner()?;
    let domain = axis(2)?;
    let codomain = axis(1)?;
    let contract = shape(&declarations, &domain, &codomain)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 2)?,
        &Descriptor::ratio(),
        &[2.0_f64, 3.0],
        &[0.125, 0.25],
    )?;
    let limits = ShapeLimits::default();
    let reshaped = OperationDraft::transform_entries(
        contract.clone(),
        codomain,
        domain,
        EntryMap::reshape(Extent::new(1, 2)?, Extent::new(2, 1)?, limits)?,
        limits,
    )?;
    for draft in [OperationDraft::dual_transpose(contract, limits)?, reshaped] {
        let result = execute(&mut heap, draft, vec![source.value()])?;
        for (index, nominal) in [2, 3].into_iter().enumerate() {
            let (center, radius, error) = entry(&heap, output(&result)?, index)?;
            assert!(Rational::new(nominal, 1)?.covered(center, error)?);
            assert!(
                Rational::new(nominal, 1)?
                    .add(Rational::new(i128::try_from(index + 1)?, 8)?)?
                    .covered(center, radius)?
            );
        }
        assert_eq!(
            heap.value(output(&result)?.value())?.region().extent(),
            Extent::new(2, 1)?
        );
    }
    Ok(())
}

/// Derive a repeated result selection without losing either binary input dependency.
fn select_sum(
    heap: &mut Heap,
    summed: &nrz_matrix::Outcome<Vec<nrz_matrix::PublishedBundle<f64>>>,
    column: Axis,
    coordinates: &Axis,
) -> TestResult<nrz_matrix::PublishedBundle<f64>> {
    let sum_shape = heap
        .operation_record(summed.record())?
        .operation()
        .outputs()
        .first()
        .ok_or("output shape")?
        .shape()
        .clone();
    let limits = ShapeLimits::default();
    let selected_axis = Axis::new(
        vec![
            coordinates.entries()[1].clone(),
            coordinates.entries()[1].clone(),
            coordinates.entries()[0].clone(),
        ],
        Partition::whole(3, limits)?,
        limits,
    )?;
    let map = EntryMap::new(
        Extent::new(2, 1)?,
        Extent::new(3, 1)?,
        vec![1, 1, 0],
        limits,
    )?;
    let selected = execute(
        heap,
        OperationDraft::transform_entries(sum_shape, column, selected_axis, map, limits)?,
        vec![output(summed)?.value()],
    )?;
    output(&selected)
}

/// Every repeated result retains the exact center and its complete selected total.
fn check_selection(heap: &Heap, selected: nrz_matrix::PublishedBundle<f64>) -> TestResult {
    for (index, target) in [7, 7, 3].into_iter().enumerate() {
        let (center, radius, error) = entry(heap, selected, index)?;
        assert!(Rational::new(target, 1)?.covered(center, error)?);
        assert!(
            Rational::new(target, 1)?
                .add(Rational::new(if index == 2 { 1 } else { 2 }, 4)?)?
                .covered(center, radius)?
        );
    }
    Ok(())
}
