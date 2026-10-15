// SPDX-License-Identifier: AGPL-3.0-only
//! Original coordinate construction remains the actual input of symbolic lowering.

use nrz_cas::limits::{Limits, LimitsOverride};
use nrz_cas::proof::Statement;
use nrz_cas::syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort};

use crate::{
    Context, Declaration, Descriptor, Evidence, ExponentPolicy, Registry, SensitivityRecipe,
};

/// The derivative request uses checked coherent coordinates, not the unconverted source.
/// ´test:crate:sensitivity-checked-coordinate-association´
#[test]
fn sensitivity_checked_coordinate_association() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        Environment {
            declarations: vec![Mathematical {
                key: "x".into(),
                display_name: "x".into(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            }],
            ..Environment::default()
        },
    )?;
    let measure = crate::Measure::new(
        "percent",
        crate::Scale::Exact(crate::exact::ratio(1, 100)?),
        nrz_cas::Rational::zero(),
    )?;
    let descriptor = Descriptor::new(crate::Kind::Ratio, measure, crate::Role::Difference)?;
    builder.declare(
        "x".into(),
        Declaration::Input {
            descriptor,
            nonzero: false,
            evidence: Evidence::default(),
        },
    )?;
    let original = builder.parse("square", "x^2")?;
    let checked = builder.check(&original)?;
    assert_ne!(checked.coordinates, original.syntax);
    let recipe = builder.check_sensitivity(
        SensitivityRecipe::Square,
        &[original],
        &["x".into()],
        &LimitsOverride::default(),
    )?;
    let Statement::Jacobian { input, .. } = &recipe.jacobian().witness().judgment().statement
    else {
        return Err("missing Jacobian".into());
    };
    let cas = recipe.declarations().cas()?;
    let sources = cas.operands(input, &mut crate::exact::budget()?)?;
    assert_eq!(
        sources.as_slice(),
        std::slice::from_ref(&checked.coordinates)
    );
    assert_eq!(recipe.snapshot_id(), checked.snapshot_id());
    Ok(())
}
