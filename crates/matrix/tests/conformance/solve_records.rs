// SPDX-License-Identifier: AGPL-3.0-only
//! Complete solve records preserve binding, factor, numerical and publication refusals.

use crate::recovery_testing::{policy, proof};
use crate::solve_testing::{TestResult, draft};
use nrz_accuracy::{Limits, Promotion};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, ProvenanceLimits, SolveRecordKeys,
    SolveStage,
};
use nrz_units::Descriptor;

/// Every failed boundary retains its original operands and only the numerical phases that ran.
/// ´test:integration:solve-records-retain-every-failure-stage´
#[test]
fn solve_records_retain_every_failure_stage() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut foreign = Heap::new(HeapLimits::default())?;
    let absent =
        foreign.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64])?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)?.check(limits)?;
    let refused = heap
        .solve_recorded(Binding::new(
            operation.clone(),
            vec![absent.value(); 2],
            limits,
        ))
        .err()
        .ok_or("foreign bound")?;
    assert_eq!(refused.record().stage(), SolveStage::Binding);
    assert!(refused.record().numerical().is_none());
    assert_eq!(refused.record().inputs().len(), 2);
    let zero = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[0.0_f64])?;
    let refused = heap
        .solve_recorded(Binding::new(
            operation.clone(),
            vec![zero.value(); 2],
            limits,
        ))
        .err()
        .ok_or("singular solved")?;
    assert_eq!(refused.record().stage(), SolveStage::Factorization);
    assert_eq!(
        refused
            .record()
            .numerical()
            .ok_or("factor failure")?
            .record()
            .historical_evidence(),
        []
    );
    let value = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f64])?;
    let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64])?;
    let limited = draft(1, 1)?
        .with_solve_policy(policy(0.0, 0, Promotion::Never, Limits::new(1, 10000))?)?
        .check(limits)?;
    let refused = heap
        .solve_recorded(Binding::new(
            limited,
            vec![value.value(), rhs.value()],
            limits,
        ))
        .err()
        .ok_or("budget met")?;
    assert_eq!(refused.record().stage(), SolveStage::Evaluation);
    assert!(
        proof(
            refused
                .record()
                .numerical()
                .ok_or("numerical")?
                .record()
                .historical_evidence()
        )?
        .is_complete()
    );
    heap.set_provenance_limits(ProvenanceLimits::new(100, 0, 100, 100_000, 100_000))?;
    let refused = heap
        .solve_recorded(Binding::new(operation, vec![value.value(); 2], limits))
        .err()
        .ok_or("publication exceeded limit")?;
    assert_eq!(refused.record().stage(), SolveStage::Publication);
    assert!(
        proof(
            refused
                .record()
                .numerical()
                .ok_or("publication proof")?
                .record()
                .historical_evidence()
        )?
        .factor_use()
        .is_some()
    );
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}
/// External metadata is bounded immediately below, at and above its declared maximum.
/// ´test:integration:solve-record-key-storage-has-an-explicit-boundary´
#[test]
fn solve_record_key_storage_has_an_explicit_boundary() -> TestResult {
    for count in [4095, 4096, 4097] {
        let keys = SolveRecordKeys::new(Some("p".repeat(count)), None, None, None);
        if count > 4096 {
            assert!(keys.is_err());
        } else {
            assert_eq!(keys?.plan().ok_or("plan")?.len(), count);
        }
    }
    Ok(())
}

/// Precision orchestration retains the same complete binding refusal record as a direct solve.
/// ´test:integration:retry-binding-refusal-retains-complete-request´
#[test]
fn retry_binding_refusal_retains_complete_request() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut foreign = Heap::new(HeapLimits::default())?;
    let input = foreign.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f32])?;
    let limits = ExecutionLimits::default();
    let binding = Binding::new(draft(1, 1)?.check(limits)?, vec![input.value(); 2], limits);
    let failure = heap
        .solve_recovering(binding)
        .err()
        .ok_or("foreign input admitted")?;
    let record = failure.initial_record().ok_or("binding record")?;
    assert_eq!(record.stage(), SolveStage::Binding);
    assert_eq!(record.inputs().len(), 2);
    assert!(record.numerical().is_none());
    assert!(failure.initial_failure().is_none());
    Ok(())
}
