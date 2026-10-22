// SPDX-License-Identifier: AGPL-3.0-only
//! Schema definitions preserve ordered quotient meaning, identity and explicit refusal boundaries.

use nrz_bindings::{
    CountDomain, Descriptor, Kind, MapError, MapKind, Measure, OriginDefinition, PairingError,
    PairingWitness, ReferenceDefinition, Role, Scale, SnapshotError, UncertaintyMode, UnitError,
};
use nrz_cas::limits::{BudgetKind, LimitsOverride};

use crate::testing::constants::{constant, difference, point, rational};
use crate::testing::declarations::{TestResult, context, input, length};

/// Heterogeneous references export their ordered quotient without a fabricated scalar kind.
/// ´test:integration:schema-quotient-constants-and-inverse´
#[test]
fn schema_quotient_constants_and_inverse() -> TestResult {
    let owner = context(&[])?.freeze()?;
    let distance = difference(length()?.kind().clone(), "2/3")?;
    let angle = difference(Kind::Angle, "5/7")?;
    let row = ReferenceDefinition::unit("length_ref", &distance)?;
    let column = ReferenceDefinition::unit("angle_ref", &angle)?;
    let quotient = PairingWitness::sensitivity(&distance, &angle)?;
    let limits = LimitsOverride::default();
    let map = owner.quotient_normalization_map(&quotient, &row, &column, None, &limits)?;
    assert_eq!(map.scale().export(&limits)?, "15/14");
    assert_eq!(map.shift().export(&limits)?, "0");
    assert_eq!(map.id().source().descriptor(), None);
    assert_eq!(map.id().source().quotient(), Some(&quotient));
    assert_eq!(map.id().source().role(), Role::Difference);
    assert_eq!(map.id().target().descriptor(), Some(&Descriptor::ratio()));
    assert_eq!(map.id().reference_definition(), Some(&row));
    assert_eq!(map.id().column_reference(), Some(&column));
    assert_eq!(map.scale().map_id(), map.id());
    assert_eq!(map.shift().map_id(), map.id());
    let inverse = map.inverse()?;
    assert_eq!(inverse.id().kind(), MapKind::Restoration);
    assert_eq!(inverse.scale().export(&limits)?, "14/15");
    assert_eq!(inverse.shift().export(&limits)?, "0");
    assert_eq!(inverse.id().target(), map.id().source());
    assert_eq!(inverse.inverse()?, map);
    map.validate_snapshot(&owner)?;
    Ok(())
}

/// Named reference coordinates participate in the quotient as well as their measure scales.
/// ´test:integration:schema-quotient-uses-exact-reference-coordinates´
#[test]
fn schema_quotient_uses_exact_reference_coordinates() -> TestResult {
    let centimetres = difference(length()?.kind().clone(), "1/100")?;
    let percent = difference(Kind::Ratio, "1/100")?;
    let mut builder = context(&["row", "column"])?;
    constant(&mut builder, "row", &centimetres, "200")?;
    constant(&mut builder, "column", &percent, "50")?;
    let owner = builder.freeze()?;
    let quotient = PairingWitness::sensitivity(&centimetres, &percent)?;
    let row = owner.reference_definition("row")?;
    let column = owner.reference_definition("column")?;
    let map = owner.quotient_normalization_map(
        &quotient,
        &row,
        &column,
        None,
        &LimitsOverride::default(),
    )?;
    assert_eq!(map.scale().export(&LimitsOverride::default())?, "1/4");
    assert_eq!(
        map.inverse()?.scale().export(&LimitsOverride::default())?,
        "4"
    );
    assert_eq!(map.inverse()?.inverse()?, map);
    Ok(())
}

/// Source-free schemas retain definition identity even at equal labels and exact coordinates.
/// ´test:integration:schema-definition-identities-do-not-collapse´
#[test]
fn schema_definition_identities_do_not_collapse() -> TestResult {
    let owner = context(&[])?.freeze()?;
    let origin_descriptor = point(Kind::Angle, "2")?;
    let row = ReferenceDefinition::unit("reference", &origin_descriptor.difference())?;
    let another = ReferenceDefinition::unit("reference", &origin_descriptor.difference())?;
    let origin = OriginDefinition::new("origin", &origin_descriptor)?;
    let other_origin = OriginDefinition::new("origin", &origin_descriptor)?;
    let limits = LimitsOverride::default();
    let map = owner.schema_normalization_map(&origin_descriptor, &row, Some(&origin), &limits)?;
    let repeated =
        owner.schema_normalization_map(&origin_descriptor, &row, Some(&origin), &limits)?;
    assert_eq!(map, repeated);
    for (reference, selected_origin) in [(&another, &origin), (&row, &other_origin)] {
        let changed = owner.schema_normalization_map(
            &origin_descriptor,
            reference,
            Some(selected_origin),
            &limits,
        )?;
        assert_ne!(map.id(), changed.id());
        assert_eq!(
            map.scale().export(&limits)?,
            changed.scale().export(&limits)?
        );
        assert_eq!(
            map.shift().export(&limits)?,
            changed.shift().export(&limits)?
        );
    }
    Ok(())
}

