// SPDX-License-Identifier: AGPL-3.0-only
//! Binding refusal twins at the public operation boundary.

use crate::binding_testing::{TestResult, operation, shape};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, OperationDraft, OperationError,
};
use nrz_units::Descriptor;

/// Foreign and retired handles refuse while the surviving local occupation binds.
/// ´test:integration:binding-refuses-foreign-and-retired-inputs´
#[test]
fn binding_refuses_foreign_and_retired_inputs() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut foreign = Heap::new(HeapLimits::default())?;
    let extent = Extent::new(1, 1)?;
    let old = heap.import_represented(extent, &Descriptor::ratio(), &[1.0_f64])?;
    let other = foreign.import_represented(extent, &Descriptor::ratio(), &[1.0_f64])?;
    assert!(matches!(
        Binding::new(
            operation(1, 1)?,
            vec![other.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Heap(HeapError::ForeignHandle))
    ));
    heap.retire(old.value())?;
    let fresh = heap.import_represented(extent, &Descriptor::ratio(), &[2.0_f64])?;
    assert!(matches!(
        Binding::new(
            operation(1, 1)?,
            vec![old.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Heap(HeapError::StaleHandle))
    ));
    let bound = Binding::new(
        operation(1, 1)?,
        vec![fresh.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    assert_eq!(
        bound
            .normalized_inputs()?
            .views()
            .first()
            .ok_or("missing view")?
            .get(0, 0)?
            .to_bits(),
        2.0_f64.to_bits()
    );
    Ok(())
}

/// Exact shape matching refuses a different extent before lending any operand.
/// ´test:integration:binding-requires-exact-slot-schemas´
#[test]
fn binding_requires_exact_slot_schemas() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let value =
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0_f32, 2.0])?;
    assert!(matches!(
        Binding::new(
            operation(1, 2)?,
            vec![value.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::InputMismatch { slot: 0 })
    ));
    assert!(matches!(
        Binding::<f32>::new(operation(2, 1)?, Vec::new(), ExecutionLimits::default())
            .validate(&mut heap),
        Err(OperationError::InputCount)
    ));
    let bound = Binding::new(
        operation(2, 1)?,
        vec![value.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    assert_eq!(
        bound
            .normalized_inputs()?
            .views()
            .first()
            .ok_or("missing view")?
            .extent(),
        Extent::new(2, 1)?
    );
    Ok(())
}

/// Work includes carried bounds and is tested immediately below, at and above the total.
/// ´test:integration:binding-work-and-scratch-boundaries´
#[test]
fn binding_work_and_scratch_boundaries() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let value = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0],
        &[0.5, 0.25],
    )?;
    for limit in [3, 4, 5] {
        for policy in [
            ExecutionLimits::new(1, limit, 4),
            ExecutionLimits::new(1, 4, limit),
        ] {
            let result =
                Binding::new(operation(2, 1)?, vec![value.value()], policy).validate(&mut heap);
            if limit < 4 {
                assert!(matches!(result, Err(OperationError::ResourceLimit)));
            } else {
                assert_eq!(result?.output_reservations().len(), 1);
            }
        }
    }
    Ok(())
}

/// Complete output reservation capacity counts both values and companions together.
/// ´test:integration:binding-capacity-boundaries-are-atomic´
#[test]
fn binding_capacity_boundaries_are_atomic() -> TestResult {
    for capacity in [3, 4, 5] {
        let mut heap = Heap::new(HeapLimits::new(capacity, capacity, 2))?;
        let value = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[1.0_f64],
            &[0.5],
        )?;
        let stamp = heap.value(value.value())?.stamp();
        let result = Binding::new(
            operation(1, 1)?,
            vec![value.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap);
        if capacity < 4 {
            assert!(matches!(
                result,
                Err(OperationError::Heap(HeapError::Capacity))
            ));
        } else {
            assert_eq!(result?.output_reservations().len(), 1);
        }
        assert_eq!(heap.value(value.value())?.stamp(), stamp);
    }
    Ok(())
}

/// A later wrong input aborts the connected reservation, leaving earlier input evidence intact.
/// ´test:integration:binding-later-slot-refusal-preserves-earlier-input´
#[test]
fn binding_later_slot_refusal_preserves_earlier_input() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(3, 3, 2))?;
    let first = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64])?;
    let second =
        heap.import_represented(Extent::new(1, 2)?, &Descriptor::ratio(), &[2.0_f64, 3.0])?;
    let recipe = OperationDraft::copy(vec![shape(1, 1, &Descriptor::ratio())?; 2])
        .check(ExecutionLimits::default())?;
    assert!(matches!(
        Binding::new(
            recipe,
            vec![first.value(), second.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::InputMismatch { slot: 1 })
    ));
    assert_eq!(
        heap.value(first.value())?.region().get(0, 0)?.to_bits(),
        1.0_f64.to_bits()
    );
    heap.retire(second.value())?;
    assert_eq!(
        Binding::new(
            operation(1, 1)?,
            vec![first.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap)?
        .output_reservations()
        .len(),
        1
    );
    Ok(())
}

/// Center structure and whole-family structure impose different bound obligations.
/// ´test:integration:binding-checks-structural-zero-scope´
#[test]
fn binding_checks_structural_zero_scope() -> TestResult {
    use nrz_matrix::StructureScope;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[0.0_f64], &[0.5])?;
    for scope in [StructureScope::CenterOnly, StructureScope::WholeFamily] {
        let recipe = OperationDraft::copy(vec![crate::binding_testing::zero_shape(scope)?])
            .check(ExecutionLimits::default())?;
        let result = Binding::new(recipe, vec![source.value()], ExecutionLimits::default())
            .validate(&mut heap);
        if scope == StructureScope::WholeFamily {
            assert!(matches!(
                result,
                Err(OperationError::StructureMismatch { slot: 0, entry: 0 })
            ));
        } else {
            assert_eq!(result?.output_reservations().len(), 1);
        }
    }
    let version = heap.value(source.value())?.stamp().version();
    heap.replace_declared_bounds(source.value(), version, &[0.0], &[0.0])?;
    let recipe = OperationDraft::copy(vec![crate::binding_testing::zero_shape(
        StructureScope::WholeFamily,
    )?])
    .check(ExecutionLimits::default())?;
    assert_eq!(
        Binding::new(recipe, vec![source.value()], ExecutionLimits::default())
            .validate(&mut heap)?
            .output_reservations()
            .len(),
        1
    );
    Ok(())
}
