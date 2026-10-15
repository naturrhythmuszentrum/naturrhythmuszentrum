// SPDX-License-Identifier: AGPL-3.0-only
//! Exact origin/reference schema acceptance and refusal pairs.

use crate::shape_testing::{ResultOf, axis, quantity, ratio_shape, ratios};
use nrz_matrix::{
    Axis, CoordinateMap, NormalizationSchema, PointOrigin, ReferenceScale, ShapeError, ShapeLimits,
};
use nrz_units::{Conversion, Descriptor, Kind, Measure, Role, Scale, UnitError};

/// Small exact descriptor constants, without importing or evaluating any heap number.
fn measure(name: &str, scale: u32, origin: u32) -> ResultOf<Measure> {
    let coherent = Measure::coherent();
    let increment = Conversion {
        scale: coherent.scale().clone(),
        shift: coherent.scale().clone(),
    };
    let literal = |count| {
        let mut value = coherent.offset().clone();
        for _ in 0..count {
            value = increment.apply(&value)?;
        }
        Ok::<_, UnitError>(value)
    };
    Ok(Measure::new(
        name,
        Scale::Exact(literal(scale)?),
        literal(origin)?,
    )?)
}

/// One unit of the chosen measure defines an exact source-free reference.
fn reference(name: &str, quantity: &Descriptor, scale: u32) -> ResultOf<ReferenceScale> {
    Ok(ReferenceScale::new(
        name,
        &Descriptor::new(
            quantity.kind().clone(),
            measure(name, scale, 0)?,
            Role::Difference,
        )?,
    )?)
}

/// A difference-axis map, with deliberately explicit reference identities.
fn differences(axis: &Axis, scale: u32) -> ResultOf<CoordinateMap> {
    let references = axis
        .entries()
        .iter()
        .map(|entry| reference(entry.name(), entry.quantity().ok_or("scalar axis")?, scale))
        .collect::<ResultOf<_>>()?;
    Ok(CoordinateMap::check(
        axis,
        references,
        vec![None; axis.entries().len()],
        ShapeLimits::default(),
    )?)
}

/// Exact matrix maps use both row and column references, with reciprocal restoration.
/// ´test:integration:normalization-retains-two-sided-exact-maps´
#[test]
fn normalization_retains_two_sided_exact_maps() -> ResultOf<()> {
    let snapshot = ratios()?;
    let domain = axis(&["x"], &Descriptor::ratio())?;
    let codomain = axis(&["y"], &Descriptor::ratio())?;
    let columns = axis(&["w0", "w1"], &Descriptor::ratio())?;
    let operator = ratio_shape(&snapshot, &domain, &codomain)?;
    let rhs = ratio_shape(&snapshot, &columns, &codomain)?;
    let schema = NormalizationSchema::check(
        &operator,
        &rhs,
        differences(&domain, 2)?,
        differences(&codomain, 3)?,
        differences(&columns, 5)?,
        ShapeLimits::default(),
    )?;
    let operator_map = schema.operator_maps().first().ok_or("operator map")?;
    assert_eq!(
        operator_map.normalization().scale,
        measure("numerator", 2, 0)?
            .conversion_to(&measure("denominator", 3, 0)?, Role::Difference)?
            .scale
    );
    assert_eq!(
        operator_map.restoration().scale,
        measure("numerator", 3, 0)?
            .conversion_to(&measure("denominator", 2, 0)?, Role::Difference)?
            .scale
    );
    assert_eq!(
        schema
            .rhs_maps()
            .first()
            .ok_or("rhs map")?
            .normalization()
            .scale,
        measure("numerator", 5, 0)?
            .conversion_to(&measure("denominator", 3, 0)?, Role::Difference)?
            .scale
    );
    assert_eq!(
        schema
            .solution_maps()
            .first()
            .ok_or("solution map")?
            .restoration()
            .scale,
        measure("numerator", 2, 0)?
            .conversion_to(&measure("denominator", 5, 0)?, Role::Difference)?
            .scale
    );
    assert_eq!(schema.origin_corrections(), []);
    assert_eq!(schema.alignments().len(), 3);
    Ok(())
}

/// Reference identity survives clones and stays distinct from equal spelling and value.
/// ´test:integration:reference-identity-and-role-refusals´
#[test]
fn reference_identity_and_role_refusals() -> ResultOf<()> {
    let temperature = quantity("temperature", [0, 0, 0, 0, 1, 0, 0])?;
    let first = reference("reference", &temperature, 1)?;
    let second = reference("reference", &temperature, 1)?;
    assert_eq!(first, first.clone());
    assert_ne!(first, second);
    let point = Descriptor::new(
        temperature.kind().clone(),
        measure("origin", 1, 7)?,
        Role::Point,
    )?;
    assert!(matches!(
        ReferenceScale::new("point", &point),
        Err(ShapeError::Reference)
    ));
    assert!(PointOrigin::new("difference", &temperature).is_err());
    assert!(measure("zero", 0, 0).is_err());
    let axis = axis(&["temperature"], &temperature)?;
    assert!(
        CoordinateMap::check(
            &axis,
            vec![reference("wrong", &Descriptor::ratio(), 1)?],
            vec![None],
            ShapeLimits::default()
        )
        .is_err()
    );
    assert!(
        CoordinateMap::check(
            &axis,
            vec![first],
            vec![Some(PointOrigin::new("extra", &point)?)],
            ShapeLimits::default()
        )
        .is_err()
    );
    Ok(())
}

