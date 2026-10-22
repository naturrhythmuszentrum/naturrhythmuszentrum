// SPDX-License-Identifier: AGPL-3.0-only
//! Existing rational-only matrix schema boundaries refuse exact pi coefficients by name.

use nrz_bindings::{Descriptor, Kind, Measure, Role, Scale, UnitError};
use nrz_matrix::{
    Axis, AxisEntry, CoordinateMap, ExactCoefficient, Partition, ReferenceScale, ShapeError,
    ShapeLimits,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn pi_ratio() -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Ratio,
        Measure::new("pi", Scale::pi()?, Measure::coherent().offset().clone())?,
        Role::Difference,
    )?)
}

/// Normalization references need a future numerical pi enclosure before entering the old map path.
/// ´test:integration:coordinate-reference-refuses-irrational-export´
#[test]
fn coordinate_reference_refuses_irrational_export() -> TestResult {
    let quantity = Descriptor::ratio();
    let limits = ShapeLimits::default();
    let axis = Axis::new(
        vec![AxisEntry::new("ratio", quantity)?],
        Partition::whole(1, limits)?,
        limits,
    )?;
    let reference = ReferenceScale::new("pi", &pi_ratio()?)?;
    assert!(matches!(
        CoordinateMap::check(&axis, vec![reference], vec![None], limits),
        Err(ShapeError::Units(UnitError::UnsupportedExactConstantExport))
    ));
    Ok(())
}

/// Structural coefficient arithmetic cannot reinterpret pi as a rational stored coefficient.
/// ´test:integration:structural-coefficient-refuses-irrational-export´
#[test]
fn structural_coefficient_refuses_irrational_export() -> TestResult {
    assert_eq!(
        ExactCoefficient::unit(&pi_ratio()?),
        Err(ShapeError::Units(UnitError::UnsupportedExactConstantExport))
    );
    let unit = Measure::coherent();
    let irrational = pi_ratio()?.conversion_to(&Descriptor::ratio())?;
    assert_eq!(
        ExactCoefficient::new(&Descriptor::ratio(), &irrational),
        Err(ShapeError::Units(UnitError::UnsupportedExactConstantExport))
    );
    assert_eq!(
        unit.rational_scale()?,
        Measure::coherent().rational_scale()?
    );
    Ok(())
}
