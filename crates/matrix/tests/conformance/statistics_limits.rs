// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical work and complete bundle storage boundaries refuse atomically.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, owner, shape};
use crate::statistics_testing::{check, scalar};
use nrz_matrix::{
    Binding, ExecutionLimits, Heap, HeapLimits, OperationDraft, OperationError, ShapeLimits,
};

/// The complete statistical phase admits exactly its published allowance and refuses one less.
/// ´test:integration:statistical-work-limit-below-at-and-above´
#[test]
fn statistical_work_limit_below_at_and_above() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let input = shape(&declarations, &coordinate, &coordinate)?;
    let operation = OperationDraft::add(input.clone(), input, ShapeLimits::default())?
        .check(ExecutionLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = scalar::<f64>(&mut heap)?;
    let required = Binding::new(
        operation.clone(),
        vec![source.value(); 2],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?
    .statistical_work();
    for allowance in [required - 1, required, required + 1] {
        let bound = Binding::new(
            operation.clone(),
            vec![source.value(); 2],
            ExecutionLimits::new(2, allowance, allowance),
        )
        .validate(&mut heap);
        if allowance < required {
            assert!(matches!(bound, Err(OperationError::ResourceLimit)));
        } else {
            let bound = bound?;
            let candidate = bound.evaluate(bound.normalized_inputs()?)?;
            let outcome = bound.publish(candidate)?;
            let output = outcome.output().first().ok_or("missing output")?;
            check(
                heap.moments(heap.moment_companion(output.value())?)?
                    .covariance(),
                &[Rational::new(1, 1)?],
            )?;
        }
    }
    Ok(())
}

/// Storage accounts the value, deterministic regions and every statistical parameter pair together.
/// ´test:integration:statistical-storage-limit-below-at-and-above´
#[test]
fn statistical_storage_limit_below_at_and_above() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let input = shape(&declarations, &coordinate, &coordinate)?;
    let operation = OperationDraft::add(input.clone(), input, ShapeLimits::default())?
        .check(ExecutionLimits::default())?;
    for capacity in [13, 14, 15] {
        let mut heap = Heap::new(HeapLimits::new(32, capacity, 32))?;
        let source = scalar::<f64>(&mut heap)?;
        let bound = Binding::new(
            operation.clone(),
            vec![source.value(); 2],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let result = bound.publish(candidate);
        if capacity < 14 {
            assert!(matches!(
                result.err().ok_or("incomplete storage admitted")?.cause(),
                OperationError::Heap(nrz_matrix::HeapError::Capacity)
            ));
            assert_eq!(heap.provenance_usage().records(), 0);
            assert!(heap.value(source.value()).is_ok());
        } else {
            let result = result?;
            let value = result
                .output()
                .first()
                .ok_or("missing complete output")?
                .value();
            check(
                heap.moments(heap.moment_companion(value)?)?.covariance(),
                &[Rational::new(1, 1)?],
            )?;
        }
    }
    Ok(())
}
