// SPDX-License-Identifier: AGPL-3.0-only
//! Physical native output replacement preserves complete companions and invalidates old computations.

mod fixture;

use crate::binding_testing::{TestResult, shape};
use nrz_bindings::{BindingSymbols, Budget, Limits};
use nrz_matrix::{
    Binding, Coverage, DependencyPolicy, ExecutionLimits, Heap, HeapLimits, OperationDraft,
    SymbolState,
};
use nrz_scalar::Scalar;

/// A declared voltage result and its complete radius bundle advance together in both formats.
/// ´test:integration:bounded-reading-replacement-retains-physical-targets´
#[test]
fn bounded_reading_replacement_retains_physical_targets() -> TestResult {
    lifecycle::<f32>()?;
    lifecycle::<f64>()
}

/// Only public APIs connect exact result declarations to numerical publication and replacement.
fn lifecycle<P: Scalar + 'static>() -> TestResult {
    let quantity = fixture::voltage()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(&quantity, [2, 1], [true, true], &mut budget)?;
    let source = heap.import_symbols(
        &symbols,
        &[P::ONE, P::from_u8(2)],
        Some(&[P::ONE; 2]),
        Some(&[P::ZERO; 2]),
        Coverage::DeclaredBounds,
    )?;
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::copy(vec![shape(2, 1, &quantity)?]).check(limits)?;
    let mut bound =
        Binding::from_symbols(operation.clone(), vec![source], limits).validate(&mut heap)?;
    let (graph, coverage) = fixture::plan(&quantity)?;
    let readings = bound.predeclare_result_readings(
        graph,
        coverage,
        DependencyPolicy::Entries,
        &mut budget,
    )?;
    let original = bound
        .result_bindings()
        .first()
        .ok_or("result binding")?
        .clone();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let bundle = *outcome.output().first().ok_or("result")?;
    let before = heap.value(bundle.value())?.stamp();
    let dependent =
        Binding::from_symbols(operation, vec![original.clone()], limits).validate(&mut heap)?;
    let candidate = dependent.evaluate(dependent.normalized_inputs()?)?;
    let copied = dependent.publish(candidate)?;
    heap.replace_reading_bundle(
        &readings[0],
        &[P::ONE, P::from_u8(2)],
        &[P::from_u8(2); 2],
        &[P::ONE; 2],
        &mut budget,
    )?;
    assert_eq!(readings[0].state(), SymbolState::Unavailable);
    assert!(heap.value_by_symbol(original.symbol(), &original).is_err());
    assert!(
        heap.value(copied.output().first().ok_or("copy")?.value())
            .is_err()
    );
    changed(&heap, bundle, before, &quantity)
}

/// Current value and companion target versions share one physical declaration.
fn changed<P: Scalar + 'static>(
    heap: &Heap,
    bundle: nrz_matrix::PublishedBundle<P>,
    before: nrz_matrix::ValueStamp,
    quantity: &nrz_bindings::Descriptor,
) -> TestResult {
    let current = heap.symbol_binding(bundle.value())?;
    assert_eq!(current.symbol().descriptor(), quantity);
    let after = heap.value_by_symbol(current.symbol(), &current)?.stamp();
    assert_eq!(after.id(), before.id());
    assert_eq!(after.version().number(), before.version().number() + 1);
    for (handle, expected) in [
        (bundle.bound(), P::from_u8(2)),
        (bundle.evaluation(), P::ONE),
    ] {
        let view = heap.companion(handle.ok_or("companion")?)?;
        assert_eq!(view.target(), bundle.value());
        assert_eq!(view.target_stamp(), after);
        assert_eq!(view.region().export()?, vec![expected; 2]);
    }
    Ok(())
}
