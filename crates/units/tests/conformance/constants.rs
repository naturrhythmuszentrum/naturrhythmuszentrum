// SPDX-License-Identifier: AGPL-3.0-only
//! Exact map spellings, reference identity, point origins and bounded export refusals.

use nrz_cas::{
    Rational,
    limits::{Budget, Limits, LimitsOverride},
};
use nrz_units::{Declaration, Descriptor, Evidence, Kind, MapError, MapKind, Measure, Role, Scale};

use crate::testing::declarations::{TestResult, context, length};

/// Build exact test descriptors from literal definitions, never production conversion results.
fn measure(name: &str, scale: &str, offset: &str) -> TestResult<Measure> {
    let mut ledger = Budget::new(Limits::default())?;
    Ok(Measure::new(
        name,
        Scale::Exact(Rational::parse(scale, &mut ledger)?),
        Rational::parse(offset, &mut ledger)?,
    )?)
}

/// Retain an exact declaration used only as a descriptor reference or point origin.
fn constant(
    builder: &mut nrz_units::Context,
    key: &str,
    descriptor: &Descriptor,
    value: &str,
) -> TestResult {
    builder.declare(
        key.into(),
        Declaration::Constant {
            descriptor: descriptor.clone(),
            value: Rational::parse(value, &mut Budget::new(Limits::default())?)?,
            evidence: Evidence::default(),
        },
    )?;
    Ok(())
}

/// Fahrenheit-to-Celsius coefficients and their inverse are exported as exact definitions.
/// ´test:integration:descriptor-map-exact-affine-spellings´
#[test]
fn descriptor_map_exact_affine_spellings() -> TestResult {
    let temperature = Kind::Named(
        "temperature".into(),
        Box::new(nrz_units::Dimension::base(
            nrz_units::BaseDimension::Temperature,
        )),
    );
    let fahrenheit = Descriptor::new(
        temperature.clone(),
        measure("F", "5/9", "45967/180")?,
        Role::Point,
    )?;
    let celsius = Descriptor::new(temperature, measure("C", "1", "27315/100")?, Role::Point)?;
    let frozen = context(&[])?.freeze()?;
    let map = frozen.conversion_map(&fahrenheit, &celsius)?;
    let limits = LimitsOverride::default();
    assert_eq!(map.scale().export(&limits)?, "5/9");
    assert_eq!(map.shift().export(&limits)?, "-160/9");
    assert_eq!(map.id().source().descriptor(), Some(&fahrenheit));
    assert_eq!(map.id().target().descriptor(), Some(&celsius));
    assert_eq!(map.scale().map_id(), map.id());
    assert_eq!(map.shift().map_id(), map.id());
    let inverse = map.inverse()?;
    assert_eq!(inverse.scale().export(&limits)?, "9/5");
    assert_eq!(inverse.shift().export(&limits)?, "32");
    assert_eq!(inverse.inverse()?, map);
    let difference = frozen.conversion_map(&fahrenheit.difference(), &celsius.difference())?;
    assert_eq!(difference.shift().export(&limits)?, "0");
    Ok(())
}

/// Equal reference magnitudes retain distinct declaration identities and reversible maps.
/// ´test:integration:descriptor-reference-map-identities´
#[test]
fn descriptor_reference_map_identities() -> TestResult {
    let distance = length()?;
    let centimetres = Descriptor::new(
        distance.kind().clone(),
        measure("cm", "1/100", "0")?,
        Role::Difference,
    )?;
    let mut builder = context(&["reference", "same"])?;
    constant(&mut builder, "reference", &centimetres, "200")?;
    constant(&mut builder, "same", &distance, "2")?;
    let frozen = builder.freeze()?;
    let map = frozen.normalization_map(&centimetres, "reference", None)?;
    let other = frozen.normalization_map(&centimetres, "same", None)?;
    let limits = LimitsOverride::default();
    assert_eq!(map.scale().export(&limits)?, "1/200");
    assert_eq!(map.shift().export(&limits)?, "0");
    assert_eq!(
        map.id()
            .reference()
            .ok_or("missing named reference")?
            .name(),
        "reference"
    );
    assert_ne!(map.id(), other.id());
    assert_eq!(map.scale().export(&limits)?, other.scale().export(&limits)?);
    let inverse = map.inverse()?;
    assert_eq!(inverse.id().kind(), MapKind::Restoration);
    assert_eq!(inverse.scale().export(&limits)?, "200");
    assert_eq!(inverse.inverse()?, map);
    map.validate_snapshot(&frozen)?;
    assert!(matches!(
        map.validate_snapshot(&context(&[])?.freeze()?),
        Err(MapError::Snapshot(_))
    ));
    Ok(())
}

