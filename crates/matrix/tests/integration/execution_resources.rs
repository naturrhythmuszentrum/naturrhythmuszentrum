// SPDX-License-Identifier: AGPL-3.0-only
//! Public retirement keeps independent factor consumers and immutable allocation history usable.

use crate::solve_testing::{TestResult, draft};
use nrz_bindings::Descriptor;
use nrz_matrix::{Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits};
use nrz_scalar::Scalar;

/// A retired origin cannot release an external factor or a separately completed consumer.
/// ´test:integration:execution-retirement-preserves-factor-reuse-and-history´
#[test]
fn execution_retirement_preserves_factor_reuse_and_history() -> TestResult {
    lifecycle::<f32>()?;
    lifecycle::<f64>()
}
/// Both scalar formats preserve exact original allocation identity across retirement and reuse.
fn lifecycle<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[P::from_u8(2)])?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)?.check(limits)?;
    let bound =
        Binding::new(operation.clone(), vec![source.value(); 2], limits).validate(&mut heap)?;
    let factor = bound.factor()?;
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let origin = bound.publish(candidate)?;
    let original = heap.execution_inventory(origin.record())?.clone();
    heap.retire_execution(origin.record())?;
    let bound = Binding::new(operation, vec![source.value(); 2], limits).validate(&mut heap)?;
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let consumer = bound.publish(candidate)?;
    drop(factor);
    let retained = heap.execution_inventory(consumer.record())?;
    assert_eq!(retained.factors().len(), 1);
    assert_eq!(
        retained.factors()[0].ticket(),
        original.factors()[0].ticket()
    );
    assert_eq!(
        retained.backend_allowance_bytes(),
        original.backend_allowance_bytes()
    );
    assert_eq!(retained.observed_allocations(), 9);
    assert_eq!(original.observed_allocations(), 11);
    heap.retire_execution(consumer.record())?;
    assert!(heap.historical_record(origin.record()).is_ok());
    assert!(heap.historical_record(consumer.record()).is_ok());
    assert_eq!(
        heap.execution_inventory(consumer.record())?.factors().len(),
        1
    );
    assert!(heap.value(consumer.output()[0].value()).is_ok());
    assert!(matches!(
        heap.retire_execution(origin.record()),
        Err(HeapError::StaleHandle)
    ));
    let mut foreign = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        foreign.retire_execution(consumer.record()),
        Err(HeapError::ForeignHandle)
    ));
    Ok(())
}
