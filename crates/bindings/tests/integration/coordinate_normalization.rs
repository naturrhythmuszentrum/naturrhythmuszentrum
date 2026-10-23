// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed mixed references, affine origins and exact angle cancellation through checked CAS maps.

use crate::testing::normalization::{check, constant, entries, frame};
use crate::testing::readings::{Fixture, TestResult, ledger};
use nrz_bindings::readings::{CoordinateDirection, CoordinateNormalization, EntryMeaning};
use nrz_bindings::{
    Descriptor, ExponentPolicy, Measure, OriginDefinition, ReferenceDefinition, Registry, Role,
    Scale,
};

/// The LC references retain checked inverses and independently specified per-entry rational maps.
/// ´test:integration:mixed-coordinate-images-retain-reference-inverse´
#[test]
fn mixed_coordinate_images_retain_reference_inverse() -> TestResult {
    let (mut case, coordinates) = frame()?;
    assert!(coordinates.inverse().witness().is_discharged());
    assert!(coordinates.inverse().nonsingularity().is_discharged());
    let normalization = coordinates.image(
        &mut case.cas,
        CoordinateDirection::Normalize,
        &mut ledger()?,
    )?;
    check(
        &normalization.export(&mut ledger()?)?,
        &["1/5", "0", "0", "10"],
        &["0", "0"],
    )?;
    let restoration =
        coordinates.image(&mut case.cas, CoordinateDirection::Restore, &mut ledger()?)?;
    check(
        &restoration.export(&mut ledger()?)?,
        &["5", "0", "0", "1/10"],
        &["0", "0"],
    )?;
    assert_ne!(normalization.reuse_key(), restoration.reuse_key());
    assert!(normalization.source().is_none());
    coordinates.check_entries(
        normalization.target().ok_or("target")?.entries(),
        &mut ledger()?,
    )?;
    let composed = case.cas.compose_reading_maps(
        coordinates.restoration(),
        coordinates.inverse().map(),
        &mut ledger()?,
    )?;
    let oracle = case.cas.constant_reading_map(
        &case.context,
        (2, 2),
        &[
            constant("1")?,
            constant("0")?,
            constant("0")?,
            constant("1")?,
        ],
        &[constant("0")?, constant("0")?],
        &mut ledger()?,
    )?;
    case.cas.prove_reading_equality(
        &case.context,
        composed.expression(),
        oracle.expression(),
        &mut ledger()?,
    )?;
    Ok(())
}

/// Point origins are retained in center maps while the positive reference alone scales differences.
/// ´test:integration:coordinate-images-retain-point-origin´
#[test]
fn coordinate_images_retain_point_origin() -> TestResult {
    let mut case = Fixture::new(&[])?;
    let quantity = Descriptor::new(
        nrz_bindings::Kind::Named(
            "temperature".into(),
            Box::new(nrz_bindings::Dimension::integers([0, 0, 0, 0, 1, 0, 0])?),
        ),
        Measure::coherent(),
        Role::Point,
    )?;
    let origin_descriptor = Descriptor::new(
        quantity.kind().clone(),
        Measure::new(
            "origin",
            Scale::Exact(nrz_cas::Rational::one()),
            nrz_cas::Rational::parse("27315/100", &mut ledger()?)?,
        )?,
        Role::Point,
    )?;
    let reference = ReferenceDefinition::new(
        "temperature-reference",
        &quantity.difference(),
        Scale::Expression(constant("10")?),
    )?;
    let origin = OriginDefinition::new("freezing", &origin_descriptor)?;
    let entry = EntryMeaning::new(quantity, reference, Some(origin), &mut ledger()?)?;
    let coordinates =
        CoordinateNormalization::new(&mut case.cas, &case.context, &[entry], &mut ledger()?)?;
    let normalize = coordinates.image(
        &mut case.cas,
        CoordinateDirection::Normalize,
        &mut ledger()?,
    )?;
    check(
        &normalize.export(&mut ledger()?)?,
        &["1/10"],
        &["-5463/200"],
    )?;
    let restore = coordinates.image(&mut case.cas, CoordinateDirection::Restore, &mut ledger()?)?;
    check(&restore.export(&mut ledger()?)?, &["10"], &["5463/20"])?;
    let original = coordinates.entries().first().ok_or("point")?;
    let changed = EntryMeaning::new(
        original.descriptor().clone(),
        original.reference().clone(),
        Some(OriginDefinition::new("freezing", &origin_descriptor)?),
        &mut ledger()?,
    )?;
    assert!(matches!(
        coordinates.check_entries(&[changed], &mut ledger()?),
        Err(nrz_bindings::readings::ReadingError::Origin)
    ));
    let difference = EntryMeaning::new(
        original.descriptor().difference(),
        original.reference().clone(),
        None,
        &mut ledger()?,
    )?;
    assert!(matches!(
        coordinates.check_entries(&[difference], &mut ledger()?),
        Err(nrz_bindings::readings::ReadingError::Role)
    ));
    Ok(())
}