/// Symbolic or nonpositive definitions cannot acquire checked reference authority.
/// ´test:integration:schema-reference-exactness-and-positivity-refusals´
#[test]
fn schema_reference_exactness_and_positivity_refusals() -> TestResult {
    let descriptor = length()?;
    assert!(matches!(
        ReferenceDefinition::new("symbolic", &descriptor, Scale::Symbolic("pi".into())),
        Err(MapError::Units(UnitError::UnsupportedMeasureScale(_))),
    ));
    for text in ["0", "-1", "-1/10"] {
        assert_eq!(
            ReferenceDefinition::new("reference", &descriptor, Scale::Exact(rational(text)?)),
            Err(MapError::Reference),
        );
    }
    assert!(ReferenceDefinition::unit(" ", &descriptor).is_err());
    assert_eq!(
        ReferenceDefinition::unit("point", &point(Kind::Angle, "0")?),
        Err(MapError::Reference)
    );
    let count = Descriptor::new(
        Kind::Count("items".into(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    assert_eq!(
        ReferenceDefinition::unit("count", &count),
        Err(MapError::Units(UnitError::CountDomain))
    );
    Ok(())
}

/// Runtime claims and wrong affine roles do not become exact reference or origin declarations.
/// ´test:integration:schema-named-definition-refusals´
#[test]
fn schema_named_definition_refusals() -> TestResult {
    let descriptor = length()?;
    let mut builder = context(&["runtime", "zero", "difference"])?;
    input(&mut builder, "runtime", &descriptor)?;
    constant(&mut builder, "zero", &descriptor, "0")?;
    constant(&mut builder, "difference", &descriptor, "1")?;
    let owner = builder.freeze()?;
    for key in ["runtime", "missing"] {
        assert!(matches!(
            owner.reference_definition(key),
            Err(MapError::ConstantDeclaration(_))
        ));
        assert!(matches!(
            owner.origin_definition(key),
            Err(MapError::ConstantDeclaration(_))
        ));
    }
    assert_eq!(owner.reference_definition("zero"), Err(MapError::Reference));
    assert_eq!(owner.origin_definition("difference"), Err(MapError::Origin));
    assert!(OriginDefinition::new("origin", &descriptor).is_err());
    assert!(OriginDefinition::new(" ", &point(Kind::Angle, "0")?).is_err());
    Ok(())
}

/// A formal quotient checks both ordered targets, including semantic kinds and measures.
/// ´test:integration:schema-quotient-target-refusals´
#[test]
fn schema_quotient_target_refusals() -> TestResult {
    let owner = context(&[])?.freeze()?;
    let distance = length()?;
    let row = ReferenceDefinition::unit("row", &distance)?;
    let column = ReferenceDefinition::unit("column", &Descriptor::ratio())?;
    let quotient = PairingWitness::sensitivity(&distance, &Descriptor::ratio())?;
    let limits = LimitsOverride::default();
    assert!(matches!(
        owner.quotient_normalization_map(&quotient, &column, &row, None, &limits),
        Err(MapError::Pairing(PairingError::TargetMismatch { .. }))
    ));
    let scaled = ReferenceDefinition::unit("scaled", &difference(distance.kind().clone(), "2")?)?;
    assert!(matches!(
        owner.quotient_normalization_map(&quotient, &scaled, &column, None, &limits),
        Err(MapError::Pairing(PairingError::TargetMismatch { .. }))
    ));
    let radius = PairingWitness::uncertainty(UncertaintyMode::Radius, &distance, None, None)?;
    assert_eq!(
        owner.quotient_normalization_map(&radius, &row, &column, None, &limits),
        Err(MapError::Pairing(PairingError::ExpectedSensitivity))
    );
    Ok(())
}

/// Point origins require their row kind and ratio columns; scalar differences never translate.
/// ´test:integration:schema-origin-role-and-kind-refusals´
#[test]
fn schema_origin_role_and_kind_refusals() -> TestResult {
    let owner = context(&[])?.freeze()?;
    let point_descriptor = point(Kind::Angle, "3")?;
    let row = ReferenceDefinition::unit("row", &point_descriptor.difference())?;
    let column = ReferenceDefinition::unit("column", &length()?)?;
    let origin = OriginDefinition::new("origin", &point_descriptor)?;
    let quotient = PairingWitness::sensitivity(row.descriptor(), column.descriptor())?;
    let limits = LimitsOverride::default();
    assert_eq!(
        owner.quotient_normalization_map(&quotient, &row, &column, Some(&origin), &limits),
        Err(MapError::Origin)
    );
    assert_eq!(
        owner.schema_normalization_map(&point_descriptor, &row, None, &limits),
        Err(MapError::Origin)
    );
    assert_eq!(
        owner.schema_normalization_map(row.descriptor(), &row, Some(&origin), &limits),
        Err(MapError::Origin)
    );
    let wrong = OriginDefinition::new(
        "wrong",
        &point(
            Kind::Named("phase".into(), Box::new(nrz_bindings::Dimension::zero())),
            "3",
        )?,
    )?;
    assert_eq!(
        owner.schema_normalization_map(&point_descriptor, &row, Some(&wrong), &limits),
        Err(MapError::Units(UnitError::KindMismatch))
    );
    Ok(())
}

/// Definitions resolved in another owner cannot silently rebind to identical local declarations.
/// ´test:integration:schema-foreign-declarations-refuse´
#[test]
fn schema_foreign_declarations_refuse() -> TestResult {
    let mut builder = context(&["reference", "origin"])?;
    let point_descriptor = point(Kind::Angle, "2")?;
    constant(
        &mut builder,
        "reference",
        &point_descriptor.difference(),
        "1",
    )?;
    constant(&mut builder, "origin", &point_descriptor, "0")?;
    let original = builder.freeze()?;
    let foreign = context(&[])?.freeze()?;
    let reference = original.reference_definition("reference")?;
    let origin = original.origin_definition("origin")?;
    let local = ReferenceDefinition::unit("local", reference.descriptor())?;
    let limits = LimitsOverride::default();
    for candidate in [&reference, &local] {
        let result =
            foreign.schema_normalization_map(&point_descriptor, candidate, Some(&origin), &limits);
        assert_eq!(
            result,
            Err(MapError::Snapshot(SnapshotError::ForeignContext))
        );
    }
    let map =
        original.schema_normalization_map(&point_descriptor, &reference, Some(&origin), &limits)?;
    assert_eq!(
        map.validate_snapshot(&foreign),
        Err(MapError::Snapshot(SnapshotError::ForeignContext))
    );
    Ok(())
}

/// Exact schema arithmetic has explicit work and rational component ceilings.
/// ´test:integration:schema-map-budget-refusals´
#[test]
fn schema_map_budget_refusals() -> TestResult {
    let owner = context(&[])?.freeze()?;
    let row = ReferenceDefinition::unit("row", &difference(Kind::Angle, "10")?)?;
    let column = ReferenceDefinition::unit("column", &Descriptor::ratio())?;
    let quotient = PairingWitness::sensitivity(row.descriptor(), column.descriptor())?;
    for (limits, expected) in [
        (
            LimitsOverride {
                work: Some(1),
                ..LimitsOverride::default()
            },
            BudgetKind::Work,
        ),
        (
            LimitsOverride {
                component_bits: Some(1),
                ..LimitsOverride::default()
            },
            BudgetKind::ComponentBits,
        ),
    ] {
        let error = owner
            .quotient_normalization_map(&quotient, &row, &column, None, &limits)
            .err()
            .ok_or("budget ignored")?;
        assert!(
            matches!(error, MapError::Algebra(nrz_cas::Fault { kind: nrz_cas::FaultKind::Exhausted { budget, .. }, .. }) if budget == expected)
        );
        assert!(
            owner
                .schema_normalization_map(row.descriptor(), &row, None, &limits)
                .is_err()
        );
    }
    let map = owner.quotient_normalization_map(
        &quotient,
        &row,
        &column,
        None,
        &LimitsOverride::default(),
    )?;
    assert_eq!(map.scale().export(&LimitsOverride::default())?, "1/10");
    Ok(())
}

/// Work exhaustion has a precise public success boundary and leaves the owner unchanged.
/// ´test:integration:schema-map-work-boundary´
#[test]
fn schema_map_work_boundary() -> TestResult {
    let owner = context(&[])?.freeze()?;
    let captured = owner.id().clone();
    let row = ReferenceDefinition::unit("row", &difference(Kind::Angle, "7/3")?)?;
    let column = ReferenceDefinition::unit("column", &difference(Kind::Ratio, "5/2")?)?;
    let quotient = PairingWitness::sensitivity(row.descriptor(), column.descriptor())?;
    let attempt = |work| {
        owner.quotient_normalization_map(
            &quotient,
            &row,
            &column,
            None,
            &LimitsOverride {
                work: Some(work),
                ..LimitsOverride::default()
            },
        )
    };
    let mut refused = 1;
    let mut accepted = nrz_cas::limits::Limits::default().work;
    let expected = attempt(accepted)?;
    while accepted - refused > 1 {
        let middle = refused + (accepted - refused) / 2;
        match attempt(middle) {
            Ok(_) => accepted = middle,
            Err(MapError::Algebra(nrz_cas::Fault {
                kind:
                    nrz_cas::FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    },
                ..
            })) => refused = middle,
            Err(error) => return Err(error.into()),
        }
    }
    assert_eq!(attempt(accepted)?, expected);
    assert_eq!(attempt(accepted + 1)?, expected);
    assert!(matches!(
        attempt(accepted - 1),
        Err(MapError::Algebra(nrz_cas::Fault {
            kind: nrz_cas::FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        }))
    ));
    assert_eq!(
        expected.scale().export(&LimitsOverride::default())?,
        "15/14"
    );
    assert_eq!(owner.id(), &captured);
    Ok(())
}
