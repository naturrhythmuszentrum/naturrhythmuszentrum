// SPDX-License-Identifier: AGPL-3.0-only
//! Schema exports preserve physical definition identity and typed quotient coordinates.

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, contract, execute, measure, owner, shape};
use nrz_bindings::{BaseDimension, Descriptor, Dimension, Kind, LimitsOverride, Role};
use nrz_matrix::{
    Axis, ConversionContract, ConversionError, CoordinateMap, Extent, Heap, HeapLimits,
    NormalizationSchema, OperationError, PointOrigin, ReferenceScale, ShapeLimits,
};

/// Make independent exact ratio references; equal descriptions do not create identity.
fn reference(scale: u32) -> TestResult<ReferenceScale> {
    Ok(ReferenceScale::new(
        "scale",
        &Descriptor::new(Kind::Ratio, measure(scale, 1, 0, 1)?, Role::Difference)?,
    )?)
}

/// One point equation with a formal solution and independently named X/Y/W definitions.
fn schema(
    declarations: &std::rc::Rc<nrz_bindings::CheckedDeclarations>,
    domain: ReferenceScale,
    columns: ReferenceScale,
) -> TestResult<NormalizationSchema> {
    let length = Descriptor::new(
        Kind::Named(
            "length".into(),
            Box::new(Dimension::base(BaseDimension::Length)),
        ),
        measure(1, 1, 0, 1)?,
        Role::Difference,
    )?;
    let point = Descriptor::new(length.kind().clone(), measure(1, 1, 3, 1)?, Role::Point)?;
    let operator = shape(declarations, &length, 1)?;
    let limits = ShapeLimits::default();
    let entry = operator
        .codomain()
        .entries()
        .first()
        .ok_or("row")?
        .with_origin(point.clone())?;
    let points = Axis::new(vec![entry], operator.codomain().partition().clone(), limits)?;
    let row = ReferenceScale::new(
        "row",
        &Descriptor::new(
            length.kind().clone(),
            measure(10, 1, 0, 1)?,
            Role::Difference,
        )?,
    )?;
    Ok(NormalizationSchema::check(
        &operator,
        &operator,
        CoordinateMap::check(operator.domain(), vec![domain], vec![None], limits)?,
        CoordinateMap::check(
            &points,
            vec![row],
            vec![Some(PointOrigin::new("origin", &point)?)],
            limits,
        )?,
        CoordinateMap::check(operator.domain(), vec![columns], vec![None], limits)?,
        limits,
    )?)
}

/// Scalar schema maps subtract origins and publish a checked round-trip with outward companions.
/// ´test:integration:scalar-schema-map-roundtrip-encloses-rational-oracle´
#[test]
fn scalar_schema_map_roundtrip_encloses_rational_oracle() -> TestResult {
    let declarations = owner()?;
    let configuration = schema(&declarations, reference(2)?, reference(5)?)?;
    let maps =
        ConversionContract::from_normalization_schema(&configuration, &LimitsOverride::default())?;
    let map = maps.codomain().first().ok_or("scalar row map")?;
    let source = map.id().source().descriptor().ok_or("scalar descriptor")?;
    let physical = shape(&declarations, source, 1)?;
    let normalized = shape(&declarations, &Descriptor::ratio(), 1)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let original = heap.import_declared_bounds(Extent::new(1, 1)?, source, &[23.0_f64], &[1.0])?;
    let result = execute(
        &mut heap,
        contract(physical.clone(), normalized.clone(), map.clone())?,
        original.value(),
    )?;
    let center = heap.value(result.value())?.region().get(0, 0)?;
    let error = heap
        .companion(result.evaluation().ok_or("evaluation")?)?
        .region()
        .get(0, 0)?;
    assert!(covers(center, error, 2, 1)?);
    let total = heap
        .companion(result.bound().ok_or("total")?)?
        .region()
        .get(0, 0)?;
    assert!(covers(center, total, 19, 10)?);
    assert!(covers(center, total, 21, 10)?);
    let restored = execute(
        &mut heap,
        contract(normalized, physical, map.inverse()?)?,
        result.value(),
    )?;
    let center = heap.value(restored.value())?.region().get(0, 0)?;
    let total = heap
        .companion(restored.bound().ok_or("restored total")?)?
        .region()
        .get(0, 0)?;
    assert!(covers(center, total, 22, 1)?);
    assert!(covers(center, total, 24, 1)?);
    assert_eq!(map.inverse()?.inverse()?, *map);
    Ok(())
}

