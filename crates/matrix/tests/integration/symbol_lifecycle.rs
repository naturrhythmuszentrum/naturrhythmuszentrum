// SPDX-License-Identifier: AGPL-3.0-only
//! Public symbol-selected import, planned copy and atomic publication lifecycles.

use crate::binding_testing::{TestResult, operation, shape};
use nrz_bindings::{
    AdmissionError, BindingSymbols, Budget, CompanionKind, Descriptor, Limits, SymbolKind,
};
use nrz_matrix::{
    Binding, Coverage, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, Layout,
    OperationDraft, ProvenanceLimits, SymbolState,
};
use nrz_scalar::Scalar;

/// Predeclared imports and both result companions survive exact copy in each precision.
/// ´test:integration:symbol-import-copy-publishes-complete-bundle´
#[test]
fn symbol_import_copy_publishes_complete_bundle() -> TestResult {
    copy_symbols::<f32>()?;
    copy_symbols::<f64>()
}

/// Use only symbol-selected input/output access across the complete numerical operation.
fn copy_symbols<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [2, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    let source = heap.import_symbols(
        &symbols,
        &[P::ONE, P::ZERO],
        Some(&[P::ONE, P::ONE]),
        Some(&[P::ZERO, P::ZERO]),
        Coverage::DeclaredBounds,
    )?;
    assert_eq!(source.symbol(), symbols.value());
    let checked = operation(2, 1)?;
    let bound = Binding::from_symbols(
        checked.clone(),
        vec![source.clone()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let result = bound
        .result_bindings()
        .first()
        .ok_or("planned output")?
        .clone();
    assert_eq!(result.state(), SymbolState::Pending);
    assert!(matches!(
        bound.value_by_symbol(result.symbol(), &result),
        Err(HeapError::UnavailableSymbol)
    ));
    assert_ne!(source.symbol(), result.symbol());
    assert_eq!(
        result.symbols().bound().ok_or("bound")?.kind(),
        SymbolKind::Companion(CompanionKind::DeterministicBound)
    );
    assert_eq!(
        result.symbols().error().ok_or("error")?.kind(),
        SymbolKind::Companion(CompanionKind::NumericalError)
    );
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    assert_eq!(result.state(), SymbolState::Published);
    let view = heap.value_by_symbol(result.symbol(), &result)?;
    assert_eq!(view.region().export()?, [P::ONE, P::ZERO]);
    let bundle = *outcome.output().first().ok_or("output")?;
    for handle in [bundle.bound(), bundle.evaluation()].into_iter().flatten() {
        assert_eq!(heap.companion(handle)?.target_stamp(), view.stamp());
        assert_eq!(heap.companion(handle)?.target(), bundle.value());
    }
    let repeated = Binding::from_symbols(checked, vec![result], ExecutionLimits::default())
        .validate(&mut heap)?;
    let next = repeated.result_bindings().first().ok_or("next")?.clone();
    let candidate = repeated.evaluate(repeated.normalized_inputs()?)?;
    repeated.publish(candidate)?;
    assert_eq!(
        heap.value_by_symbol(next.symbol(), &next)?
            .region()
            .export()?,
        [P::ONE, P::ZERO]
    );
    Ok(())
}

/// Dropped plans stay explicitly unavailable when their private slot positions are reused.
/// ´test:integration:abandoned-symbol-plan-cannot-revive´
#[test]
fn abandoned_symbol_plan_cannot_revive() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f64])?;
    let input = heap.symbol_binding(source.value())?;
    let checked = operation(1, 1)?;
    let bound = Binding::from_symbols(
        checked.clone(),
        vec![input.clone()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let abandoned = bound.result_bindings().first().ok_or("plan")?.clone();
    drop(bound);
    assert_eq!(abandoned.state(), SymbolState::Unavailable);
    let bound = Binding::from_symbols(checked, vec![input], ExecutionLimits::default())
        .validate(&mut heap)?;
    let fresh = bound.result_bindings().first().ok_or("fresh")?.clone();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    bound.publish(candidate)?;
    assert!(matches!(
        heap.value_by_symbol(abandoned.symbol(), &abandoned),
        Err(HeapError::UnavailableSymbol)
    ));
    assert_eq!(
        heap.value_by_symbol(fresh.symbol(), &fresh)?
            .region()
            .get(0, 0)?,
        3.0
    );
    Ok(())
}

/// A late provenance refusal publishes neither symbol result nor either companion bundle.
/// ´test:integration:failed-symbol-publication-has-no-readable-prefix´
#[test]
fn failed_symbol_publication_has_no_readable_prefix() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    heap.set_provenance_limits(ProvenanceLimits::new(0, 0, 0, 0, 0))?;
    let source =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f64], &[0.5])?;
    let input = heap.symbol_binding(source.value())?;
    let checked = OperationDraft::copy(vec![shape(1, 1, &Descriptor::ratio())?; 2])
        .check(ExecutionLimits::default())?;
    let bound = Binding::from_symbols(checked, vec![input.clone(); 2], ExecutionLimits::default())
        .validate(&mut heap)?;
    let planned = bound.result_bindings().to_vec();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    assert!(bound.publish(candidate).is_err());
    for result in planned {
        assert_eq!(result.state(), SymbolState::Unavailable);
        assert!(matches!(
            heap.value_by_symbol(result.symbol(), &result),
            Err(HeapError::UnavailableSymbol)
        ));
    }
    assert_eq!(heap.provenance_usage().records(), 0);
    assert_eq!(
        heap.value_by_symbol(input.symbol(), &input)?
            .region()
            .get(0, 0)?,
        3.0
    );
    Ok(())
}

