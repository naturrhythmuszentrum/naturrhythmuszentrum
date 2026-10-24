// SPDX-License-Identifier: AGPL-3.0-only
//! Public rounded-copy admission retains the actual selected copy and one exact lineage.

use crate::binding_testing::TestResult;
use crate::copy_testing as fixture;
use crate::reading_testing::ledger;
use nrz_matrix::{DependencyPolicy, Heap, HeapLimits, SymbolState};
use nrz_scalar::Scalar;

/// Both repeated and negative copies admit different centers only with separate selected error coverage.
/// ´test:integration:complex-copies-retain-selected-discrepancies´
#[test]
fn complex_copies_retain_selected_discrepancies() -> TestResult {
    for negative in [false, true] {
        success::<f32>(negative)?;
        success::<f64>(negative)?;
    }
    Ok(())
}

/// Verify the selected copies, outward discrepancy and exact family identity independently.
fn success<P: Scalar + 'static>(negative: bool) -> TestResult {
    let prepared = fixture::plan(negative, true)?;
    // Complex output sort intentionally remains outside the homogeneous Real facade.
    assert!(matches!(
        nrz_bindings::BindingSymbols::from_reading(
            std::rc::Rc::clone(&prepared.graph),
            prepared.coverage.readings()[0].reading(),
            &mut ledger()?,
        ),
        Err(nrz_bindings::AdmissionError::UnsupportedRecipe)
    ));
    let regions = fixture::regions::<P>(negative, true)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = heap.import_readings(
        prepared.graph,
        prepared.coverage,
        prepared.companions,
        &regions,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    )?;
    let [whole, left, right] = imported.bindings() else {
        return Err("readings".into());
    };
    assert_eq!(heap.reading_entries(whole)?, regions[0].values);
    assert_eq!(
        left.reading().family().root(),
        right.reading().family().root()
    );
    let copies = whole.copy_discrepancies().collect::<Vec<_>>();
    let [copy] = copies.as_slice() else {
        return Err("copy evidence".into());
    };
    assert!(copy.upper() >= 0.125 && copy.upper() < 0.126);
    assert_eq!(copy.errors().map(f64::to_bits), [0.25_f64.to_bits(); 2]);
    assert_eq!(copy.negated(), negative);
    assert_eq!(copy.entries()[0], left.adapter().entries()[0]);
    assert_eq!(copy.entries()[1], right.adapter().entries()[0]);
    assert_eq!(
        copy.parameters()
            .iter()
            .map(|entry| entry.index)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_ne!(heap.reading_entries(left)?, heap.reading_entries(right)?);
    heap.replace_reading_entries(
        whole,
        &[P::from_u8(2), P::from_u8(3), -P::from_u8(3), P::from_u8(2)],
        &mut ledger()?,
    )?;
    for reading in imported.bindings() {
        assert_eq!(reading.state(), SymbolState::Unavailable);
    }
    Ok(())
}

/// Executed copies retain each selected radius and both original repeated-entry discrepancies.
/// ´test:integration:complex-copies-select-companions-through-execution´
#[test]
fn complex_copies_select_companions_through_execution() -> TestResult {
    for negative in [false, true] {
        selected_copies::<f32>(negative)?;
        selected_copies::<f64>(negative)?;
    }
    Ok(())
}

