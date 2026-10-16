// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic public sums and fixed maps are checked against independent integer fractions.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use nrz_matrix::{
    Extent, FixedCoefficient, Heap, HeapLimits, LinearMap, OperationDraft, ShapeLimits,
};
use nrz_scalar::{LiteralLimits, Scalar};
use nrz_units::Descriptor;

/// All corners, nominal centers and repeated-input multiplicity use an independent rational oracle.
/// ´test:integration:sums-and-differences-enclose-rational-corners´
#[test]
fn sums_and_differences_enclose_rational_corners() -> TestResult {
    sum_format::<f32>()?;
    sum_format::<f64>()
}

/// Run both signed laws on exactly represented small operands in one sealed format.
fn sum_format<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(3)],
        &[P::ONE / P::from_u8(4)],
    )?;
    for subtract in [false, true] {
        let draft = if subtract {
            OperationDraft::subtract(contract.clone(), contract.clone(), ShapeLimits::default())?
        } else {
            OperationDraft::add(contract.clone(), contract.clone(), ShapeLimits::default())?
        };
        let result = execute(&mut heap, draft, vec![source.value(), source.value()])?;
        let (center, radius, error) = entry(&heap, output(&result)?, 0)?;
        let nominal = Rational::new(if subtract { 0 } else { 6 }, 1)?;
        assert!(nominal.covered(center, error)?);
        assert!(Rational::new(1, 2)?.at_most(Rational::binary(radius)?)?);
        for first in [-1, 1] {
            for second in [-1, 1] {
                let left = Rational::new(12 + first, 4)?;
                let right = Rational::new(12 + second, 4)?;
                let target = if subtract {
                    left.subtract(right)?
                } else {
                    left.add(right)?
                };
                assert!(target.covered(center, radius)?);
            }
        }
    }
    Ok(())
}

/// Negative fixed coefficients, offsets and input radii follow the exact affine law in both formats.
/// ´test:integration:fixed-affine-map-encloses-exact-rational-image´
#[test]
fn fixed_affine_map_encloses_exact_rational_image() -> TestResult {
    affine_format::<f32>()?;
    affine_format::<f64>()
}

/// Materialize a two-row fixed map with independently computed nominal and extreme images.
fn affine_format<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let column = axis(1)?;
    let input = shape(&declarations, &column, &coordinates)?;
    let operator = shape(&declarations, &coordinates, &coordinates)?;
    let coefficient = |value| FixedCoefficient::represented(Descriptor::ratio(), value);
    let coefficients = [1.0, -1.0, 2.0, 1.0]
        .into_iter()
        .map(coefficient)
        .collect::<Result<_, _>>()?;
    let offsets = [1.0, -1.0]
        .into_iter()
        .map(coefficient)
        .collect::<Result<_, _>>()?;
    let map = LinearMap::new(
        operator,
        input,
        coefficients,
        offsets,
        LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[P::from_u8(2), P::from_u8(3)],
        &[P::ONE / P::from_u8(8), P::ONE / P::from_u8(4)],
    )?;
    let result = execute(
        &mut heap,
        OperationDraft::linear_map(map),
        vec![source.value()],
    )?;
    for (index, nominal) in [9, 0].into_iter().enumerate() {
        let (center, radius, error) = entry(&heap, output(&result)?, index)?;
        assert!(Rational::new(nominal, 1)?.covered(center, error)?);
        let extent = Rational::new(if index == 0 { 5 } else { 3 }, 8)?;
        assert!(
            Rational::new(nominal, 1)?
                .add(extent)?
                .covered(center, radius)?
        );
        assert!(
            Rational::new(nominal, 1)?
                .subtract(extent)?
                .covered(center, radius)?
        );
    }
    Ok(())
}

/// Exact descriptor coefficients are lowered outward instead of treated as represented binary constants.
/// ´test:integration:linear-map-encloses-exact-descriptor-coefficient´
#[test]
fn linear_map_encloses_exact_descriptor_coefficient() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let ratio = Descriptor::ratio();
    let third = Descriptor::new(
        ratio.kind().clone(),
        crate::conversion_testing::measure(1, 3, 0, 1)?,
        nrz_units::Role::Difference,
    )?;
    let exact = declarations.conversion_map(&third, &ratio)?;
    let coefficient = FixedCoefficient::descriptor(ratio.clone(), exact.scale().clone())?;
    let map = LinearMap::new(
        contract.clone(),
        contract,
        vec![coefficient],
        Vec::new(),
        LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_declared_bounds(Extent::new(1, 1)?, &ratio, &[2.0_f32], &[0.25])?;
    let result = execute(
        &mut heap,
        OperationDraft::linear_map(map),
        vec![input.value()],
    )?;
    let (center, radius, error) = entry(&heap, output(&result)?, 0)?;
    assert!(
        Rational::new(2, 1)?
            .divide(Rational::new(3, 1)?)?
            .covered(center, error)?
    );
    assert!(
        Rational::new(9, 4)?
            .multiply(Rational::new(1, 3)?)?
            .covered(center, radius)?
    );
    assert!(Rational::new(7, 12)?.covered(center, radius)?);
    Ok(())
}
