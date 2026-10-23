// SPDX-License-Identifier: AGPL-3.0-only
//! Native bundle writes check private target edges, entry revisions and all-or-nothing refusal.

use super::*;
use crate::{Binding, Coverage, ExecutionLimits, Handle, Value};

/// Complete numerical fixture and the original checked axes retained for descendant execution.
type NativeFixture<P> = (
    Heap,
    Vec<ReadingBinding<P>>,
    Handle<Value, P>,
    crate::CheckedOperation,
);

/// Publish an actual bounded copy with a native result family at either supported precision.
fn fixture<P: Scalar + 'static>() -> TestResult<NativeFixture<P>> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [2, 1],
        [true, true],
        &mut ledger()?,
    )?;
    let source = heap.import_symbols(
        &symbols,
        &[P::ONE, P::from_u8(2)],
        Some(&[P::ONE; 2]),
        Some(&[P::ZERO; 2]),
        Coverage::DeclaredBounds,
    )?;
    let limits = ExecutionLimits::default();
    let operation = crate::binding_testing::operation(2, 1)?;
    let mut bound =
        Binding::from_symbols(operation.clone(), vec![source], limits).validate(&mut heap)?;
    let (graph, coverage) = result_plan()?;
    let readings = bound.predeclare_result_readings(
        graph,
        coverage,
        DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let handle = outcome.output().first().ok_or("output")?.value();
    Ok((heap, readings, handle, operation))
}

/// Selected replacement preserves untouched channels and commits all target revisions and invalidations.
/// ´test:unit:bounded-reading-write-updates-complete-native-bundle´
#[test]
fn bounded_reading_write_updates_complete_native_bundle() -> TestResult {
    success::<f32>()?;
    success::<f64>()
}

/// Inspect native target edges in addition to the public value and radius observations.
fn success<P: Scalar + 'static>() -> TestResult {
    let (mut heap, readings, handle, _) = fixture::<P>()?;
    let original = heap.symbol_binding(handle)?;
    let before = heap.value(handle)?.stamp();
    let bound = heap.value(handle)?.bound().ok_or("bound")?;
    let error = heap.value(handle)?.evaluation().ok_or("error")?;
    heap.replace_reading_bundle(
        &readings[1],
        &[P::from_u8(3)],
        &[P::from_u8(2)],
        &[P::ONE],
        &mut ledger()?,
    )?;
    let after = heap.value(handle)?.stamp();
    assert_eq!(after.id(), before.id());
    assert_eq!(after.version().number(), before.version().number() + 1);
    assert_eq!(
        heap.value(handle)?.region().export()?,
        vec![P::from_u8(3), P::from_u8(2)]
    );
    assert_eq!(
        heap.companion(bound)?.region().export()?,
        vec![P::from_u8(2), P::ONE]
    );
    assert_eq!(
        heap.companion(error)?.region().export()?,
        vec![P::ONE, P::ZERO]
    );
    assert_eq!(heap.companion(bound)?.target_stamp(), after);
    assert_eq!(heap.companion(error)?.target_stamp(), after);
    for key in [handle.key, bound.key, error.key] {
        let cover = heap.symbols.cover(key)?;
        assert_eq!(cover.stamp, after);
        assert_eq!(
            cover
                .versions
                .iter()
                .map(std::cell::Cell::get)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
    }
    for reading in &readings {
        assert_eq!(reading.state(), SymbolState::Unavailable);
        assert!(heap.reading_entries(reading).is_err());
    }
    assert!(heap.value_by_symbol(original.symbol(), &original).is_err());
    assert!(heap.symbol_binding(handle).is_ok());
    Ok(())
}

/// Invalid final-channel data leaves centers, both companions, versions and all readers unchanged.
/// ´test:unit:bounded-reading-invalid-payload-is-atomic´
#[test]
fn bounded_reading_invalid_payload_is_atomic() -> TestResult {
    for (centers, totals, errors) in [
        (vec![3.0, f64::NAN], vec![2.0; 2], vec![1.0; 2]),
        (vec![3.0; 2], vec![2.0, -1.0], vec![1.0; 2]),
        (vec![3.0; 2], vec![2.0; 2], vec![1.0, f64::INFINITY]),
        (vec![3.0; 2], vec![2.0; 2], vec![1.0, 3.0]),
        (vec![3.0; 2], vec![2.0; 2], vec![1.0]),
    ] {
        let (mut heap, readings, handle, _) = fixture::<f64>()?;
        let before = heap.value(handle)?.stamp();
        assert!(
            heap.replace_reading_bundle(&readings[0], &centers, &totals, &errors, &mut ledger()?)
                .is_err()
        );
        unchanged(&heap, &readings, handle, before)?;
    }
    Ok(())
}

