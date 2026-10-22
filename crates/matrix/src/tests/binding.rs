// SPDX-License-Identifier: AGPL-3.0-only
//! Erased-boundary refusal and invisible reservation interactions.

use crate::binding_testing::{TestResult, operation};
use crate::{
    Binding, ExecutionLimits, Extent, Handle, Heap, HeapError, HeapLimits, OperationError, Value,
};
use nrz_bindings::Descriptor;

/// Runtime category and precision checks refuse forged locators before lending a view.
/// ´test:crate:binding-rechecks-erased-category-and-precision´
#[test]
fn binding_rechecks_erased_category_and_precision() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let bundle =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64], &[0.5])?;
    let bound = bundle.bound().ok_or("missing bound")?;
    let wrong_category = Handle::<Value, f64>::new(bound.scope, bound.key);
    assert!(matches!(
        Binding::new(
            operation(1, 1)?,
            vec![wrong_category],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Heap(HeapError::CategoryMismatch))
    ));
    let wrong_precision = Handle::<Value, f32>::new(bundle.value().scope, bundle.value().key);
    assert!(matches!(
        Binding::new(
            operation(1, 1)?,
            vec![wrong_precision],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::Heap(HeapError::PrecisionMismatch))
    ));
    let checked = Binding::new(
        operation(1, 1)?,
        vec![bundle.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    assert_eq!(checked.normalized_inputs()?.views().len(), 1);
    Ok(())
}

/// Reservations hold real storage and slots without making them resolve as checked values.
/// ´test:crate:binding-reservations-remain-invisible´
#[test]
fn binding_reservations_remain_invisible() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(4, 4, 2))?;
    let input =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64], &[0.5])?;
    let captured = heap.value(input.value())?.stamp();
    let checked = Binding::new(
        operation(1, 1)?,
        vec![input.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let output = checked
        .reservations
        .bundles
        .first()
        .ok_or("missing reserved output")?;
    assert!(matches!(
        checked.heap.value(output.value()),
        Err(HeapError::StaleHandle)
    ));
    assert_eq!(checked.heap.descriptors.iter().count(), 2);
    assert_ne!(
        checked
            .output_reservations()
            .first()
            .ok_or("missing reservation")?
            .stamp()
            .id(),
        captured.id()
    );
    drop(checked);
    let retry = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    assert_eq!(heap.value(input.value())?.stamp(), captured);
    assert_eq!(heap.value(retry.value())?.stamp().version().number(), 1);
    Ok(())
}

/// Frozen whole-family zeros survive as original contracts in the bound operation.
/// ´test:crate:binding-retains-original-shape-owner´
#[test]
fn binding_retains_original_shape_owner() -> TestResult {
    use crate::{OperationDraft, StructureScope};
    let shape = crate::binding_testing::zero_shape(StructureScope::WholeFamily)?;
    let declarations = std::rc::Rc::clone(&shape.declarations);
    let checked = OperationDraft::copy(vec![shape]).check(ExecutionLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[0.0_f64])?;
    let bound = Binding::new(checked, vec![source.value()], ExecutionLimits::default())
        .validate(&mut heap)?;
    let retained = bound
        .operation()
        .inputs()
        .first()
        .ok_or("missing contract")?
        .shape();
    assert!(std::rc::Rc::ptr_eq(&declarations, &retained.declarations));
    assert!(
        retained
            .entries()
            .first()
            .is_some_and(crate::EntryContract::is_zero)
    );
    Ok(())
}

/// Cloned recipes share immutable checked schemas instead of recursively copying their owners.
/// ´test:crate:checked-operation-clones-share-schema-evidence´
#[test]
fn checked_operation_clones_share_schema_evidence() -> TestResult {
    let original = operation(1, 1)?;
    let copied = original.clone();
    assert!(std::rc::Rc::ptr_eq(&original.inputs, &copied.inputs));
    Ok(())
}
