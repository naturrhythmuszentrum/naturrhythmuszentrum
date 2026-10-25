// SPDX-License-Identifier: AGPL-3.0-only
//! Exact schema constants retain pi without claiming rational numerical export.

use nrz_bindings::{Descriptor, Kind, Measure, Role, Scale, UnitError};
use nrz_matrix::{
    Axis, AxisEntry, CoordinateMap, ExactCoefficient, Partition, ReferenceScale, ShapeLimits,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn pi_ratio() -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Ratio,
        Measure::new("pi", Scale::pi()?, Measure::coherent().offset().clone())?,
        Role::Difference,
    )?)
}

/// Positive pi references preserve exact reciprocal maps and their independent definition identity.
/// ´test:integration:coordinate-reference-retains-irrational-definition´
#[test]
fn coordinate_reference_retains_irrational_definition() -> TestResult {
    let quantity = Descriptor::ratio();
    let limits = ShapeLimits::default();
    let axis = Axis::new(
        vec![AxisEntry::new("ratio", quantity)?],
        Partition::whole(1, limits)?,
        limits,
    )?;
    let reference = ReferenceScale::new("pi", &pi_ratio()?)?;
    let checked = CoordinateMap::check(&axis, vec![reference.clone()], vec![None], limits)?;
    assert_eq!(checked.references(), std::slice::from_ref(&reference));
    let normalize = checked.normalization().first().ok_or("normalization")?;
    let restore = checked.restoration().first().ok_or("restoration")?;
    assert_eq!(restore.scale, *pi_ratio()?.measure().scale());
    assert_eq!(
        normalize.apply_exact(&restore.scale)?,
        *Measure::coherent().scale()
    );
    assert_ne!(reference, ReferenceScale::new("pi", &pi_ratio()?)?);
    Ok(())
}

/// Irrational units and explicit factors retain their exact definitions and refuse rational export.
/// ´test:integration:structural-coefficient-retains-irrational-definition´
#[test]
fn structural_coefficient_retains_irrational_definition() -> TestResult {
    let definition = pi_ratio()?;
    let coefficient = ExactCoefficient::unit(&definition)?;
    assert_eq!(coefficient.descriptor(), &Descriptor::ratio());
    assert_eq!(coefficient.exact_map().scale, *definition.measure().scale());
    assert_eq!(
        coefficient.exact_map().rational_scale(),
        Err(UnitError::UnsupportedExactConstantExport)
    );
    let unit = Measure::coherent();
    let irrational = pi_ratio()?.conversion_to(&Descriptor::ratio())?;
    assert_eq!(
        ExactCoefficient::new(&Descriptor::ratio(), &irrational)?,
        coefficient
    );
    assert_eq!(
        unit.rational_scale()?,
        Measure::coherent().rational_scale()?
    );
    Ok(())
}
