// SPDX-License-Identifier: AGPL-3.0-only
//! Typed angle and full-cycle relations retain every deterministic error through restoration.

use super::{conversion, oracle, owner, pi};
use crate::{
    conversion_testing::{TestResult, execute, shape},
    kernel_oracle::Rational,
};
use nrz_bindings::{Budget, Limits, MapError, UnitError};
use nrz_matrix::{Coverage, Extent, Heap, HeapLimits, PublishedBundle};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Angle conversions enclose independent pi truth and declared endpoint boxes in both formats.
/// ´test:integration:pi-conversion-angle-roundtrip´
#[test]
fn pi_conversion_angle_roundtrip() -> TestResult {
    angle::<f32>()?;
    angle::<f64>()
}

/// Exercise inverse pi, exact cancellation and forward pi with the same constant owner.
fn angle<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let radian = declarations
        .registry()
        .measure("rad")
        .ok_or("radian")?
        .coherent();
    let degree = declarations.registry().measure("degree").ok_or("degree")?;
    let turn = declarations.registry().measure("turn").ok_or("turn")?;
    let coordinates = shape(&declarations, &radian, 1)?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &radian,
        &[P::ONE],
        &[P::ONE / P::from_u8(8)],
    )?;
    let first = execute(
        &mut heap,
        conversion(
            &declarations,
            [&radian, degree],
            [&coordinates; 2],
            &circle,
            false,
        )?,
        source.value(),
    )?;
    check(&heap, first, oracle::factor(180, 1, true, 48)?)?;
    let degree_restored = execute(
        &mut heap,
        conversion(
            &declarations,
            [degree, &radian],
            [&coordinates; 2],
            &circle,
            false,
        )?,
        first.value(),
    )?;
    check(&heap, degree_restored, [Rational::new(1, 1)?; 2])?;
    let direct_turn = execute(
        &mut heap,
        conversion(
            &declarations,
            [&radian, turn],
            [&coordinates; 2],
            &circle,
            false,
        )?,
        source.value(),
    )?;
    check(&heap, direct_turn, oracle::factor(1, 2, true, 56)?)?;
    let second = execute(
        &mut heap,
        conversion(
            &declarations,
            [degree, turn],
            [&coordinates; 2],
            &circle,
            false,
        )?,
        first.value(),
    )?;
    check(&heap, second, oracle::factor(1, 2, true, 56)?)?;
    let restored = execute(
        &mut heap,
        conversion(
            &declarations,
            [turn, &radian],
            [&coordinates; 2],
            &circle,
            false,
        )?,
        second.value(),
    )?;
    check(&heap, restored, [Rational::new(1, 1)?; 2])
}

/// Frequency conversion is an explicit full-cycle relation and preserves kind mismatch refusals.
/// ´test:integration:pi-conversion-frequency-roundtrip´
#[test]
fn pi_conversion_frequency_roundtrip() -> TestResult {
    frequency::<f32>()?;
    frequency::<f64>()
}

/// Carry one declared frequency interval through angular frequency and back.
fn frequency<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let hertz = declarations
        .registry()
        .measure("Hz")
        .ok_or("hertz")?
        .coherent();
    let angular = declarations
        .registry()
        .measure("rad/s")
        .ok_or("angular")?
        .coherent();
    let rate = declarations.registry().measure("s^-1").ok_or("rate")?;
    let cyclic_shape = shape(&declarations, &hertz, 1)?;
    let angular_shape = shape(&declarations, &angular, 1)?;
    assert_eq!(hertz.dimension(), angular.dimension());
    assert!(matches!(
        declarations.conversion_map(&hertz, &angular),
        Err(MapError::Units(UnitError::KindMismatch))
    ));
    assert!(matches!(
        declarations.frequency_map(rate, &angular),
        Err(MapError::Units(UnitError::KindMismatch))
    ));
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &hertz,
        &[P::ONE],
        &[P::ONE / P::from_u8(8)],
    )?;
    let first = execute(
        &mut heap,
        conversion(
            &declarations,
            [&hertz, &angular],
            [&cyclic_shape, &angular_shape],
            &circle,
            true,
        )?,
        source.value(),
    )?;
    check(&heap, first, oracle::factor(2, 1, false, 54)?)?;
    let restored = execute(
        &mut heap,
        conversion(
            &declarations,
            [&angular, &hertz],
            [&angular_shape, &cyclic_shape],
            &circle,
            true,
        )?,
        first.value(),
    )?;
    check(&heap, restored, [Rational::new(1, 1)?; 2])
}

/// Full errors cover original declared endpoints while numerical errors cover nominal truth.
pub(super) fn check<P: Scalar + 'static>(
    heap: &Heap,
    output: PublishedBundle<P>,
    bounds: [Rational; 2],
) -> TestResult {
    let view = heap.value(output.value())?;
    let center = view.region().get(0, 0)?.to_f64();
    let numerical = heap
        .companion(output.evaluation().ok_or("numerical error")?)?
        .region()
        .get(0, 0)?
        .to_f64();
    let total = heap
        .companion(output.bound().ok_or("total error")?)?
        .region()
        .get(0, 0)?
        .to_f64();
    oracle::covers(bounds, center, numerical)?;
    oracle::covers(
        [
            bounds[0].multiply(Rational::new(7, 8)?)?,
            bounds[1].multiply(Rational::new(9, 8)?)?,
        ],
        center,
        total,
    )?;
    assert!(numerical <= total);
    assert_eq!(view.coverage(), Coverage::DeclaredBounds);
    Ok(())
}

/// An irrational affine shift uses the same pi owner and remains distinct from difference conversion.
/// ´test:integration:pi-conversion-affine-point-shift´
#[test]
fn pi_conversion_affine_point_shift() -> TestResult {
    point::<f32>()?;
    point::<f64>()
}

/// A point at the declared target origin maps to zero and restores with complete errors.
fn point<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let degree = declarations.registry().measure("degree").ok_or("degree")?;
    let source = nrz_bindings::Descriptor::new(
        nrz_bindings::Kind::Angle,
        nrz_bindings::Measure::coherent(),
        nrz_bindings::Role::Point,
    )?;
    let target = nrz_bindings::Descriptor::new(
        nrz_bindings::Kind::Angle,
        nrz_bindings::Measure::new(
            "offset degree",
            nrz_bindings::Scale::Expression(degree.measure().scale().clone()),
            nrz_bindings::Measure::coherent().rational_scale()?.clone(),
        )?,
        nrz_bindings::Role::Point,
    )?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let coordinates = shape(&declarations, &source, 1)?;
    let original = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &source,
        &[P::ONE],
        &[P::ONE / P::from_u8(8)],
    )?;
    let first = execute(
        &mut heap,
        conversion(
            &declarations,
            [&source, &target],
            [&coordinates; 2],
            &circle,
            false,
        )?,
        original.value(),
    )?;
    let center = heap.value(first.value())?.region().get(0, 0)?.to_f64();
    let numerical = heap
        .companion(first.evaluation().ok_or("error")?)?
        .region()
        .get(0, 0)?
        .to_f64();
    assert!(Rational::new(0, 1)?.covered(center, numerical)?);
    let restored = execute(
        &mut heap,
        conversion(
            &declarations,
            [&target, &source],
            [&coordinates; 2],
            &circle,
            false,
        )?,
        first.value(),
    )?;
    check(&heap, restored, [Rational::new(1, 1)?; 2])?;
    assert!(
        declarations
            .conversion_map(&source, &target.difference())
            .is_err()
    );
    Ok(())
}
