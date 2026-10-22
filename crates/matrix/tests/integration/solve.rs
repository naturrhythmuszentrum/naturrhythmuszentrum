// SPDX-License-Identifier: AGPL-3.0-only
//! Opaque factors, independent proof, publication and dependency invalidation.

use crate::kernel_testing::output;
use crate::solve_testing::{TestResult, draft};
use nrz_bindings::Descriptor;
use nrz_matrix::{Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits};

/// A factored solve publishes both source edges, then an RHS change invalidates its result.
/// ´test:integration:factored-solve-publication-retains-both-input-dependencies´
#[test]
fn factored_solve_publication_retains_both_input_dependencies() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[2.0_f64, 0.0, 0.0, 4.0],
    )?;
    let rhs = heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[2.0_f64, 8.0])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(2, 1)?.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let factor = bound.factor()?;
    assert_eq!(factor.execution(), bound.execution());
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let outcome = bound.publish(candidate)?;
    let result = output(&outcome)?;
    crate::solve_public::check(&heap, result, &[2.0, 0.0, 0.0, 4.0], &[2.0, 8.0], 2)?;
    let record = heap.operation_record(outcome.record())?;
    let identity = record
        .outputs()
        .first()
        .ok_or("output record")?
        .provenance();
    assert_eq!(heap.provenance(identity)?.sources().len(), 2);
    heap.compact()?;
    crate::solve_public::check(&heap, result, &[2.0, 0.0, 0.0, 4.0], &[2.0, 8.0], 2)?;
    let version = heap.value(rhs.value())?.stamp().version();
    heap.replace_represented(rhs.value(), version, &[4.0, 8.0])?;
    assert!(matches!(
        heap.value(result.value()),
        Err(HeapError::StaleEvidence)
    ));
    Ok(())
}