/// Exact angle scales cancel before export, and remaining pi coefficients refuse by name.
/// ´test:integration:coordinate-degree-turn-cancellation-stays-exact´
#[test]
fn coordinate_degree_turn_cancellation_stays_exact() -> TestResult {
    let mut case = Fixture::new(&[])?;
    let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    let mut frames = Vec::new();
    for name in ["degree", "turn"] {
        let quantity = registry.measure(name).ok_or("angle measure")?;
        let reference = ReferenceDefinition::unit(name, quantity)?;
        let entry = EntryMeaning::new(quantity.coherent(), reference, None, &mut ledger()?)?;
        frames.push(CoordinateNormalization::new(
            &mut case.cas,
            &case.context,
            &[entry],
            &mut ledger()?,
        )?);
    }
    let source = frames.first().ok_or("degree")?;
    let target = frames.get(1).ok_or("turn")?;
    let conversion = source.conversion_to(&mut case.cas, target, &mut ledger()?)?;
    check(&conversion.export(&mut ledger()?)?, &["1/360"], &["0"])?;
    let restoration = source.image(&mut case.cas, CoordinateDirection::Restore, &mut ledger()?)?;
    assert!(matches!(
        restoration.export(&mut ledger()?),
        Err(nrz_bindings::readings::ReadingError::Units(
            nrz_bindings::UnitError::UnsupportedExactConstantExport
        ))
    ));
    assert!(restoration.scales()[0].as_rational().is_none());
    Ok(())
}

/// Portable mathematical keys do not replace independent physical reference identities.
/// ´test:integration:coordinate-reuse-keeps-physical-definition-identity´
#[test]
fn coordinate_reuse_keeps_physical_definition_identity() -> TestResult {
    let (mut first, original) = frame()?;
    let (mut second, independent) = frame()?;
    assert_eq!(
        original.reuse_key(&mut first.cas, &mut ledger()?)?,
        independent.reuse_key(&mut second.cas, &mut ledger()?)?
    );
    assert!(matches!(
        original.check_entries(independent.entries(), &mut ledger()?),
        Err(nrz_bindings::readings::ReadingError::Reference)
    ));
    original.check_entries(original.entries(), &mut ledger()?)?;
    let mut changed = entries()?;
    changed.reverse();
    assert!(original.check_entries(&changed, &mut ledger()?).is_err());
    Ok(())
}

/// The zero-coordinate identity keeps empty maps without traversing a nonexistent physical axis.
/// ´test:integration:coordinate-empty-frame-is-exact´
#[test]
fn coordinate_empty_frame_is_exact() -> TestResult {
    let mut case = Fixture::new(&[])?;
    let coordinates =
        CoordinateNormalization::new(&mut case.cas, &case.context, &[], &mut ledger()?)?;
    let image = coordinates.image(
        &mut case.cas,
        CoordinateDirection::Normalize,
        &mut ledger()?,
    )?;
    let map = image.export(&mut ledger()?)?;
    assert_eq!(map.inputs(), 0);
    assert_eq!(map.outputs(), 0);
    assert_eq!(map.coefficients(), [] as [nrz_cas::Rational; 0]);
    assert_eq!(map.offsets(), [] as [nrz_cas::Rational; 0]);
    Ok(())
}
