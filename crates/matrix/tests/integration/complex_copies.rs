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