/// Point origins subtract first, equation shifts remain explicit, and bounds receive no origin.
/// ´test:integration:point-normalization-retains-equation-shift´
#[test]
fn point_normalization_retains_equation_shift() -> ResultOf<()> {
    let snapshot = ratios()?;
    let value = quantity("temperature", [0, 0, 0, 0, 1, 0, 0])?;
    let domain = axis(&["x"], &value)?;
    let codomain = axis(&["y"], &value)?;
    let column = axis(&["w"], &Descriptor::ratio())?;
    let operator = crate::shape_testing::dense(
        &snapshot,
        &domain,
        &codomain,
        vec![nrz_matrix::EntryDraft::Declared(Descriptor::ratio())],
    )?;
    let rhs = crate::shape_testing::dense(
        &snapshot,
        &column,
        &codomain,
        vec![nrz_matrix::EntryDraft::Declared(value.clone())],
    )?;
    let x = point_map(&domain, &value, 2, 7)?;
    let y = point_map(&codomain, &value, 3, 11)?;
    let schema = NormalizationSchema::check(
        &operator,
        &rhs,
        x,
        y,
        differences(&column, 1)?,
        ShapeLimits::default(),
    )?;
    let solution = schema.solution_maps().first().ok_or("solution map")?;
    assert_eq!(
        solution.normalization().shift,
        Measure::coherent()
            .conversion_to(&measure("expected", 2, 7)?, Role::Point)?
            .shift
    );
    assert_eq!(
        solution.restoration().shift,
        measure("expected", 1, 7)?.offset().clone()
    );
    assert!(solution.difference_restoration().shift.is_zero());
    assert!(
        schema
            .operator_maps()
            .first()
            .ok_or("operator map")?
            .normalization()
            .shift
            .is_zero()
    );
    assert_eq!(
        schema
            .rhs_maps()
            .first()
            .ok_or("rhs map")?
            .normalization()
            .shift,
        Measure::coherent()
            .conversion_to(&measure("expected", 3, 11)?, Role::Point)?
            .shift
    );
    assert_eq!(schema.origin_corrections().len(), 2);
    assert!(matches!(
        schema.origin_corrections().first(),
        Some(nrz_matrix::OriginCorrection::RightOrigin { row: 0, .. })
    ));
    Ok(())
}

/// Explicit origin declarations preserve the original coordinate identity.
fn point_map(
    axis: &Axis,
    quantity: &Descriptor,
    scale: u32,
    origin: u32,
) -> ResultOf<CoordinateMap> {
    let point = Descriptor::new(
        quantity.kind().clone(),
        measure("point", 1, origin)?,
        Role::Point,
    )?;
    let entries = axis
        .entries()
        .iter()
        .map(|entry| entry.with_origin(point.clone()))
        .collect::<Result<_, _>>()?;
    let points = Axis::new(entries, axis.partition().clone(), ShapeLimits::default())?;
    Ok(CoordinateMap::check(
        &points,
        vec![reference("scale", quantity, scale)?],
        vec![Some(PointOrigin::new("origin", &point)?)],
        ShapeLimits::default(),
    )?)
}

/// Scalar exact references cannot conceal unsupported affine matrix-column origins.
/// ´test:integration:affine-column-and-reference-limits-refuse´
#[test]
fn affine_column_and_reference_limits_refuse() -> ResultOf<()> {
    let snapshot = ratios()?;
    let value = Descriptor::new(
        Kind::Named("coordinate".into(), Box::new(nrz_units::Dimension::zero())),
        Measure::coherent(),
        Role::Difference,
    )?;
    let domain = axis(&["x"], &value)?;
    let columns = axis(&["w0", "w1"], &Descriptor::ratio())?;
    let operator = crate::shape_testing::dense(
        &snapshot,
        &domain,
        &domain,
        vec![nrz_matrix::EntryDraft::Declared(Descriptor::ratio())],
    )?;
    let rhs = crate::shape_testing::dense(
        &snapshot,
        &columns,
        &domain,
        vec![nrz_matrix::EntryDraft::Declared(value.clone()); 2],
    )?;
    let schema = NormalizationSchema::check(
        &operator,
        &rhs,
        point_map(&domain, &value, 1, 1)?,
        differences(&domain, 1)?,
        differences(&columns, 1)?,
        ShapeLimits::default(),
    )?;
    assert_eq!(schema.solution_maps().len(), 2);
    assert_eq!(
        schema
            .solution_maps()
            .first()
            .ok_or("first column")?
            .restoration(),
        schema
            .solution_maps()
            .get(1)
            .ok_or("second column")?
            .restoration()
    );
    let named_columns = axis(&["w0", "w1"], &value)?;
    let named_rhs = crate::shape_testing::dense(
        &snapshot,
        &named_columns,
        &domain,
        vec![nrz_matrix::EntryDraft::Declared(Descriptor::ratio()); 2],
    )?;
    assert!(matches!(
        NormalizationSchema::check(
            &operator,
            &named_rhs,
            point_map(&domain, &value, 1, 1)?,
            differences(&domain, 1)?,
            differences(&named_columns, 1)?,
            ShapeLimits::default()
        ),
        Err(ShapeError::Unsupported(_))
    ));
    assert!(matches!(
        CoordinateMap::check(
            &domain,
            vec![reference("s", &value, 1)?],
            vec![None],
            ShapeLimits::new(1, 1, 1, 0)
        ),
        Err(ShapeError::Limit)
    ));
    Ok(())
}
