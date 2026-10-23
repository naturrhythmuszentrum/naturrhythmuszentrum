// SPDX-License-Identifier: AGPL-3.0-only
//! Distinct rounded Complex copies retain one exact lineage and both numerical discrepancies.

use crate::copy_testing as fixture;

use super::*;

/// Total bounds alone cannot establish numerical agreement of two copies of the nominal target.
/// ´test:unit:complex-copies-refuse-missing-numerical-coverage´
#[test]
fn complex_copies_refuse_missing_numerical_coverage() -> TestResult {
    let prepared = fixture::plan(false, false)?;
    let regions = fixture::regions::<f64>(false, false)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        heap.import_readings(
            prepared.graph,
            prepared.coverage,
            prepared.companions,
            &regions,
            DependencyPolicy::WholeBase,
            &mut ledger()?
        ),
        Err(LiveBindingError::Heap(HeapError::MissingCoverage))
    ));
    assert_eq!(heap.descriptors.iter().count(), 0);
    Ok(())
}

/// An uncovered discrepancy refuses the complete import instead of projecting centers onto the family.
/// ´test:unit:complex-copies-refuse-inconsistent-errors-atomically´
#[test]
fn complex_copies_refuse_inconsistent_errors_atomically() -> TestResult {
    let prepared = fixture::plan(false, true)?;
    let mut regions = fixture::regions::<f64>(false, true)?;
    regions[1].values[1] = 0.0;
    regions[1].values[3] = 0.0;
    let mut heap = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        heap.import_readings(
            prepared.graph,
            prepared.coverage,
            prepared.companions,
            &regions,
            DependencyPolicy::WholeBase,
            &mut ledger()?
        ),
        Err(LiveBindingError::UncoveredCopy)
    ));
    assert_eq!(heap.descriptors.iter().count(), 0);
    Ok(())
}

/// A copy-error parameter revision invalidates the whole reconstruction even through another scalar view.
/// ´test:unit:complex-copies-recheck-error-parameter-versions´
#[test]
fn complex_copies_recheck_error_parameter_versions() -> TestResult {
    let prepared = fixture::plan(false, true)?;
    let regions = fixture::regions::<f64>(false, true)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = heap.import_readings(
        prepared.graph,
        prepared.coverage,
        prepared.companions,
        &regions,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    )?;
    let whole = &imported.bindings()[0];
    let parameter = whole
        .copy_discrepancies()
        .next()
        .ok_or("copy")?
        .parameters()[0]
        .clone();
    let key = whole.group.tickets[parameter.region].key;
    heap.symbols.cover(key)?.versions[usize::try_from(parameter.index)?].set(2);
    for reading in imported.bindings() {
        assert!(matches!(
            heap.reading_entries(reading),
            Err(LiveBindingError::Heap(HeapError::StaleEvidence))
        ));
    }
    Ok(())
}

/// The complete discrepant-copy import shares one Work and retained-payload ledger with exact dependencies.
/// ´test:unit:complex-copies-import-resource-boundaries´
#[test]
fn complex_copies_import_resource_boundaries() -> TestResult {
    let prepared = fixture::plan(false, true)?;
    let regions = fixture::regions::<f64>(false, true)?;
    boundaries(|limits, precharge| {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut budget = Budget::new(limits)?;
        budget.work(precharge, super::super::admission::AT)?;
        let result = heap.import_readings(
            Rc::clone(&prepared.graph),
            prepared.coverage.clone(),
            prepared.companions.clone(),
            &regions,
            DependencyPolicy::WholeBase,
            &mut budget,
        );
        if result.is_err() {
            assert_eq!(heap.descriptors.iter().count(), 0);
        }
        result?;
        Ok(budget.usage().work)
    })
}

/// Repeated storage remains one dependency even when a caller borrows only the other scalar copy.
/// ´test:unit:complex-copies-recheck-both-center-versions´
#[test]
fn complex_copies_recheck_both_center_versions() -> TestResult {
    for index in [0, 3] {
        let prepared = fixture::plan(false, true)?;
        let regions = fixture::regions::<f64>(false, true)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let imported = heap.import_readings(
            prepared.graph,
            prepared.coverage,
            prepared.companions,
            &regions,
            DependencyPolicy::WholeBase,
            &mut ledger()?,
        )?;
        let whole = &imported.bindings()[0];
        let key = whole.group.tickets[0].key;
        heap.symbols.cover(key)?.versions[index].set(2);
        for reading in imported.bindings() {
            assert!(matches!(
                heap.reading_entries(reading),
                Err(LiveBindingError::Heap(HeapError::StaleEvidence))
            ));
        }
    }
    Ok(())
}

/// Finite input centers cannot turn an overflowing comparison into an infinite admitted discrepancy.
/// ´test:unit:complex-copies-refuse-comparison-overflow-atomically´
#[test]
fn complex_copies_refuse_comparison_overflow_atomically() -> TestResult {
    let prepared = fixture::plan(false, true)?;
    let mut regions = fixture::regions::<f64>(false, true)?;
    regions[0].values[0] = f64::MAX;
    regions[0].values[3] = -f64::MAX;
    let mut heap = Heap::new(HeapLimits::default())?;
    let result = heap.import_readings(
        prepared.graph,
        prepared.coverage,
        prepared.companions,
        &regions,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    );
    assert!(matches!(result, Err(LiveBindingError::Arithmetic(_))));
    assert_eq!(heap.descriptors.iter().count(), 0);
    Ok(())
}

/// Distinct exact judgments retain the caller's fine-inventory budget refusal without any partial import.
/// ´test:unit:complex-copies-fine-import-budget-refusal-is-atomic´
#[test]
fn complex_copies_fine_import_budget_refusal_is_atomic() -> TestResult {
    let prepared = fixture::plan(false, true)?;
    let regions = fixture::regions::<f64>(false, true)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let result = heap.import_readings(
        prepared.graph,
        prepared.coverage,
        prepared.companions,
        &regions,
        DependencyPolicy::Entries,
        &mut ledger()?,
    );
    assert!(matches!(
        result,
        Err(LiveBindingError::Exact(
            nrz_bindings::readings::algebra::Fault {
                kind: nrz_bindings::readings::algebra::FaultKind::Exhausted { .. },
                ..
            }
        ))
    ));
    assert_eq!(heap.descriptors.iter().count(), 0);
    Ok(())
}