/// Independent native copies share exact lineage while keeping their differing centers and own radii.
fn selected_copies<P: Scalar + 'static>(negative: bool) -> TestResult {
    let prepared = fixture::plan(negative, true).map_err(|e| format!("declarations: {e:?}"))?;
    let regions = fixture::regions::<P>(negative, true).map_err(|e| format!("payloads: {e:?}"))?;
    let mut heap = Heap::new(HeapLimits::default()).map_err(|e| format!("heap: {e:?}"))?;
    let imported = heap
        .import_readings(
            prepared.graph,
            prepared.coverage,
            prepared.companions,
            &regions,
            DependencyPolicy::WholeBase,
            &mut ledger()?,
        )
        .map_err(|e| format!("import: {e:?}"))?;
    let operation = copy_operation(&imported.bindings()[1])?;
    let mut selected = Vec::new();
    for reading in &imported.bindings()[1..] {
        selected.push(
            heap.select_reading(reading, &mut ledger()?)
                .map_err(|e| format!("select copy: {e:?}"))?,
        );
    }
    let bound = nrz_matrix::Binding::from_symbols(
        operation,
        selected,
        nrz_matrix::ExecutionLimits::default(),
    )
    .validate(&mut heap)
    .map_err(|e| format!("copy binding: {e:?}"))?;
    for input in bound.dependencies() {
        assert_eq!(input.reading_companions().count(), 2);
        assert_eq!(input.copy_discrepancies().count(), 1);
    }
    let views = bound
        .normalized_inputs()
        .map_err(|e| format!("copy views: {e:?}"))?;
    let candidate = bound
        .evaluate(views)
        .map_err(|e| format!("copy evaluation: {e:?}"))?;
    let outcome = bound
        .publish(candidate)
        .map_err(|e| format!("copy publication: {e:?}"))?;
    check_copies(&heap, &outcome, &imported)?;
    let values = heap
        .reading_companion_entries(&imported.bindings()[1], 1)
        .map_err(|e| format!("error parameters: {e:?}"))?;
    heap.replace_reading_companion_entries(&imported.bindings()[1], 1, &values, &mut ledger()?)
        .map_err(|e| format!("error mutation: {e:?}"))?;
    assert!(heap.operation_record(outcome.record()).is_err());
    for output in outcome.output() {
        assert!(heap.value(output.value()).is_err());
    }
    Ok(())
}

/// Native publication keeps exact copy provenance and the original nominal-error radius.
fn check_copies<P: Scalar + 'static>(
    heap: &Heap,
    outcome: &nrz_matrix::Outcome<Vec<nrz_matrix::PublishedBundle<P>>>,
    imported: &nrz_matrix::ReadingImport<P>,
) -> TestResult {
    let record = heap
        .operation_record(outcome.record())
        .map_err(|e| format!("copy record: {e:?}"))?;
    for ((output, original), dependency) in outcome
        .output()
        .iter()
        .zip(&imported.bindings()[1..])
        .zip(record.inputs())
    {
        let expected = heap
            .reading_entries(original)
            .map_err(|e| format!("original copy: {e:?}"))?[0];
        let (center, total, error) = crate::kernel_testing::entry(heap, *output, 0)
            .map_err(|e| format!("copied values: {e:?}"))?;
        assert_eq!(center.to_bits(), expected.to_f64().to_bits());
        assert_eq!(total.to_bits(), 1.0_f64.to_bits());
        assert_eq!(error.to_bits(), 0.25_f64.to_bits());
        assert_eq!(dependency.reading(), Some(original.reading()));
        assert_eq!(dependency.copy_discrepancies().count(), 1);
        assert_eq!(dependency.reading_companions().count(), 2);
    }
    Ok(())
}

/// Exact output shapes preserve the selected scalar meaning in both independent destinations.
fn copy_operation<P: Scalar>(
    reading: &nrz_matrix::ReadingBinding<P>,
) -> TestResult<nrz_matrix::CheckedOperation> {
    let owner = crate::conversion_testing::owner().map_err(|e| format!("shape owner: {e:?}"))?;
    let meaning = reading.reading().entries()[0].descriptor();
    let shape = crate::conversion_testing::shape(&owner, meaning, 1)
        .map_err(|e| format!("shape: {e:?}"))?;
    let operation = nrz_matrix::OperationDraft::copy(vec![shape; 2])
        .check(nrz_matrix::ExecutionLimits::default())
        .map_err(|e| format!("copy definition: {e:?}"))?;
    Ok(operation)
}
