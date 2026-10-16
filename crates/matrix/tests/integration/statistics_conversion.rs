// SPDX-License-Identifier: AGPL-3.0-only
//! Both conversion axes and source identities survive independent target representations.

use crate::conversion_testing::{TestResult, measure, owner, shape};
use crate::statistics_testing::{check, vector, worked_covariance};
use nrz_matrix::{
    Binding, ConversionContract, ConversionMode, ExecutionLimits, Heap, HeapLimits, JointLimits,
    OperationDraft,
};
use nrz_scalar::{LiteralLimits, Scalar};
use nrz_units::{Descriptor, PairingWitness, Role, UncertaintyMode};

/// Statistical and deterministic requests both preserve carried sources and both paired scale factors.
/// ´test:integration:paired-conversions-scale-both-covariance-axes´
#[test]
fn paired_conversions_scale_both_covariance_axes() -> TestResult {
    paired::<f32>()?;
    paired::<f64>()
}
/// Two presentations of the same original vector retain their full cross covariance.
fn paired<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let quantity = Descriptor::ratio();
    let first = Descriptor::new(
        quantity.kind().clone(),
        measure(1, 100, 0, 1)?,
        Role::Difference,
    )?;
    let second = Descriptor::new(
        quantity.kind().clone(),
        measure(1, 10, 0, 1)?,
        Role::Difference,
    )?;
    let mut contracts = Vec::new();
    for (target, mode) in [
        (&first, ConversionMode::Statistical),
        (&second, ConversionMode::Deterministic),
    ] {
        contracts.push(ConversionContract::new(
            shape(&declarations, &quantity, 2)?,
            shape(&declarations, target, 2)?,
            declarations.conversion_map(&quantity, target)?,
            LiteralLimits::default(),
            mode,
        )?);
    }
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<P>(&mut heap, true)?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        OperationDraft::convert(contracts).check(limits)?,
        vec![source.value(); 2],
        limits,
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outputs = bound.publish(candidate)?.into_output();
    let left = outputs.first().ok_or("left representation")?;
    let right = outputs.get(1).ok_or("right representation")?;
    let first_view = heap.moments(heap.moment_companion(left.value())?)?;
    let model = first_view.model();
    let ids = first_view.source_ids().to_vec();
    check_scaled(first_view.covariance(), 10000)?;
    check_scaled(
        heap.moments(heap.moment_companion(right.value())?)?
            .covariance(),
        100,
    )?;
    let cross =
        heap.materialize_cross_covariance(left.value(), right.value(), JointLimits::default())?;
    let view = heap.moments(cross)?;
    check_scaled(view.covariance(), 1000)?;
    assert_eq!(view.model(), model);
    assert_eq!(view.source_ids(), ids);
    assert_eq!(
        view.targets(),
        [
            heap.value(left.value())?.stamp(),
            heap.value(right.value())?.stamp()
        ]
    );
    let descriptor =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &first, Some(&second), None)?;
    assert_eq!(view.covariance().descriptor(0, 1)?, &descriptor);
    assert_eq!(
        view.covariance().error_descriptor(0, 1)?.target(),
        &descriptor
    );
    Ok(())
}
/// Independent exact scaling checks all entries, including the off-diagonal pair.
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

/// Point origins shift declared means while covariance uses only difference scales.
/// ´test:integration:point-conversion-shifts-means-without-translating-covariance´
#[test]
fn point_conversion_shifts_means_without_translating_covariance() -> TestResult {
    let declarations = owner()?;
    let point = Descriptor::new(
        nrz_units::Kind::Named(
            "temperature".into(),
            Box::new(nrz_units::Dimension::base(
                nrz_units::BaseDimension::Temperature,
            )),
        ),
        nrz_units::Measure::coherent(),
        Role::Point,
    )?;
    let target = Descriptor::new(
        point.kind().clone(),
        measure(1, 1, 27315, 100)?,
        Role::Point,
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(nrz_matrix::Extent::new(1, 1)?, &point, &[300.0_f64])?;
    crate::statistics_testing::attach(&mut heap, source.value(), &[0.25], true)?;
    let conversion = ConversionContract::new(
        shape(&declarations, &point, 1)?,
        shape(&declarations, &target, 1)?,
        declarations.conversion_map(&point, &target)?,
        LiteralLimits::default(),
        ConversionMode::Statistical,
    )?;
    let output = crate::conversion_testing::execute(&mut heap, conversion, source.value())?;
    let view = heap.moments(heap.moment_companion(output.value())?)?;
    check(
        view.covariance(),
        &[crate::kernel_oracle::Rational::new(1, 4)?],
    )?;
    check(
        view.means().ok_or("converted means")?,
        &[crate::kernel_oracle::Rational::new(537, 20)?],
    )?;
    assert_eq!(
        view.means().ok_or("means")?.mean_descriptor(),
        Some(&target)
    );
    Ok(())
}