/// Formal maps retain both reference sides, exact inverses and separate center/difference shifts.
/// ´test:integration:quotient-schema-map-roundtrip-preserves-formal-coordinates´
#[test]
fn quotient_schema_map_roundtrip_preserves_formal_coordinates() -> TestResult {
    let declarations = owner()?;
    let configuration = schema(&declarations, reference(2)?, reference(5)?)?;
    let limits = LimitsOverride::default();
    let maps = ConversionContract::from_normalization_schema(&configuration, &limits)?;
    for (entries, expected) in [
        (maps.operator(), "1/5"),
        (maps.rhs(), "1/2"),
        (maps.solution(), "5/2"),
    ] {
        let map = entries.first().ok_or("formal entry")?.centers();
        assert!(map.id().source().descriptor().is_none());
        assert!(map.id().source().quotient().is_some());
        assert_eq!(map.scale().export(&limits)?, expected);
        assert_eq!(map.inverse()?.id().target(), map.id().source());
        assert_eq!(map.inverse()?.inverse()?, *map);
        map.validate_snapshot(configuration.operator().declarations())?;
    }
    let rhs = maps.rhs().first().ok_or("rhs")?;
    assert_eq!(rhs.centers().shift().export(&limits)?, "-3/2");
    assert_eq!(rhs.centers().inverse()?.shift().export(&limits)?, "3");
    assert_eq!(rhs.differences().shift().export(&limits)?, "0");
    assert_eq!(maps.schema().origin_corrections().len(), 1);
    assert!(matches!(
        ConversionContract::new(
            configuration.operator().clone(),
            configuration.operator().clone(),
            rhs.centers().clone(),
            nrz_scalar::LiteralLimits::default(),
            nrz_matrix::ConversionMode::Deterministic
        ),
        Err(OperationError::UnsupportedShape)
    ));
    Ok(())
}

/// Every use of an H definition shares its exported identity, while newly declared twins differ.
/// ´test:integration:schema-maps-reuse-definition-identities´
#[test]
fn schema_maps_reuse_definition_identities() -> TestResult {
    let declarations = owner()?;
    let retained = reference(2)?;
    let limits = LimitsOverride::default();
    let shared = ConversionContract::from_normalization_schema(
        &schema(&declarations, retained.clone(), retained.clone())?,
        &limits,
    )?;
    let variable = shared
        .domain()
        .first()
        .ok_or("X")?
        .id()
        .reference_definition();
    assert_eq!(
        variable,
        shared
            .columns()
            .first()
            .ok_or("W")?
            .id()
            .reference_definition()
    );
    let solution = shared.solution().first().ok_or("solution")?.centers();
    assert_eq!(variable, solution.id().reference_definition());
    assert_eq!(variable, solution.id().column_reference());
    assert_eq!(
        shared
            .codomain()
            .first()
            .ok_or("Y")?
            .id()
            .origin_definition(),
        shared
            .rhs()
            .first()
            .ok_or("rhs")?
            .centers()
            .id()
            .origin_definition()
    );
    let distinct = ConversionContract::from_normalization_schema(
        &schema(&declarations, retained, reference(2)?)?,
        &limits,
    )?;
    assert_ne!(
        distinct
            .domain()
            .first()
            .ok_or("X")?
            .id()
            .reference_definition(),
        distinct
            .columns()
            .first()
            .ok_or("W")?
            .id()
            .reference_definition()
    );
    assert_eq!(
        distinct
            .domain()
            .first()
            .ok_or("X")?
            .scale()
            .export(&limits)?,
        distinct
            .columns()
            .first()
            .ok_or("W")?
            .scale()
            .export(&limits)?
    );
    Ok(())
}

/// A failed schema export preserves the exact bindings cause rather than an unavailable placeholder.
/// ´test:integration:schema-map-refusal-retains-units-cause´
#[test]
fn schema_map_refusal_retains_units_cause() -> TestResult {
    let declarations = owner()?;
    let configuration = schema(&declarations, reference(2)?, reference(5)?)?;
    let limits = LimitsOverride {
        work: Some(1),
        ..LimitsOverride::default()
    };
    let good =
        ConversionContract::from_normalization_schema(&configuration, &LimitsOverride::default())?;
    let first = good.domain().first().ok_or("X")?;
    let expected = configuration
        .operator()
        .declarations()
        .schema_normalization_map(
            first.id().source().descriptor().ok_or("scalar")?,
            first.id().reference_definition().ok_or("reference")?,
            None,
            &limits,
        )
        .err()
        .ok_or("units budget accepted")?;
    let refused = ConversionContract::from_normalization_schema(&configuration, &limits)
        .err()
        .ok_or("matrix budget accepted")?;
    let OperationError::Conversion(ConversionError::Map(cause)) = refused else {
        return Err("units cause lost".into());
    };
    assert!(matches!(cause, nrz_bindings::MapError::Algebra(_)));
    assert_eq!(cause, expected);
    Ok(())
}
