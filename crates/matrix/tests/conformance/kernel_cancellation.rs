// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact truth survives catastrophic cancellation in backend product centers.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use nrz_bindings::Descriptor;
use nrz_matrix::{Extent, Heap, HeapLimits, OperationDraft, ShapeLimits};
use nrz_scalar::Scalar;

/// A tiny exact dot beside huge canceling terms is covered in both candidate formats.
/// ´test:integration:product-cancellation-retains-an-independent-enclosure´
#[test]
fn product_cancellation_retains_an_independent_enclosure() -> TestResult {
    let large_single = 16_777_216.0_f32;
    let first = faer::Mat::from_fn(1, 3, |_, column| [large_single, 1.0, -large_single][column]);
    let second = faer::Mat::<f32>::from_fn(3, 1, |_, _| 1.0);
    let expected = &first * &second;
    cancellation(large_single, f64::from(expected[(0, 0)]))?;
    let large_double = 9_007_199_254_740_992.0_f64;
    let first = faer::Mat::from_fn(1, 3, |_, column| [large_double, 1.0, -large_double][column]);
    let second = faer::Mat::<f64>::from_fn(3, 1, |_, _| 1.0);
    let expected = &first * &second;
    cancellation(large_double, expected[(0, 0)])
}

/// The rational oracle decodes all operands and derives truth without the backend reduction.
fn cancellation<P: Scalar + 'static>(large: P, backend: f64) -> TestResult {
    let declarations = owner()?;
    let row = axis(1)?;
    let contracted = axis(3)?;
    let column = axis(1)?;
    let operator = shape(&declarations, &contracted, &row)?;
    let vector = shape(&declarations, &column, &contracted)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let left = heap.import_represented(
        Extent::new(1, 3)?,
        &Descriptor::ratio(),
        &[large, P::ONE, -large],
    )?;
    let right = heap.import_represented(Extent::new(3, 1)?, &Descriptor::ratio(), &[P::ONE; 3])?;
    let result = execute(
        &mut heap,
        OperationDraft::matmul(operator, vector, ShapeLimits::default())?,
        vec![left.value(), right.value()],
    )?;
    let exact = Rational::binary(large.to_f64())?
        .add(Rational::new(1, 1)?)?
        .subtract(Rational::binary(large.to_f64())?)?;
    assert!(exact.covered(1.0, 0.0)?);
    let (center, radius, error) = entry(&heap, output(&result)?, 0)?;
    assert_eq!(center.to_bits(), backend.to_bits());
    assert!(exact.covered(center, error)?);
    assert!(exact.covered(center, radius)?);
    assert!(Rational::binary(radius)?.at_most(Rational::new(4, 1)?)?);
    Ok(())
}