/// A point normalization removes only its named origin and restores that same exact map.
/// ´test:integration:descriptor-point-origin-normalization´
#[test]
fn descriptor_point_origin_normalization() -> TestResult {
    let kind = Kind::Named(
        "temperature".into(),
        Box::new(nrz_units::Dimension::base(
            nrz_units::BaseDimension::Temperature,
        )),
    );
    let point = Descriptor::new(kind, measure("C", "1", "27315/100")?, Role::Point)?;
    let difference = point.difference();
    let mut builder = context(&["reference", "origin", "equal_origin"])?;
    constant(&mut builder, "reference", &difference, "10")?;
    constant(&mut builder, "origin", &point, "20")?;
    constant(&mut builder, "equal_origin", &point, "20")?;
    let frozen = builder.freeze()?;
    let map = frozen.normalization_map(&point, "reference", Some("origin"))?;
    let other = frozen.normalization_map(&point, "reference", Some("equal_origin"))?;
    let limits = LimitsOverride::default();
    assert_eq!(map.scale().export(&limits)?, "1/10");
    assert_eq!(map.shift().export(&limits)?, "-2");
    assert_eq!(map.id().origin(), Some("origin"));
    assert_ne!(map.id(), other.id());
    assert_eq!(map.inverse()?.scale().export(&limits)?, "10");
    assert_eq!(map.inverse()?.shift().export(&limits)?, "20");
    assert!(matches!(
        frozen.normalization_map(&point, "reference", None),
        Err(MapError::Origin)
    ));
    assert!(matches!(
        frozen.normalization_map(&difference, "reference", Some("origin")),
        Err(MapError::Origin)
    ));
    Ok(())
}

/// Nonconstant and nonpositive references remain explicit refusals despite nonzero input claims.
/// ´test:integration:descriptor-map-reference-refusals´
#[test]
fn descriptor_map_reference_refusals() -> TestResult {
    let descriptor = length()?;
    let mut builder = context(&["runtime", "zero", "negative", "point"])?;
    builder.declare(
        "runtime".into(),
        Declaration::Input {
            descriptor: descriptor.clone(),
            nonzero: true,
            evidence: Evidence::default(),
        },
    )?;
    constant(&mut builder, "zero", &descriptor, "0")?;
    constant(&mut builder, "negative", &descriptor, "-1")?;
    let point = Descriptor::new(descriptor.kind().clone(), Measure::coherent(), Role::Point)?;
    constant(&mut builder, "point", &point, "1")?;
    let frozen = builder.freeze()?;
    for key in ["runtime", "missing"] {
        assert!(matches!(
            frozen.normalization_map(&descriptor, key, None),
            Err(MapError::ConstantDeclaration(_))
        ));
    }
    for key in ["zero", "negative", "point"] {
        assert!(matches!(
            frozen.normalization_map(&descriptor, key, None),
            Err(MapError::Reference)
        ));
    }
    assert!(
        frozen
            .conversion_map(&descriptor, &Descriptor::ratio())
            .is_err()
    );
    assert!(frozen.conversion_map(&descriptor, &point).is_err());
    Ok(())
}

/// Exact spelling export enforces a byte/work budget instead of silently approximating a coefficient.
/// ´test:integration:descriptor-constant-export-limit´
#[test]
fn descriptor_constant_export_limit() -> TestResult {
    let frozen = context(&[])?.freeze()?;
    let map = frozen.conversion_map(&Descriptor::ratio(), &Descriptor::ratio())?;
    let limits = LimitsOverride {
        output_bytes: Some(1),
        ..LimitsOverride::default()
    };
    assert!(matches!(
        map.scale().export(&limits),
        Err(MapError::Algebra(nrz_cas::Fault {
            kind: nrz_cas::FaultKind::Exhausted {
                budget: nrz_cas::limits::BudgetKind::OutputBytes,
                ..
            },
            ..
        }))
    ));
    assert_eq!(map.scale().export(&LimitsOverride::default())?, "1");
    Ok(())
}
