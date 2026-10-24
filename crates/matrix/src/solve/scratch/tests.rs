// SPDX-License-Identifier: AGPL-3.0-only
//! Ownership refusals are independent of numerical equality and allocator address reuse.

use super::{Run, Stage};
use crate::solve_testing::{TestResult, draft};
use crate::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError,
    PrivateAllocationError as Error,
};
use nrz_bindings::Descriptor;

/// The exact planned role and scalar representation precede the allocator callback.
/// ´test:unit:solve-inventory-refuses-unplanned-and-mistyped-allocations´
#[test]
fn solve_inventory_refuses_unplanned_and_mistyped_allocations() -> TestResult {
    let run = fixture()?;
    assert!(matches!(
        run.buffer::<f64>(Stage::CorrectionRightHandSide, Some(1)),
        Err(OperationError::PrivateAllocation(Error::UnplannedMeaning))
    ));
    assert!(matches!(
        run.buffer::<f32>(Stage::Centers, Some(0)),
        Err(OperationError::PrivateAllocation(Error::Representation))
    ));
    let entered = std::cell::Cell::new(false);
    let result = run.backend(Stage::PackedRightHandSide, 0, || {
        entered.set(true);
        Ok(1.0_f64)
    });
    assert!(matches!(
        result,
        Err(OperationError::PrivateAllocation(Error::Representation))
    ));
    assert!(!entered.get());
    Ok(())
}
/// A released permit cannot be reissued and no planned array can grow beyond its finite universe.
/// ´test:unit:solve-inventory-refuses-reissue-and-unplanned-growth´
#[test]
fn solve_inventory_refuses_reissue_and_unplanned_growth() -> TestResult {
    let run = fixture()?;
    let mut values = run.buffer::<f64>(Stage::Centers, Some(0))?;
    values.push(1.0)?;
    assert!(matches!(
        values.push(2.0),
        Err(OperationError::PrivateAllocation(Error::IncompleteCoverage))
    ));
    drop(values);
    assert!(matches!(
        run.buffer::<f64>(Stage::Centers, Some(0)),
        Err(OperationError::PrivateAllocation(Error::DuplicateTicket))
    ));
    Ok(())
}
/// Equal numerical bytes cannot replace a live payload owner or erase an earlier planned stage.
/// ´test:unit:solve-publication-refuses-unticketed-and-incomplete-arrays´
#[test]
fn solve_publication_refuses_unticketed_and_incomplete_arrays() -> TestResult {
    for unticketed in [false, true] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let limits = ExecutionLimits::default();
        let bound = Binding::new(draft(1, 1)?.check(limits)?, vec![source.value(); 2], limits)
            .validate(&mut heap)?;
        let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let payload = candidate.outputs.first_mut().ok_or("payload")?;
        if unticketed {
            payload.centers = payload.centers.to_vec().into();
        } else {
            let plan = payload.centers.solve_plan().ok_or("plan")?;
            let entry = plan
                .specifications
                .iter()
                .find(|s| s.stage == Stage::Residuals)
                .ok_or("residual stage")?;
            entry.state.set(super::State::Planned);
        }
        let planned = bound.result_bindings().to_vec();
        let failure = bound
            .publish(candidate)
            .err()
            .ok_or("corrupt inventory published")?;
        assert!(matches!(
            failure.cause(),
            OperationError::PrivateAllocation(Error::UnticketedBuffer | Error::IncompleteCoverage)
        ));
        assert!(
            planned
                .iter()
                .all(|binding| binding.state() == crate::SymbolState::Unavailable)
        );
        assert_eq!(heap.descriptors.iter().count(), 1);
    }
    Ok(())
}
/// A candidate channel from an equal-shaped foreign schedule cannot transfer allocation authority.
/// ´test:unit:solve-publication-refuses-foreign-allocation-permit´
#[test]
fn solve_publication_refuses_foreign_allocation_permit() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(draft(1, 1)?.check(limits)?, vec![source.value(); 2], limits)
        .validate(&mut heap)?;
    let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let foreign = fixture()?;
    let mut values = foreign.buffer(Stage::OutputCenters, None)?;
    values.push(1.0_f64)?;
    candidate.outputs.first_mut().ok_or("payload")?.centers = values.into_candidate()?;
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("foreign permit published")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(Error::ForeignInventory)
    ));
    Ok(())
}
/// Equal-valued total and error channels cannot exchange their original allocating meanings.
/// ´test:unit:solve-publication-refuses-exchanged-companion-permits´
#[test]
fn solve_publication_refuses_exchanged_companion_permits() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(draft(1, 1)?.check(limits)?, vec![source.value(); 2], limits)
        .validate(&mut heap)?;
    let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let payload = candidate.outputs.first_mut().ok_or("payload")?;
    std::mem::swap(&mut payload.bounds, &mut payload.evaluation);
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("exchanged permits published")?;
    assert!(matches!(
        failure.cause(),
        OperationError::PrivateAllocation(Error::UnticketedBuffer)
    ));
    Ok(())
}
/// A typed backend owner must still realize the exact planned logical rectangle.
/// ´test:unit:solve-inventory-refuses-backend-shape-and-partial-candidate´
#[test]
fn solve_inventory_refuses_backend_shape_and_partial_candidate() -> TestResult {
    let run = fixture()?;
    let wrong = run.backend(Stage::PackedRightHandSide, 0, || {
        Ok(faer::Mat::<f64>::zeros(2, 2))
    });
    assert!(matches!(
        wrong,
        Err(OperationError::PrivateAllocation(Error::IncompleteCoverage))
    ));
    let partial = run.buffer::<f64>(Stage::OutputCenters, None)?;
    assert!(matches!(
        partial.into_candidate(),
        Err(OperationError::PrivateAllocation(Error::IncompleteCoverage))
    ));
    Ok(())
}
/// Public checked operation admission supplies the same original bindings as production execution.
fn fixture() -> TestResult<Run> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(draft(1, 1)?.check(limits)?, vec![source.value(); 2], limits)
        .validate(&mut heap)?;
    let budget = nrz_bindings::Budget::new(nrz_bindings::Limits::default())?;
    Ok(Run::new(&bound, 0, &budget)?)
}

#[path = "recovery_tests.rs"]
mod recovery;
