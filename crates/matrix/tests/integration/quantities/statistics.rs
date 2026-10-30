// SPDX-License-Identifier: AGPL-3.0-only
//! Existing source models retain both covariance scales through quantity conversions.

use super::{TestResult, context, measure};
use crate::statistics_testing::{check, vector, worked_covariance};
use nrz_bindings::{Budget, Descriptor, Limits, PairingWitness, Role, UncertaintyMode};
use nrz_matrix::{Coverage, Heap, HeapLimits, JointLimits, Quantity, QuantityRole};
use nrz_scalar::Scalar;

/// Independently converted quantities retain source lineage and both ordered covariance axes.
/// ´test:integration:quantity-conversions-retain-paired-covariance´
/// ´claim:matrix:quantity-paired-covariance´ Quantity conversions retain covariance targets, both scales and parameter error without promoting covariance into a deterministic radius.
#[test]
fn quantity_conversions_retain_paired_covariance() -> TestResult {
    paired::<f32>()?;
    paired::<f64>()
}

/// Exercise each precision using independent exact rational covariance expectations.
fn paired<P: Scalar + 'static>() -> TestResult {
    let context = context()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<P>(&mut heap, true)?;
    let binding = heap.symbol_binding(source.value())?;
    let mut budget = Budget::new(Limits::default())?;
    let source = context.capture(&heap, binding, &mut budget)?;
    let ratio = Descriptor::ratio();
    let first = Descriptor::new(
        ratio.kind().clone(),
        measure(1, 100, 0, 1)?,
        Role::Difference,
    )?;
    let second = Descriptor::new(
        ratio.kind().clone(),
        measure(1, 10, 0, 1)?,
        Role::Difference,
    )?;
    let left = context.convert(&mut heap, &source, &first, &mut budget)?;
    let right = context.convert(&mut heap, &source, &second, &mut budget)?;
    let left_handle = handle(&heap, &left)?;
    let right_handle = handle(&heap, &right)?;
    let moments = heap.moments(heap.moment_companion(left_handle)?)?;
    let model = moments.model();
    let ids = moments.source_ids().to_vec();
    check_scaled(moments.covariance(), 10_000)?;
    check_scaled(
        heap.moments(heap.moment_companion(right_handle)?)?
            .covariance(),
        100,
    )?;
    let cross =
        heap.materialize_cross_covariance(left_handle, right_handle, JointLimits::default())?;
    let cross = heap.moments(cross)?;
    check_scaled(cross.covariance(), 1_000)?;
    assert_eq!(cross.model(), model);
    assert_eq!(cross.source_ids(), ids);
    assert_eq!(
        cross.targets(),
        [left.view(&heap)?.stamp(), right.view(&heap)?.stamp()]
    );
    let descriptor =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &first, Some(&second), None)?;
    assert_eq!(cross.covariance().descriptor(0, 1)?, &descriptor);
    assert_eq!(
        cross.covariance().error_descriptor(0, 1)?.target(),
        &descriptor
    );
    let view = left.view(&heap)?;
    assert_eq!(view.coverage(), Coverage::RepresentedInputs);
    let total = heap
        .companion(view.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?
        .to_f64();
    assert!(total < 0.01);
    Ok(())
}

/// Quantity companions expose the existing raw target only after live validation.
fn handle<P: Scalar + 'static, R: QuantityRole>(
    heap: &Heap,
    quantity: &Quantity<P, R>,
) -> TestResult<nrz_matrix::Handle<nrz_matrix::Value, P>> {
    Ok(heap
        .companion(quantity.view(heap)?.bound().ok_or("total")?)?
        .target())
}

/// Both covariance-axis scales multiply the independent rational oracle.
fn check_scaled<P: Scalar + 'static>(
    view: &nrz_matrix::ParameterView<'_, P>,
    scale: i128,
) -> TestResult {
    let factor = crate::kernel_oracle::Rational::new(scale, 1)?;
    let expected = worked_covariance()?
        .into_iter()
        .map(|entry| entry.multiply(factor))
        .collect::<Result<Vec<_>, _>>()?;
    check(view, &expected)
}
