// SPDX-License-Identifier: AGPL-3.0-only
//! Refusals preserve physical declarations, selected copies and whole-family equations.

use crate::testing::readings::{
    Fixture, TestResult, complex_case, ledger, map, matrix, meaning, real, state_case,
};
use nrz_bindings::readings::{
    CoordinateFamily, EntryMeaning, PhysicalReading, ReadingAxis, ReadingDefinition, ReadingError,
    ReadingRelation, ReadingShape,
};
use nrz_cas::readings::{AffineMap, EntryIndex, SelectionSpec};

/// A sign or absent expansion cannot establish the second copy's real-component equation.
/// ´test:integration:physical-overlap-refuses-sign-and-center-only-structure´
#[test]
fn physical_overlap_refuses_sign_and_center_only_structure() -> TestResult {
    for (sign, structured) in [(-1, true), (1, false)] {
        let mut case = complex_case(1, sign, structured)?;
        let left = case
            .component
            .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
        let right = case
            .whole
            .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
        assert!(
            ReadingRelation::prove(&mut case.fixture.cas, left, right, &mut ledger()?).is_err()
        );
    }
    Ok(())
}

/// Another valid selected copy cannot reuse the first copy's expected judgment.
/// ´test:integration:physical-overlap-replay-refuses-selected-copy-substitution´
#[test]
fn physical_overlap_replay_refuses_selected_copy_substitution() -> TestResult {
    let mut source = complex_case(0, 1, true)?;
    let left = source
        .component
        .project(&mut source.fixture.cas, &[0], &mut ledger()?)?;
    let right = source
        .whole
        .project(&mut source.fixture.cas, &[0], &mut ledger()?)?;
    let relation = ReadingRelation::prove(&mut source.fixture.cas, left, right, &mut ledger()?)?;
    let bytes = relation.export(&source.fixture.cas, &mut ledger()?)?;
    let mut receiver = complex_case(1, 1, true)?;
    let left = receiver
        .component
        .project(&mut receiver.fixture.cas, &[0], &mut ledger()?)?;
    let right = receiver
        .whole
        .project(&mut receiver.fixture.cas, &[0], &mut ledger()?)?;
    assert!(
        ReadingRelation::replay(
            &mut receiver.fixture.cas,
            left,
            right,
            &bytes,
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Equal display names and units supply no relation between two free families.
/// ´test:integration:physical-overlap-refuses-unrelated-families´
#[test]
fn physical_overlap_refuses_unrelated_families() -> TestResult {
    let mut fixture = Fixture::new(&[
        ("z", matrix(1, 1)),
        ("w", matrix(1, 1)),
        ("x", real()),
        ("y", real()),
    ])?;
    let entry = meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let mut projections = Vec::new();
    for (source, target) in [("z", "x"), ("w", "y")] {
        let family = CoordinateFamily::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol(source)?,
            vec![entry.clone()],
            &mut ledger()?,
        )?;
        let value = fixture.define(
            &family,
            target,
            SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
            AffineMap::identity(1, &mut ledger()?)?,
            vec![entry.clone()],
        )?;
        projections.push(value.project(&mut fixture.cas, &[0], &mut ledger()?)?);
    }
    let right = projections.pop().ok_or("right")?;
    let left = projections.pop().ok_or("left")?;
    assert!(matches!(
        ReadingRelation::prove(&mut fixture.cas, left, right, &mut ledger()?),
        Err(ReadingError::Unrelated)
    ));
    Ok(())
}

/// A nonzero map cannot reinterpret voltage as current or replace its reference definition.
/// ´test:integration:physical-reading-refuses-kind-and-reference-mutations´
#[test]
fn physical_reading_refuses_kind_and_reference_mutations() -> TestResult {
    let mut case = state_case(0)?;
    let original = case.component.entries()[0].clone();
    let replacements = [
        case.whole.entries()[1].clone(),
        EntryMeaning::new(
            original.descriptor().clone(),
            nrz_bindings::ReferenceDefinition::unit("voltage", original.descriptor())?,
            None,
            &mut ledger()?,
        )?,
    ];
    for replacement in replacements {
        let definition = ReadingDefinition {
            symbol: case.fixture.symbol("v")?,
            selection: SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
            map: AffineMap::identity(1, &mut ledger()?)?,
            entries: vec![replacement],
        };
        assert!(
            PhysicalReading::define(
                &mut case.fixture.cas,
                case.whole.family(),
                definition,
                &mut ledger()?
            )
            .is_err()
        );
    }
    Ok(())
}

/// Invalid original named dimensions refuse before a zero reading could hide them.
/// ´test:integration:physical-family-checks-all-original-kinds´
#[test]
fn physical_family_checks_all_original_kinds() -> TestResult {
    let fixture = Fixture::new(&[("z", matrix(2, 1))])?;
    let entries = vec![
        meaning("same_kind", [1, 0, 0, 0, 0, 0, 0])?,
        meaning("same_kind", [0, 1, 0, 0, 0, 0, 0])?,
    ];
    assert!(matches!(
        CoordinateFamily::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("z")?,
            entries,
            &mut ledger()?
        ),
        Err(ReadingError::Units(
            nrz_bindings::UnitError::ConflictingKind(_)
        ))
    ));
    Ok(())
}

/// A changed origin cannot become a compatible point reading at equal scale and dimension.
/// ´test:integration:physical-reading-refuses-origin-mutation´
#[test]
fn physical_reading_refuses_origin_mutation() -> TestResult {
    let difference = meaning("temperature", [0, 0, 0, 0, 1, 0, 0])?;
    let point = nrz_bindings::Descriptor::new(
        difference.descriptor().kind().clone(),
        nrz_bindings::Measure::coherent(),
        nrz_bindings::Role::Point,
    )?;
    let original = nrz_bindings::OriginDefinition::new("origin", &point)?;
    let changed = nrz_bindings::OriginDefinition::new("origin", &point)?;
    let left = EntryMeaning::new(
        point.clone(),
        difference.reference().clone(),
        Some(original),
        &mut ledger()?,
    )?;
    let right = EntryMeaning::new(
        point,
        difference.reference().clone(),
        Some(changed),
        &mut ledger()?,
    )?;
    assert!(matches!(left.check(&right), Err(ReadingError::Origin)));
    Ok(())
}

/// Shape blocks must cover the exact rectangle once, and axis partitions retain order.
/// ´test:integration:physical-shape-refuses-malformed-partitions´
#[test]
fn physical_shape_refuses_malformed_partitions() -> TestResult {
    let case = state_case(0)?;
    let row = case.fixture.symbol("rows")?;
    for (order, boundaries) in [
        (vec![0, 0], vec![2]),
        (vec![0, 1], vec![1]),
        (vec![0, 1], vec![2, 1]),
    ] {
        assert!(
            ReadingAxis::new(
                &case.fixture.cas,
                &case.fixture.context,
                &row,
                order,
                boundaries,
                &mut ledger()?
            )
            .is_err()
        );
    }
    let axes = [
        ReadingAxis::new(
            &case.fixture.cas,
            &case.fixture.context,
            &row,
            vec![0, 1],
            vec![1, 2],
            &mut ledger()?,
        )?,
        ReadingAxis::new(
            &case.fixture.cas,
            &case.fixture.context,
            &case.fixture.symbol("columns")?,
            vec![0],
            vec![1],
            &mut ledger()?,
        )?,
    ];
    let block = SelectionSpec::Block {
        row: 0,
        column: 0,
        rows: 2,
        columns: 1,
    };
    ReadingShape::new(
        &case.whole,
        axes.clone(),
        std::slice::from_ref(&block),
        &mut ledger()?,
    )?;
    assert!(matches!(
        ReadingShape::new(&case.whole, axes.clone(), &[], &mut ledger()?),
        Err(ReadingError::Partition)
    ));
    assert!(matches!(
        ReadingShape::new(&case.whole, axes, &[block.clone(), block], &mut ledger()?),
        Err(ReadingError::Partition)
    ));
    Ok(())
}

/// Reordering homogeneous block coordinates changes the equation even though units agree.
/// ´test:integration:physical-reading-refuses-coordinate-order-mutation´
#[test]
fn physical_reading_refuses_coordinate_order_mutation() -> TestResult {
    let mut case = complex_case(0, 1, true)?;
    let changed = case.fixture.define(
        case.whole.family(),
        "c",
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 2, &[0, 1, 0, 0, 1, 0, 0, 0], &[0, 0])?,
        case.whole.entries().to_vec(),
    )?;
    let left = case
        .component
        .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let right = changed.project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    assert!(ReadingRelation::prove(&mut case.fixture.cas, left, right, &mut ledger()?).is_err());
    Ok(())
}