/// Failed preparation has no visible value, coverage, target, version or state transition.
fn unchanged(
    heap: &Heap,
    readings: &[ReadingBinding<f64>],
    handle: Handle<Value, f64>,
    stamp: crate::ValueStamp,
) -> TestResult {
    assert_eq!(heap.value(handle)?.stamp(), stamp);
    assert_eq!(heap.value(handle)?.region().export()?, vec![1.0, 2.0]);
    assert_eq!(
        heap.selected_bound(handle)?
            .ok_or("bound")?
            .region()
            .export()?,
        vec![1.0; 2]
    );
    assert_eq!(
        heap.selected_evaluation(handle)?
            .ok_or("error")?
            .region()
            .export()?,
        vec![0.0; 2]
    );
    for reading in readings {
        assert_eq!(reading.state(), SymbolState::Published);
        assert!(heap.reading_entries(reading).is_ok());
    }
    Ok(())
}

/// Cumulative Work and complete retained payload refuse at one below the measured boundary.
/// ´test:unit:bounded-reading-write-resource-boundaries´
#[test]
fn bounded_reading_write_resource_boundaries() -> TestResult {
    for (work, staged, precharge, succeeds) in [
        (257, 1392, 0, true),
        (256, 1392, 0, false),
        (257, 1391, 0, false),
        (257, 1392, 1, false),
    ] {
        let (mut heap, readings, handle, _) = fixture::<f64>()?;
        let before = heap.value(handle)?.stamp();
        let mut budget = Budget::new(nrz_bindings::Limits {
            work,
            staged_payload_bytes: staged,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(precharge, super::super::admission::AT)?;
        let result = heap.replace_reading_bundle(&readings[1], &[3.0], &[2.0], &[1.0], &mut budget);
        assert_eq!(result.is_ok(), succeeds, "{result:?}");
        if succeeds {
            assert_eq!(budget.usage().work, 257);
        } else {
            unchanged(&heap, &readings, handle, before)?;
        }
    }
    Ok(())
}

/// Even an unchanged center advances versions and stales a previously computed dependent copy.
/// ´test:unit:bounded-reading-companion-write-stales-descendants´
#[test]
fn bounded_reading_companion_write_stales_descendants() -> TestResult {
    let (mut heap, readings, handle, operation) = fixture::<f64>()?;
    let selected = heap.symbol_binding(handle)?;
    let limits = ExecutionLimits::default();
    let bound = Binding::from_symbols(operation, vec![selected], limits).validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let copied = outcome.output().first().ok_or("copy")?.value();
    heap.replace_reading_bundle(
        &readings[0],
        &[1.0, 2.0],
        &[2.0; 2],
        &[1.0; 2],
        &mut ledger()?,
    )?;
    assert!(heap.value(copied).is_err());
    assert!(heap.value(handle).is_ok());
    assert!(
        heap.replace_reading_bundle(
            &readings[0],
            &[1.0, 2.0],
            &[2.0; 2],
            &[1.0; 2],
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Storage refusal after dependency preparation leaves every original native target readable.
/// ´test:unit:bounded-reading-storage-refusal-is-atomic´
#[test]
fn bounded_reading_storage_refusal_is_atomic() -> TestResult {
    let (mut heap, readings, handle, _) = fixture::<f64>()?;
    let before = heap.value(handle)?.stamp();
    // Each two-entry export fits; the complete twelve-entry live heap cannot.
    heap.limits.elements = 2;
    let result = heap.replace_reading_bundle(
        &readings[0],
        &[3.0; 2],
        &[2.0; 2],
        &[1.0; 2],
        &mut ledger()?,
    );
    assert!(
        matches!(result, Err(LiveBindingError::Heap(HeapError::Capacity))),
        "{result:?}"
    );
    unchanged(&heap, &readings, handle, before)
}
