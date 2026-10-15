// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen binding lifecycles over repeated maps, mutation and relocation.

use crate::binding_testing::{TestResult, operation, shape};
use nrz_matrix::{
    Binding, Coverage, ExecutionLimits, Extent, Heap, HeapLimits, Layout, OperationDraft,
};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// Repeated read selections preserve the base target and companion order in both precisions.
fn repeated_binding<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[P::ONE, P::ZERO],
        &[P::ZERO, P::ONE],
    )?;
    let selected = heap.alias(
        source.value(),
        &Layout::gather(Extent::new(3, 1)?, 2, &[1, 0, 1], 3)?,
    )?;
    let captured = heap.value(source.value())?.stamp();
    let checked = Binding::new(operation(3, 1)?, vec![selected], ExecutionLimits::default())
        .validate(&mut heap)?;
    let dependency = checked.dependencies().first().ok_or("missing dependency")?;
    assert_eq!(dependency.stamp(), captured);
    assert_eq!(
        dependency.companion().ok_or("missing companion")?.target(),
        source.value()
    );
    let inputs = checked.normalized_inputs()?;
    assert_eq!(inputs.execution(), checked.execution());
    let view = inputs.views().first().ok_or("missing view")?;
    assert_eq!(view.get(0, 0)?, P::ZERO);
    assert_eq!(view.get(1, 0)?, P::ONE);
    assert_eq!(view.bound(2, 0)?, Some(P::ONE));
    assert_eq!(
        checked
            .output_reservations()
            .first()
            .ok_or("missing output")?
            .coverage(),
        Coverage::DeclaredBounds
    );
    Ok(())
}

/// Exact normalized copies and companion target maps work for both sealed formats.
/// ´test:integration:binding-preserves-repeated-input-dependence´
#[test]
fn binding_preserves_repeated_input_dependence() -> TestResult {
    repeated_binding::<f32>()?;
    repeated_binding::<f64>()
}

/// A new execution captures companion-only changes and later alias writes, after relocation.
/// ´test:integration:binding-recaptures-mutation-and-compaction´
#[test]
fn binding_recaptures_mutation_and_compaction() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64], &[0.5])?;
    let first = Binding::new(
        operation(1, 1)?,
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let identity = first.execution();
    let captured = first
        .dependencies()
        .first()
        .ok_or("missing dependency")?
        .stamp();
    drop(first);
    heap.replace_declared_bounds(source.value(), captured.version(), &[2.0], &[0.25])?;
    heap.compact()?;
    let second = Binding::new(
        operation(1, 1)?,
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    assert_ne!(second.execution(), identity);
    assert_eq!(
        second
            .dependencies()
            .first()
            .ok_or("missing dependency")?
            .stamp()
            .version()
            .number(),
        captured.version().number() + 1
    );
    assert_eq!(
        second
            .normalized_inputs()?
            .views()
            .first()
            .ok_or("missing view")?
            .bound(0, 0)?
            .map(f64::to_bits),
        Some(0.25_f64.to_bits())
    );
    Ok(())
}

/// Physical units retain typed reference maps while identity normalization preserves bits.
/// ´test:integration:binding-retains-physical-unit-references´
#[test]
fn binding_retains_physical_unit_references() -> TestResult {
    let quantity = crate::testing::voltage()?;
    let recipe =
        OperationDraft::copy(vec![shape(2, 1, &quantity)?]).check(ExecutionLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(2, 1)?, &quantity, &[2.0_f64, 3.0])?;
    let checked = Binding::new(recipe, vec![source.value()], ExecutionLimits::default())
        .validate(&mut heap)?;
    let physical = checked.physical_input(0)?;
    assert_eq!(physical.contract().descriptor(), &quantity);
    assert_eq!(
        physical
            .contract()
            .codomain_coordinates()
            .references()
            .len(),
        2
    );
    assert_eq!(physical.get(1, 0)?.to_bits(), 3.0_f64.to_bits());
    assert_eq!(
        checked
            .normalized_inputs()?
            .views()
            .first()
            .ok_or("missing view")?
            .get(1, 0)?
            .to_bits(),
        3.0_f64.to_bits()
    );
    Ok(())
}
