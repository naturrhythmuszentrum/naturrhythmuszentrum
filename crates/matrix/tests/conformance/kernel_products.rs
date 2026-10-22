// SPDX-License-Identifier: AGPL-3.0-only
//! Rectangular, empty and work-limited products preserve exact independent coverage.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, OperationError, ShapeLimits,
};

/// Rectangular composition checks each output contraction against independently decoded fractions.
/// ´test:integration:rectangular-product-encloses-exact-dot-products´
#[test]
fn rectangular_product_encloses_exact_dot_products() -> TestResult {
    let declarations = owner()?;
    let rows = axis(2)?;
    let contracted = axis(3)?;
    let columns = axis(2)?;
    let left_shape = shape(&declarations, &contracted, &rows)?;
    let right_shape = shape(&declarations, &columns, &contracted)?;
    let first = [2.0_f64, -1.0, 0.5, 3.0, -2.0, 1.0];
    let second = [1.0_f64, 0.25, -1.0, 2.0, -3.0, 0.5];
    let mut heap = Heap::new(HeapLimits::default())?;
    let left = heap.import_represented(Extent::new(2, 3)?, &Descriptor::ratio(), &first)?;
    let right = heap.import_represented(Extent::new(3, 2)?, &Descriptor::ratio(), &second)?;
    let result = execute(
        &mut heap,
        OperationDraft::matmul(left_shape, right_shape, ShapeLimits::default())?,
        vec![left.value(), right.value()],
    )?;
    let backend_left = faer::Mat::<f64>::from_fn(2, 3, |row, column| first[row + 2 * column]);
    let backend_right = faer::Mat::<f64>::from_fn(3, 2, |row, column| second[row + 3 * column]);
    let backend = &backend_left * &backend_right;
    for column in 0..2 {
        for row in 0..2 {
            let mut target = Rational::new(0, 1)?;
            for index in 0..3 {
                target = target.add(
                    Rational::binary(first[row + 2 * index])?
                        .multiply(Rational::binary(second[index + 3 * column])?)?,
                )?;
            }
            let (center, radius, error) = entry(&heap, output(&result)?, row + 2 * column)?;
            assert!(target.covered(center, error)?);
            assert!(target.covered(center, radius)?);
            assert_eq!(center.to_bits(), backend[(row, column)].to_bits());
        }
    }
    Ok(())
}

/// Empty contractions publish typed exact zero regions without indexing an empty input.
/// ´test:integration:zero-contraction-product-publishes-exact-typed-zeros´
#[test]
fn zero_contraction_product_publishes_exact_typed_zeros() -> TestResult {
    let declarations = owner()?;
    let empty = axis(0)?;
    let rows = axis(2)?;
    let columns = axis(1)?;
    let left_shape = shape(&declarations, &empty, &rows)?;
    let right_shape = shape(&declarations, &columns, &empty)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let left = heap.import_represented(Extent::new(2, 0)?, &Descriptor::ratio(), &[] as &[f32])?;
    let right = heap.import_represented(Extent::new(0, 1)?, &Descriptor::ratio(), &[] as &[f32])?;
    let result = execute(
        &mut heap,
        OperationDraft::matmul(left_shape, right_shape, ShapeLimits::default())?,
        vec![left.value(), right.value()],
    )?;
    for row in 0..2 {
        let (center, radius, error) = entry(&heap, output(&result)?, row)?;
        assert!(Rational::new(0, 1)?.covered(center, radius)?);
        assert_eq!((center, radius, error), (0.0, 0.0, 0.0));
    }
    Ok(())
}

/// Complete product work includes the interaction contraction and every companion region.
/// ´test:integration:product-work-limits-cover-the-interaction-phase´
#[test]
fn product_work_limits_cover_the_interaction_phase() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let draft = OperationDraft::matmul(contract.clone(), contract, ShapeLimits::default())?;
    for capacity in [31, 32, 33] {
        let limits = ExecutionLimits::new(2, capacity, capacity);
        let operation = draft.clone().check(limits);
        if capacity == 31 {
            assert!(matches!(operation, Err(OperationError::ResourceLimit)));
            continue;
        }
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let bound = Binding::new(operation?, vec![source.value(), source.value()], limits)
            .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let result = bound.publish(candidate)?;
        let (center, radius, error) = entry(&heap, output(&result)?, 0)?;
        assert!(Rational::new(4, 1)?.covered(center, radius)?);
        assert!(Rational::new(4, 1)?.covered(center, error)?);
    }
    Ok(())
}