/// Symbol identity, heap scope and every captured version are required for access.
/// ´test:integration:symbol-lookup-refuses-foreign-wrong-and-stale-bindings´
#[test]
fn symbol_lookup_refuses_foreign_wrong_and_stale_bindings() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let raw = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f64])?;
    let input = heap.symbol_binding(raw.value())?;
    let other = BindingSymbols::declare(
        &Descriptor::ratio(),
        [1, 1],
        [false, false],
        &mut Budget::new(Limits::default())?,
    )?;
    assert!(matches!(
        heap.value_by_symbol(other.value(), &input),
        Err(HeapError::Symbols(AdmissionError::ValueMismatch))
    ));
    let foreign = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        foreign.value_by_symbol(input.symbol(), &input),
        Err(HeapError::ForeignHandle)
    ));
    let version = heap.value(raw.value())?.stamp().version();
    heap.replace_represented(raw.value(), version, &[3.0])?;
    assert!(matches!(
        heap.value_by_symbol(input.symbol(), &input),
        Err(HeapError::StaleEvidence)
    ));
    assert!(
        Binding::from_symbols(operation(1, 1)?, vec![input], ExecutionLimits::default())
            .validate(&mut heap)
            .is_err()
    );
    Ok(())
}

/// Aliases retain their base symbol while copies get a fresh symbol and selected companions.
/// ´test:integration:symbol-alias-copy-retains-owner-and-version-closure´
#[test]
fn symbol_alias_copy_retains_owner_and_version_closure() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let raw = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[3.0_f64, 5.0],
        &[0.25, 0.5],
    )?;
    let base = heap.symbol_binding(raw.value())?;
    let alias = heap.alias(
        raw.value(),
        &Layout::gather(Extent::new(3, 1)?, 2, &[1, 0, 1], 3)?,
    )?;
    let selected = heap.symbol_binding(alias)?;
    assert_eq!(base.symbol(), selected.symbol());
    let bound = Binding::from_symbols(
        operation(3, 1)?,
        vec![selected.clone()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let result = bound.result_bindings().first().ok_or("output")?.clone();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    bound.publish(candidate)?;
    assert_eq!(
        heap.value_by_symbol(result.symbol(), &result)?
            .region()
            .export()?,
        [5.0, 3.0, 5.0]
    );
    let version = heap.value(raw.value())?.stamp().version();
    heap.replace_declared_bounds(raw.value(), version, &[3.0, 5.0], &[0.125, 0.5])?;
    assert!(heap.value_by_symbol(selected.symbol(), &selected).is_err());
    assert!(heap.value_by_symbol(result.symbol(), &result).is_err());
    Ok(())
}

/// Missing numerical companions and uncovered import errors leave the complete capacity reusable.
/// ´test:integration:declared-symbol-import-refusals-are-atomic´
#[test]
fn declared_symbol_import_refusals_are_atomic() -> TestResult {
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    let mut heap = Heap::new(HeapLimits::new(3, 3, 3))?;
    assert!(
        heap.import_symbols(
            &symbols,
            &[1.0_f64],
            Some(&[0.25]),
            None,
            Coverage::DeclaredBounds
        )
        .is_err()
    );
    assert!(
        heap.import_symbols(
            &symbols,
            &[1.0_f64],
            Some(&[0.25]),
            Some(&[0.5]),
            Coverage::DeclaredBounds
        )
        .is_err()
    );
    assert!(
        heap.import_symbols(
            &symbols,
            &[f64::NAN],
            Some(&[0.25]),
            Some(&[0.0]),
            Coverage::DeclaredBounds
        )
        .is_err()
    );
    let result = heap.import_symbols(
        &symbols,
        &[1.0_f64],
        Some(&[0.25]),
        Some(&[0.125]),
        Coverage::DeclaredBounds,
    )?;
    assert_eq!(result.state(), SymbolState::Published);
    let value = heap.value_by_symbol(result.symbol(), &result)?;
    assert_eq!(value.region().get(0, 0)?, 1.0);
    assert_eq!(
        heap.companion(value.evaluation().ok_or("error")?)?
            .region()
            .get(0, 0)?,
        0.125
    );
    Ok(())
}

/// Equal dimensions do not let a different declared kind satisfy a copy input slot.
/// ´test:integration:symbol-copy-refuses-equal-dimension-wrong-kind´
#[test]
fn symbol_copy_refuses_equal_dimension_wrong_kind() -> TestResult {
    let angle = Descriptor::new(
        nrz_bindings::Kind::Angle,
        nrz_bindings::Measure::coherent(),
        nrz_bindings::Role::Difference,
    )?;
    assert_eq!(angle.dimension(), Descriptor::ratio().dimension());
    let symbols = BindingSymbols::declare(
        &angle,
        [1, 1],
        [false, false],
        &mut Budget::new(Limits::default())?,
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_symbols(
        &symbols,
        &[1.0_f64],
        None,
        None,
        Coverage::RepresentedInputs,
    )?;
    assert!(matches!(
        Binding::from_symbols(operation(1, 1)?, vec![input], ExecutionLimits::default())
            .validate(&mut heap),
        Err(nrz_matrix::OperationError::InputMismatch { slot: 0 })
    ));
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}

/// A derived result cannot bypass its checked publication through arbitrary import bytes.
/// ´test:integration:derived-symbol-cannot-bypass-copy-publication´
#[test]
fn derived_symbol_cannot_bypass_copy_publication() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let source =
        BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [false, false], &mut budget)?;
    let result = source.copy_result([1, 1], &mut budget)?;
    let mut heap = Heap::new(HeapLimits::new(1, 1, 1))?;
    assert!(matches!(
        heap.import_symbols(&result, &[9.0_f64], None, None, Coverage::RepresentedInputs),
        Err(HeapError::Symbols(
            nrz_bindings::AdmissionError::ResultMismatch
        ))
    ));
    let imported =
        heap.import_symbols(&source, &[2.0_f64], None, None, Coverage::RepresentedInputs)?;
    assert_eq!(
        heap.value_by_symbol(imported.symbol(), &imported)?
            .region()
            .get(0, 0)?,
        2.0
    );
    Ok(())
}
