// SPDX-License-Identifier: AGPL-3.0-only
//! The worked nonlinear covariance retains physical Jacobian and covariance product meanings.

use crate::kernel_testing::{TestResult, execute, output};
use crate::shape_testing::{axis, dense, owner, quantity};
use crate::statistics_testing::{attach, check, covariance, worked_covariance};
use nrz_bindings::{
    Descriptor, Dimension, ExponentPolicy, PairingWitness, Registry, UncertaintyMode,
};
use nrz_matrix::{Elementwise, EntryDraft, Extent, Heap, HeapLimits, OperationDraft};
use nrz_scalar::Scalar;

/// The squared-voltage example carries a voltage derivative and fourth-power covariance, with independent numerical errors.
/// ´test:integration:worked-statistical-voltage-square-retains-physical-parameter-meanings´
#[test]
fn worked_statistical_voltage_square_retains_physical_parameter_meanings() -> TestResult {
    physical_square::<f32>()?;
    physical_square::<f64>()
}
/// Exact rational values are independent of the production derivative and descriptor arithmetic.
fn physical_square<P: Scalar + 'static>() -> TestResult {
    let voltage = quantity("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let squared = quantity("squared-voltage", [4, 2, -6, -2, 0, 0, 0])?;
    let (input, destination) = shapes(&voltage, &squared)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(
        Extent::new(2, 1)?,
        &voltage,
        &[P::from_u8(2), P::from_u8(3)],
    )?;
    attach(
        &mut heap,
        source.value(),
        &[
            P::ONE / P::from_u8(100),
            P::ONE / P::from_u8(200),
            P::ONE / P::from_u8(200),
            P::ONE / P::from_u8(25),
        ],
        true,
    )?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::map_elementwise(input, destination, Elementwise::Square)?,
        vec![source.value()],
    )?)?;
    let view = heap.moments(heap.moment_companion(result.value())?)?;
    let proof = view
        .checked_sensitivity()
        .ok_or("physical derivative proof")?;
    assert_eq!(proof.inputs(), std::slice::from_ref(&voltage));
    assert_eq!(proof.outputs(), std::slice::from_ref(&squared));
    let jacobian = view.jacobian().ok_or("physical derivative")?;
    assert_eq!(
        jacobian.descriptor(0, 0)?,
        &PairingWitness::sensitivity(&squared, &voltage)?
    );
    let covariance_descriptor =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &squared, Some(&squared), None)?;
    assert_eq!(view.covariance().descriptor(0, 1)?, &covariance_descriptor);
    assert_eq!(
        view.covariance().error_descriptor(0, 1)?.target(),
        &covariance_descriptor
    );
    check(
        view.covariance(),
        &covariance(&[4, 0, 0, 6], 2, &[4, 0, 0, 6], 2, 2, &worked_covariance()?)?,
    )?;
    assert!(view.means().is_none());
    Ok(())
}
/// Freeze the registered physical power relation before either shape or symbolic derivative checking.
fn shapes(
    voltage: &Descriptor,
    squared: &Descriptor,
) -> TestResult<(nrz_matrix::CheckedShape, nrz_matrix::CheckedShape)> {
    let exponents = Dimension::integers([2, 0, 0, 0, 0, 0, 0])?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_power(
        voltage.kind().clone(),
        exponents
            .exponents()
            .first()
            .ok_or("square exponent")?
            .clone(),
        squared.kind().clone(),
    )?;
    let declarations = owner(registry)?;
    let column = axis(&["ratio"], &Descriptor::ratio())?;
    let input_axis = axis(&["first", "second"], voltage)?;
    let output_axis = axis(&["first-square", "second-square"], squared)?;
    let input = dense(
        &declarations,
        &column,
        &input_axis,
        vec![EntryDraft::Declared(voltage.clone()); 2],
    )?;
    let destination = dense(
        &declarations,
        &column,
        &output_axis,
        vec![EntryDraft::Declared(squared.clone()); 2],
    )?;
    Ok((input, destination))
}