/// A receiver rebuilds the exact context rather than accepting the sender's domain.
/// ´test:integration:physical-reading-replay-refuses-changed-domain´
#[test]
fn physical_reading_replay_refuses_changed_domain() -> TestResult {
    use nrz_cas::{
        Reference,
        domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr},
        syntax::Draft,
    };
    let mut case = state_case(0)?;
    let left = case
        .component
        .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let right = case
        .whole
        .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let relation = ReadingRelation::prove(&mut case.fixture.cas, left, right, &mut ledger()?)?;
    let bytes = relation.export(&case.fixture.cas, &mut ledger()?)?;
    let header = case.fixture.cas.context_header(&case.fixture.context)?;
    let nodes = case.fixture.cas.admit(
        &[
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Empty,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header),
                admissible: Reference::Local(0),
                assumptions: Vec::new(),
            }),
        ],
        &mut ledger()?,
    )?;
    let family = CoordinateFamily::new(
        &case.fixture.cas,
        nodes.get(1).ok_or("changed context")?,
        case.whole.family().root(),
        case.whole.entries().to_vec(),
        &mut ledger()?,
    )?;
    let whole = case.fixture.define(
        &family,
        "x",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        case.whole.entries().to_vec(),
    )?;
    let component = case.fixture.define(
        &family,
        "v",
        SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        AffineMap::identity(1, &mut ledger()?)?,
        case.component.entries().to_vec(),
    )?;
    let left = component.project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let right = whole.project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    assert!(
        ReadingRelation::replay(&mut case.fixture.cas, left, right, &bytes, &mut ledger()?)
            .is_err()
    );
    Ok(())
}
