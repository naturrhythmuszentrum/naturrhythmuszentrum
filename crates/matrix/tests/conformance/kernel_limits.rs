// SPDX-License-Identifier: AGPL-3.0-only
//! Kernel refusal boundaries retain shape, capacity and coverage reasons.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, output, owner, shape};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, OperationError, ShapeLimits,
};

/// The sum's admitted work count is tested immediately below, at and above its requirement.
/// ´test:integration:sum-work-and-scratch-boundaries-refuse-before-publication´
#[test]
fn sum_work_and_scratch_boundaries_refuse_before_publication() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let draft = OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?;
    for capacity in [7, 8, 9] {
        let limits = ExecutionLimits::new(2, capacity, capacity);
        let checked = draft.clone().check(limits);
        if capacity == 7 {
            assert!(matches!(checked, Err(OperationError::ResourceLimit)));
            continue;
        }
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let bound = Binding::new(checked?, vec![source.value(), source.value()], limits)
            .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let result = bound.publish(candidate)?;
        let (center, radius, error) = entry(&heap, output(&result)?, 0)?;
        assert!(Rational::new(4, 1)?.covered(center, radius)?);
        assert!(Rational::new(4, 1)?.covered(center, error)?);
    }
    Ok(())
}

/// Equal dimensions cannot replace ordered axis identities, and actual extents are checked at binding.
/// ´test:integration:kernel-shape-and-concrete-contract-mismatches-refuse´
#[test]
fn kernel_shape_and_concrete_contract_mismatches_refuse() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let foreign = axis(1)?;
    let wrong = shape(&declarations, &coordinate, &foreign)?;
    assert!(matches!(
        OperationDraft::add(contract.clone(), wrong, ShapeLimits::default()),
        Err(OperationError::Shape(_))
    ));
    let operation = OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?
        .check(ExecutionLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[2.0_f64, 3.0])?;
    assert!(matches!(
        Binding::new(
            operation,
            vec![source.value(), source.value()],
            ExecutionLimits::default()
        )
        .validate(&mut heap),
        Err(OperationError::InputMismatch { .. })
    ));
    assert_eq!(
        heap.value(source.value())?.region().export()?,
        vec![2.0, 3.0]
    );
    Ok(())
}

/// Mixed physical and represented-only assumptions are refused instead of silently relabeled.
/// ´test:integration:kernel-mixed-coverage-refuses-with-a-typed-reason´
#[test]
fn kernel_mixed_coverage_refuses_with_a_typed_reason() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let limits = ExecutionLimits::default();
    let operation =
        OperationDraft::add(contract.clone(), contract, ShapeLimits::default())?.check(limits)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let represented =
        heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let declared = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.125],
    )?;
    assert!(matches!(
        Binding::new(
            operation,
            vec![represented.value(), declared.value()],
            limits
        )
        .validate(&mut heap),
        Err(OperationError::Kernel(
            nrz_matrix::KernelError::CoverageMismatch
        ))
    ));
    assert!(
        Rational::new(2, 1)?.covered(heap.value(represented.value())?.region().get(0, 0)?, 0.0)?
    );
    assert!(Rational::new(2, 1)?.covered(heap.value(declared.value())?.region().get(0, 0)?, 0.0)?);
    Ok(())
}
