// SPDX-License-Identifier: AGPL-3.0-only
//! Private corruption cannot manufacture factor allocation authority.

use super::{Run, Stage};
use crate::solve_testing::{TestResult, draft};
use crate::{
    Binding, ExecutionId, ExecutionLimits, Extent, FactorDependency, Heap, HeapLimits,
    OperationError, PrivateAllocationError,
};
use nrz_bindings::Descriptor;

/// Backend and scalar representations refuse before either allocator closure is entered.
/// ´test:unit:factor-allocation-rejects-wrong-representation-before-allocation´
#[test]
fn factor_allocation_rejects_wrong_representation_before_allocation() -> TestResult {
    let run = fixture()?;
    let entered = std::cell::Cell::new(false);
    let result = run.backend(Stage::PackedOperator, || {
        entered.set(true);
        Ok(0.0_f64)
    });
    assert!(matches!(
        result,
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::Representation
        ))
    ));
    assert!(!entered.get());
    assert!(matches!(
        run.buffer::<f32>(Stage::InverseCandidate),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::Representation
        ))
    ));
    assert!(matches!(
        run.buffer::<f64>(Stage::Factors),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::Representation
        ))
    ));
    Ok(())
}

/// Released permits never authorize reuse or growth beyond the original exact entry universe.
/// ´test:unit:factor-allocation-rejects-reissue-and-unplanned-growth´
#[test]
fn factor_allocation_rejects_reissue_and_unplanned_growth() -> TestResult {
    let run = fixture()?;
    let mut values = run
        .buffer::<f64>(Stage::InverseCandidate)
        .map_err(|e| format!("factor candidate allocation: {e}"))?;
    values
        .push(1.0)
        .map_err(|e| format!("factor planned entry: {e}"))?;
    assert!(matches!(
        values.push(2.0),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::IncompleteCoverage
        ))
    ));
    drop(values);
    assert!(matches!(
        run.buffer::<f64>(Stage::InverseCandidate),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::DuplicateTicket
        ))
    ));
    Ok(())
}

/// Complete factor exposure requires the exact owner's ticket and every planned completed stage.
/// ´test:unit:factor-allocation-rejects-foreign-and-incomplete-coverage´
#[test]
fn factor_allocation_rejects_foreign_and_incomplete_coverage() -> TestResult {
    let run = fixture()?;
    let foreign = crate::PrivateAllocationTicket::new(ExecutionId(0), 0, 1);
    assert!(matches!(
        run.complete(144, foreign),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::ForeignInventory
        ))
    ));
    let factor = run
        .backend(Stage::Factors, || {
            Ok(faer::Mat::<f64>::identity(1, 1).partial_piv_lu())
        })
        .map_err(|e| format!("factor owner allocation: {e}"))?;
    assert!(matches!(
        run.complete(144, factor.ticket()),
        Err(OperationError::PrivateAllocation(
            PrivateAllocationError::IncompleteCoverage
        ))
    ));
    Ok(())
}

/// Build plans from the same public checked solve and original input association used by execution.
fn fixture() -> TestResult<Run> {
    let mut heap =
        Heap::new(HeapLimits::default()).map_err(|e| format!("factor plan heap: {e}"))?;
    let extent = Extent::new(1, 1).map_err(|e| format!("factor plan extent: {e}"))?;
    let source = heap
        .import_represented(extent, &Descriptor::ratio(), &[2.0_f64])
        .map_err(|e| format!("factor plan source: {e}"))?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)
        .map_err(|e| format!("factor plan draft: {e}"))?
        .check(limits)
        .map_err(|e| format!("factor plan check: {e}"))?;
    let bound = Binding::new(operation, vec![source.value(); 2], limits)
        .validate(&mut heap)
        .map_err(|e| format!("factor plan admission: {e}"))?;
    let crate::binding::NativeRecipe::Solve(contract) = &bound.operation.recipe else {
        return Err("factor plan recipe".into());
    };
    let input = bound.dependencies.first().ok_or("factor plan input")?;
    let dependency = FactorDependency::capture(input, contract, 1);
    Run::new(&bound.resources.trace, input, contract, &dependency)
        .map_err(|e| format!("factor plan allocation: {e}").into())
}
