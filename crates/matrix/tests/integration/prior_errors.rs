// SPDX-License-Identifier: AGPL-3.0-only
//! Original nominal errors remain overlapping components of unchanged total bounds.

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, owner, shape};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};
use nrz_matrix::{
    Binding, Coverage, ExecutionLimits, Heap, HeapLimits, OperationDraft, PrecisionContract,
    ShapeLimits,
};
use nrz_scalar::{Precision, Scalar};

/// Both sum directions retain original errors without increasing their total bounds.
/// ´test:integration:sum-prior-errors-remain-components-of-total´
/// ´claim:matrix:sum-prior-errors-components´ Binary32 and binary64 addition and subtraction retain prior numerical errors inside unchanged total bounds.
#[test]
fn sum_prior_errors_remain_components_of_total() -> TestResult {
    sum_components::<f32>()?;
    sum_components::<f64>()
}

/// Exercise zero, partial and full numerical components in fresh stores.
fn sum_components<P: Scalar + 'static>() -> TestResult {
    for subtract in [false, true] {
        let mut original_total = None;
        for numerator in [0_u8, 1, 2] {
            let declarations = owner()?;
            let descriptor = Descriptor::ratio();
            let shape = shape(&declarations, &descriptor, 1)?;
            let mut heap = Heap::new(HeapLimits::default())?;
            let mut inputs = Vec::new();
            for center in [3_u8, 2] {
                let symbols = BindingSymbols::declare(
                    &descriptor,
                    [1, 1],
                    [true, true],
                    &mut Budget::new(Limits::default())?,
                )?;
                inputs.push(heap.import_symbols(
                    &symbols,
                    &[P::from_u8(center)],
                    Some(&[P::from_u8(1) / P::from_u8(4)]),
                    Some(&[P::from_u8(numerator) / P::from_u8(8)]),
                    Coverage::DeclaredBounds,
                )?);
            }
            let draft = if subtract {
                OperationDraft::subtract(shape.clone(), shape, ShapeLimits::default())?
            } else {
                OperationDraft::add(shape.clone(), shape, ShapeLimits::default())?
            };
            let limits = ExecutionLimits::default();
            let bound =
                Binding::from_symbols(draft.check(limits)?, inputs, limits).validate(&mut heap)?;
            let candidate = bound.evaluate(bound.normalized_inputs()?)?;
            let output = bound.publish(candidate)?.into_output();
            let output = output.first().ok_or("sum")?;
            let total = heap
                .companion(output.bound().ok_or("total")?)?
                .region()
                .get(0, 0)?
                .to_f64();
            let error = heap
                .companion(output.evaluation().ok_or("error")?)?
                .region()
                .get(0, 0)?
                .to_f64();
            let center = heap.value(output.value())?.region().get(0, 0)?.to_f64();
            let exact_center = if subtract { 1 } else { 5 };
            assert!(covers(
                center,
                error,
                4 * exact_center - i128::from(numerator),
                4
            )?);
            assert!(covers(
                center,
                error,
                4 * exact_center + i128::from(numerator),
                4
            )?);
            assert!(error <= total);
            assert_eq!(
                total.to_bits(),
                *original_total.get_or_insert_with(|| total.to_bits())
            );
            assert_eq!(
                heap.companion(output.evaluation().ok_or("error")?)?
                    .target(),
                output.value()
            );
        }
    }
    Ok(())
}

/// Both precision directions retain overlapping errors and preserve their existing total calculation.
/// ´test:integration:precision-prior-errors-remain-components-of-total´
/// ´claim:matrix:precision-prior-errors-components´ Narrowing and widening retain original nominal errors inside unchanged total bounds, including a wholly numerical total.
#[test]
fn precision_prior_errors_remain_components_of_total() -> TestResult {
    let mut original_total = None;
    for numerator in [0_u8, 1, 2] {
        let declarations = owner()?;
        let descriptor = Descriptor::ratio();
        let contract = PrecisionContract::new(shape(&declarations, &descriptor, 1)?, &descriptor)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let symbols = BindingSymbols::declare(
            &descriptor,
            [1, 1],
            [true, true],
            &mut Budget::new(Limits::default())?,
        )?;
        let source = heap.import_symbols(
            &symbols,
            &[0.1_f64],
            Some(&[0.25]),
            Some(&[f64::from(numerator) / 8.0]),
            Coverage::DeclaredBounds,
        )?;
        let source_view = heap.value_by_symbol(source.symbol(), &source)?;
        let source_handle = heap
            .companion(source_view.bound().ok_or("total")?)?
            .target();
        let limits = ExecutionLimits::default();
        let operation =
            OperationDraft::precision(vec![contract.clone()], Precision::F64, Precision::F32)
                .check(limits)?;
        let bound = Binding::<f64, f32>::convert_precision(operation, vec![source_handle], limits)
            .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let outputs = bound.publish(candidate)?.into_output();
        let output = outputs.first().ok_or("narrowed")?;
        let total = f64::from(
            heap.companion(output.bound().ok_or("total")?)?
                .region()
                .get(0, 0)?,
        );
        let error = f64::from(
            heap.companion(output.evaluation().ok_or("error")?)?
                .region()
                .get(0, 0)?,
        );
        let center = f64::from(heap.value(output.value())?.region().get(0, 0)?);
        let offset = i128::from(numerator) * 4_503_599_627_370_496;
        for target in [
            3_602_879_701_896_397 - offset,
            3_602_879_701_896_397 + offset,
        ] {
            assert!(covers(center, error, target, 36_028_797_018_963_968)?);
        }
        assert!(error <= total);
        assert_eq!(
            total.to_bits(),
            *original_total.get_or_insert_with(|| total.to_bits())
        );
        let operation = OperationDraft::precision(vec![contract], Precision::F32, Precision::F64)
            .check(limits)?;
        let bound = Binding::<f32, f64>::convert_precision(operation, vec![output.value()], limits)
            .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let outputs = bound.publish(candidate)?.into_output();
        let output = outputs.first().ok_or("widened")?;
        assert_eq!(
            heap.companion(output.evaluation().ok_or("error")?)?
                .region()
                .get(0, 0)?
                .to_bits(),
            error.to_bits()
        );
        assert_eq!(
            heap.companion(output.bound().ok_or("total")?)?
                .region()
                .get(0, 0)?
                .to_bits(),
            total.to_bits()
        );
    }
    Ok(())
}
