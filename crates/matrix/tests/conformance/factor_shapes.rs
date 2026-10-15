// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor factor admission is distinct from numerical reconstruction and root existence.

use crate::shape_testing::{ResultOf, axis, dense, owner, quantity, ratio_shape, ratios};
use nrz_matrix::{CholeskyRootContract, EntryDraft, QrShapeContract, ShapeError, ShapeLimits};
use nrz_units::{Descriptor, Dimension, ExponentPolicy, Measure, Registry, Role, Scale, UnitError};

/// QR envelopes retain the exact intermediate coordinate identities.
/// ´test:integration:qr-shape-retains-intermediate-axis´
#[test]
fn qr_shape_retains_intermediate_axis() -> ResultOf<()> {
    let snapshot = ratios()?;
    let input_axis = axis(&["input"], &Descriptor::ratio())?;
    let output_axis = axis(&["output"], &Descriptor::ratio())?;
    let middle = axis(&["middle"], &Descriptor::ratio())?;
    let input = ratio_shape(&snapshot, &input_axis, &output_axis)?;
    let orthogonal = ratio_shape(&snapshot, &middle, &output_axis)?;
    let triangular = ratio_shape(&snapshot, &input_axis, &middle)?;
    let contract =
        QrShapeContract::check(&input, &orthogonal, &triangular, ShapeLimits::default())?;
    assert_eq!(contract.intermediate(), &middle);
    assert_eq!(contract.reconstruction().domain(), &input_axis);
    let alien = axis(&["middle"], &Descriptor::ratio())?;
    let wrong = ratio_shape(&snapshot, &input_axis, &alien)?;
    assert!(matches!(
        QrShapeContract::check(&input, &orthogonal, &wrong, ShapeLimits::default()),
        Err(ShapeError::AxisMismatch)
    ));
    Ok(())
}

/// Cholesky scalar root descriptors require both the exponent policy and a semantic root rule.
/// ´test:integration:cholesky-root-policy-refusal-pairs´
#[test]
fn cholesky_root_policy_refusal_pairs() -> ResultOf<()> {
    let length = quantity("length", [1, 0, 0, 0, 0, 0, 0])?;
    let area = quantity("area", [2, 0, 0, 0, 0, 0, 0])?;
    let input_axis = axis(&["ratio"], &Descriptor::ratio())?;
    let output_axis = axis(&["area"], &area)?;
    let literals = Dimension::integers([2, 0, 0, 0, 0, 0, 0])?;
    let double = Measure::new(
        "two",
        Scale::Exact(literals.exponents().first().ok_or("two")?.clone()),
        Measure::coherent().offset().clone(),
    )?;
    let half = Measure::coherent().conversion_to(&double, Role::Difference)?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_power(area.kind().clone(), half.scale, length.kind().clone())?;
    let snapshot = owner(registry)?;
    let operator = dense(
        &snapshot,
        &input_axis,
        &output_axis,
        vec![EntryDraft::Declared(area.clone())],
    )?;
    assert_eq!(CholeskyRootContract::check(&operator)?.root(), &length);
    let unregistered = dense(
        &ratios()?,
        &input_axis,
        &output_axis,
        vec![EntryDraft::Declared(area)],
    )?;
    assert!(matches!(
        CholeskyRootContract::check(&unregistered),
        Err(ShapeError::Units(UnitError::UnsupportedKindRule))
    ));
    let integral = owner(Registry::new(ExponentPolicy::Integral))?;
    let length_axis = axis(&["length"], &length)?;
    let fractional = dense(
        &integral,
        &input_axis,
        &length_axis,
        vec![EntryDraft::Declared(length)],
    )?;
    assert!(matches!(
        CholeskyRootContract::check(&fractional),
        Err(ShapeError::Units(UnitError::NonIntegralDimension))
    ));
    assert_eq!(operator.extent().entries(), 1);
    Ok(())
}
