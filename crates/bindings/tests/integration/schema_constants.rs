// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen named maps and matrix-shaped schema exports share one bindings evidence boundary.

use nrz_bindings::{Descriptor, Kind, OriginDefinition, PairingWitness, ReferenceDefinition, Role};
use nrz_cas::limits::{Budget, Limits, LimitsOverride};

use crate::testing::constants::{constant, difference, point};
use crate::testing::declarations::{TestResult, context};

/// Named scalar maps round-trip identically through the new immutable definition surface.
/// ´test:integration:schema-named-maps-preserve-frozen-owner´
#[test]
fn schema_named_maps_preserve_frozen_owner() -> TestResult {
    let celsius = point(Kind::Angle, "27315/100")?;
    let reference_descriptor = difference(Kind::Angle, "5/9")?;
    let mut builder = context(&["reference", "origin"])?;
    constant(&mut builder, "reference", &reference_descriptor, "18")?;
    constant(&mut builder, "origin", &celsius, "20")?;
    let owner = builder.freeze()?;
    let identity = owner.id().clone();
    let before = owner
        .cas()?
        .canonical_snapshot(&mut Budget::new(Limits::default())?)?;
    let reference = owner.reference_definition("reference")?;
    let origin = owner.origin_definition("origin")?;
    let limits = LimitsOverride::default();
    for (descriptor, definition, key) in [
        (celsius.clone(), Some(&origin), Some("origin")),
        (celsius.difference(), None, None),
    ] {
        let named = owner.normalization_map(&descriptor, "reference", key)?;
        let schema =
            owner.schema_normalization_map(&descriptor, &reference, definition, &limits)?;
        assert_eq!(schema, named);
        assert_eq!(schema.inverse()?, named.inverse()?);
        assert_eq!(schema.inverse()?.inverse()?, named);
        assert_eq!(schema.scale().export(&limits)?, "1/10");
        assert_eq!(
            schema.shift().export(&limits)?,
            if key.is_some() { "-2" } else { "0" }
        );
    }
    assert_eq!(owner.id(), &identity);
    assert_eq!(owner.declarations().len(), 2);
    assert_eq!(
        owner
            .cas()?
            .canonical_snapshot(&mut Budget::new(Limits::default())?)?,
        before
    );
    Ok(())
}

/// A matrix client exports coherent point and difference maps and retains correction origins.
/// ´test:integration:schema-point-entry-and-origin-correction-lifecycle´
#[test]
fn schema_point_entry_and_origin_correction_lifecycle() -> TestResult {
    let owner = context(&[])?.freeze()?;
    let captured = owner.id().clone();
    let row = ReferenceDefinition::unit("temperature_ref", &difference(Kind::Angle, "10")?)?;
    let column = ReferenceDefinition::unit("rhs_ref", &difference(Kind::Ratio, "2")?)?;
    let point_descriptor = point(Kind::Angle, "5863/20")?;
    let origin = OriginDefinition::new("operating_origin", &point_descriptor)?;
    let quotient = PairingWitness::sensitivity(row.descriptor(), column.descriptor())?;
    let limits = LimitsOverride::default();
    let centers =
        owner.quotient_normalization_map(&quotient, &row, &column, Some(&origin), &limits)?;
    let differences = owner.quotient_normalization_map(&quotient, &row, &column, None, &limits)?;
    assert_eq!(centers.scale().export(&limits)?, "1/5");
    assert_eq!(centers.shift().export(&limits)?, "-5863/100");
    assert_eq!(centers.id().source().role(), Role::Point);
    assert_eq!(differences.shift().export(&limits)?, "0");
    assert_eq!(differences.inverse()?.shift().export(&limits)?, "0");
    let restoration = centers.inverse()?;
    assert_eq!(restoration.scale().export(&limits)?, "5");
    let correction_origin = restoration.shift().clone();
    assert_eq!(correction_origin.export(&limits)?, "5863/20");
    assert_eq!(
        correction_origin.map_id().origin_definition(),
        Some(&origin)
    );
    assert_eq!(restoration.inverse()?, centers);
    assert_eq!(owner.id(), &captured);
    assert_eq!(owner.declarations().len(), 0);
    centers.validate_snapshot(&owner)?;
    Ok(())
}

/// Shared reference objects preserve identity across exports while distinct column definitions do not.
/// ´test:integration:schema-row-column-definition-lifecycle´
#[test]
fn schema_row_column_definition_lifecycle() -> TestResult {
    let owner = context(&[])?.freeze()?;
    let row = ReferenceDefinition::unit("row", &difference(Kind::Angle, "3")?)?;
    let first = ReferenceDefinition::unit("column", &Descriptor::ratio())?;
    let second = ReferenceDefinition::unit("column", &Descriptor::ratio())?;
    let quotient = PairingWitness::sensitivity(row.descriptor(), first.descriptor())?;
    let limits = LimitsOverride::default();
    let map = owner.quotient_normalization_map(&quotient, &row, &first, None, &limits)?;
    let repeated = owner.quotient_normalization_map(&quotient, &row, &first, None, &limits)?;
    let other = owner.quotient_normalization_map(&quotient, &row, &second, None, &limits)?;
    assert_eq!(map, repeated);
    assert_ne!(map.id(), other.id());
    assert_eq!(map.scale().export(&limits)?, other.scale().export(&limits)?);
    assert_eq!(map.id().column_reference(), Some(&first));
    assert_eq!(other.id().column_reference(), Some(&second));
    let scale = map.scale().clone();
    let identity = map.id().clone();
    drop(map);
    assert_eq!(scale.map_id(), &identity);
    assert_eq!(scale.export(&limits)?, "1/3");
    Ok(())
}
