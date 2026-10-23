// SPDX-License-Identifier: AGPL-3.0-only
//! Exact entry judgments use authenticated bases and preserve operand positions.

use crate::testing::builders;
use crate::testing::common::TestResult;
use crate::testing::readings::{Fixture, ledger, map, matrix};
use nrz_cas::readings::{EntryIndex, EntryMultiplicity, EntryOrder, SelectionSpec, SharedEntry};
use nrz_cas::syntax::{Declaration, Environment, Operator};
use nrz_cas::{Fault, FaultKind, Handle, InvalidCode};

const fn entry(row: u32, column: u32) -> EntryIndex {
    EntryIndex { row, column }
}

fn refuses<T>(result: Result<T, Fault>, expected: InvalidCode) {
    assert_eq!(
        result.err().map(|fault| fault.kind),
        Some(FaultKind::Invalid(expected))
    );
}

fn select(fixture: &mut Fixture, base: &Handle, spec: &SelectionSpec) -> TestResult<Handle> {
    Ok(fixture.cas.select_entries(base, spec, &mut ledger()?)?)
}

/// A row and column share one entry at independently retained operand positions.
/// ´test:integration:entry-sets-partial-overlap´
#[test]
fn entry_sets_partial_overlap() -> TestResult {
    let mut fixture = Fixture::new(&[("b", matrix(2, 3))])?;
    let base = fixture.symbol("b")?;
    let row = select(&mut fixture, &base, &SelectionSpec::Row(1))?;
    let column = select(&mut fixture, &base, &SelectionSpec::Column(2))?;
    let relation = fixture
        .cas
        .entry_set_relation(&row, &column, &mut ledger()?)?;
    assert_eq!(
        relation.intersection,
        [SharedEntry {
            entry: entry(1, 2),
            left_position: 2,
            right_position: 1,
        }]
    );
    assert!(!relation.left_contains_right());
    assert!(!relation.right_contains_left());
    assert!(!relation.equal_sets());
    assert!(!relation.disjoint());
    assert!(!relation.same_order());
    Ok(())
}

/// Whole Complex blocks contain the corner and disjoint state components cover exactly once.
/// ´test:integration:entry-sets-worked-coverages´
#[test]
fn entry_sets_worked_coverages() -> TestResult {
    let mut fixture = Fixture::new(&[("block", matrix(2, 2)), ("x", matrix(2, 1))])?;
    let base = fixture.symbol("block")?;
    let whole = select(
        &mut fixture,
        &base,
        &SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
    )?;
    let corner = select(&mut fixture, &base, &SelectionSpec::Entry(entry(0, 0)))?;
    let relation = fixture
        .cas
        .entry_set_relation(&whole, &corner, &mut ledger()?)?;
    assert_eq!(
        relation.intersection,
        [SharedEntry {
            entry: entry(0, 0),
            left_position: 0,
            right_position: 0,
        }]
    );
    assert!(relation.left_contains_right());
    assert!(!relation.right_contains_left());
    let base = fixture.symbol("x")?;
    let voltage = select(&mut fixture, &base, &SelectionSpec::Entry(entry(0, 0)))?;
    let current = select(&mut fixture, &base, &SelectionSpec::Entry(entry(1, 0)))?;
    let relation = fixture
        .cas
        .entry_set_relation(&voltage, &current, &mut ledger()?)?;
    assert!(relation.disjoint());
    assert_eq!(relation.intersection, [] as [SharedEntry; 0]);
    let cover = fixture
        .cas
        .entry_coverage(&base, &[voltage, current], &mut ledger()?)?;
    assert!(cover.exactly_once());
    assert_eq!(cover.uncovered, [] as [EntryIndex; 0]);
    assert_eq!(cover.multiple, [] as [EntryMultiplicity; 0]);
    Ok(())
}

/// Missing and repeated coverage are both reported in logical column order.
/// ´test:integration:entry-coverage-holes-and-multiplicity´
#[test]
fn entry_coverage_holes_and_multiplicity() -> TestResult {
    let mut fixture = Fixture::new(&[("b", matrix(2, 2))])?;
    let base = fixture.symbol("b")?;
    let row = select(&mut fixture, &base, &SelectionSpec::Row(0))?;
    let column = select(&mut fixture, &base, &SelectionSpec::Column(0))?;
    let cover = fixture
        .cas
        .entry_coverage(&base, &[row.clone(), column, row], &mut ledger()?)?;
    assert!(!cover.complete());
    assert!(!cover.exactly_once());
    assert_eq!(cover.uncovered, [entry(1, 1)]);
    assert_eq!(
        cover.multiple,
        [
            EntryMultiplicity {
                entry: entry(0, 0),
                count: 3
            },
            EntryMultiplicity {
                entry: entry(0, 1),
                count: 2
            },
        ]
    );
    Ok(())
}

/// Reordering equal sets preserves set equality and changes both reading and position identity.
/// ´test:integration:entry-set-order-is-independent´
#[test]
fn entry_set_order_is_independent() -> TestResult {
    let mut fixture = Fixture::new(&[("x", matrix(2, 1))])?;
    let base = fixture.symbol("x")?;
    let forward = select(&mut fixture, &base, &SelectionSpec::Column(0))?;
    let reverse = select(
        &mut fixture,
        &base,
        &SelectionSpec::Entries {
            entries: vec![entry(1, 0), entry(0, 0)],
            order: EntryOrder::Given,
        },
    )?;
    assert_ne!(forward, reverse);
    let relation = fixture
        .cas
        .entry_set_relation(&reverse, &forward, &mut ledger()?)?;
    assert!(relation.equal_sets());
    assert!(!relation.same_order());
    assert_eq!(
        relation.intersection,
        [
            SharedEntry {
                entry: entry(1, 0),
                left_position: 0,
                right_position: 1
            },
            SharedEntry {
                entry: entry(0, 0),
                left_position: 1,
                right_position: 0
            },
        ]
    );
    assert!(
        fixture
            .cas
            .entry_set_relation(&forward, &forward, &mut ledger()?)?
            .same_order()
    );
    Ok(())
}

/// Empty selections and empty bases retain the ordinary finite-set laws.
/// ´test:integration:entry-sets-empty-laws´
#[test]
fn entry_sets_empty_laws() -> TestResult {
    for rows in [0, 2] {
        let mut fixture = Fixture::new(&[("x", matrix(rows, 1))])?;
        let base = fixture.symbol("x")?;
        let selected = select(
            &mut fixture,
            &base,
            &SelectionSpec::Entries {
                entries: Vec::new(),
                order: EntryOrder::Given,
            },
        )?;
        let relation = fixture
            .cas
            .entry_set_relation(&selected, &selected, &mut ledger()?)?;
        assert!(relation.equal_sets());
        assert!(relation.same_order());
        assert!(relation.disjoint());
        let cover = fixture.cas.entry_coverage(&base, &[], &mut ledger()?)?;
        assert_eq!(cover.exactly_once(), rows == 0);
        assert_eq!(cover.uncovered.len(), usize::try_from(rows)?);
    }
    Ok(())
}

/// Every subset pair of a four-entry rectangle agrees with an independent bitset oracle.
/// ´test:integration:entry-sets-exhaustive-small-oracle´
#[test]
fn entry_sets_exhaustive_small_oracle() -> TestResult {
    let mut fixture = Fixture::new(&[("b", matrix(2, 2))])?;
    let base = fixture.symbol("b")?;
    let mut selections = Vec::new();
    for mask in 0..16_u32 {
        let entries = (0..4)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| entry(bit % 2, bit / 2))
            .collect();
        selections.push(select(
            &mut fixture,
            &base,
            &SelectionSpec::Entries {
                entries,
                order: EntryOrder::Given,
            },
        )?);
    }
    for (left_mask, left) in selections.iter().enumerate() {
        for (right_mask, right) in selections.iter().enumerate() {
            let relation = fixture
                .cas
                .entry_set_relation(left, right, &mut ledger()?)?;
            let shared = left_mask & right_mask;
            assert_eq!(
                relation.intersection.len(),
                usize::try_from(shared.count_ones())?
            );
            assert_eq!(relation.left_contains_right(), shared == right_mask);
            assert_eq!(relation.right_contains_left(), shared == left_mask);
            assert_eq!(relation.equal_sets(), left_mask == right_mask);
            assert_eq!(relation.disjoint(), shared == 0);
            let cover = fixture.cas.entry_coverage(
                &base,
                &[left.clone(), right.clone()],
                &mut ledger()?,
            )?;
            assert_eq!(cover.complete(), left_mask | right_mask == 15);
            assert_eq!(cover.multiple.len(), usize::try_from(shared.count_ones())?);
        }
    }
    Ok(())
}

/// Neither a foreign handle nor an unrelated same-shaped symbol supplies a common base.
/// ´test:integration:entry-sets-named-handle-refusals´
#[test]
fn entry_sets_named_handle_refusals() -> TestResult {
    let mut fixture = Fixture::new(&[("x", matrix(2, 1)), ("y", matrix(2, 1))])?;
    let base = fixture.symbol("x")?;
    let other = fixture.symbol("y")?;
    let first = select(&mut fixture, &base, &SelectionSpec::Column(0))?;
    let second = select(&mut fixture, &other, &SelectionSpec::Column(0))?;
    refuses(
        fixture
            .cas
            .entry_set_relation(&first, &second, &mut ledger()?),
        InvalidCode::SelectionBase,
    );
    refuses(
        fixture.cas.entry_coverage(&base, &[second], &mut ledger()?),
        InvalidCode::SelectionBase,
    );
    refuses(
        fixture
            .cas
            .entry_set_relation(&base, &first, &mut ledger()?),
        InvalidCode::SelectionExpected,
    );
    let mut foreign = Fixture::new(&[("x", matrix(2, 1))])?;
    let other = foreign.symbol("x")?;
    let selected = select(&mut foreign, &other, &SelectionSpec::Column(0))?;
    refuses(
        fixture
            .cas
            .entry_set_relation(&first, &selected, &mut ledger()?),
        InvalidCode::ForeignHandle,
    );
    refuses(
        fixture.cas.entry_coverage(&other, &[], &mut ledger()?),
        InvalidCode::ForeignHandle,
    );
    Ok(())
}

/// A same-key declaration under another snapshot never silently compares by coordinates.
/// ´test:integration:entry-sets-changed-declaration-refuses´
#[test]
fn entry_sets_changed_declaration_refuses() -> TestResult {
    let mut fixture = Fixture::new(&[("x", matrix(2, 1))])?;
    let base = fixture.symbol("x")?;
    let first = select(&mut fixture, &base, &SelectionSpec::Column(0))?;
    let environment = Environment {
        declarations: vec![Declaration {
            key: "x".into(),
            display_name: "changed".into(),
            sort: matrix(3, 1),
        }],
        ..Environment::default()
    };
    let context =
        builders::core_context(&mut fixture.cas, environment, Vec::new(), &mut ledger()?)?;
    let changed = builders::expression(
        &mut fixture.cas,
        &context,
        Operator::Free("x".into()),
        matrix(3, 1),
        &[],
        &mut ledger()?,
    )?;
    let second = select(&mut fixture, &changed, &SelectionSpec::Column(0))?;
    refuses(
        fixture
            .cas
            .entry_set_relation(&first, &second, &mut ledger()?),
        InvalidCode::SelectionDeclaration,
    );
    refuses(
        fixture.cas.entry_coverage(&base, &[second], &mut ledger()?),
        InvalidCode::SelectionDeclaration,
    );
    Ok(())
}

/// Different definitions under one unchanged declaration remain distinct bases.
/// ´test:integration:entry-sets-changed-definition-refuses´
#[test]
fn entry_sets_changed_definition_refuses() -> TestResult {
    let mut fixture = Fixture::new(&[("z", matrix(1, 1)), ("x", matrix(1, 1))])?;
    let source = fixture.symbol("z")?;
    let symbol = fixture.symbol("x")?;
    let first =
        fixture
            .cas
            .define_reading(&symbol, &source, &map(1, 1, &[1], &[0])?, &mut ledger()?)?;
    let second =
        fixture
            .cas
            .define_reading(&symbol, &source, &map(1, 1, &[-1], &[0])?, &mut ledger()?)?;
    let left = select(&mut fixture, &first, &SelectionSpec::Column(0))?;
    let right = select(&mut fixture, &second, &SelectionSpec::Column(0))?;
    refuses(
        fixture
            .cas
            .entry_set_relation(&left, &right, &mut ledger()?),
        InvalidCode::SelectionDefinition,
    );
    refuses(
        fixture.cas.entry_coverage(&first, &[right], &mut ledger()?),
        InvalidCode::SelectionDefinition,
    );
    Ok(())
}

/// A zero-entry rectangle never loops over its potentially huge unused axis.
/// ´test:integration:entry-coverage-empty-large-axis-is-bounded´
#[test]
fn entry_coverage_empty_large_axis_is_bounded() -> TestResult {
    for shape in [(0, u32::MAX), (u32::MAX, 0)] {
        let fixture = Fixture::new(&[("b", matrix(shape.0, shape.1))])?;
        let base = fixture.symbol("b")?;
        let mut budget = ledger()?;
        let cover = fixture.cas.entry_coverage(&base, &[], &mut budget)?;
        assert!(cover.exactly_once());
        assert_eq!(budget.usage().work, 1);
    }
    Ok(())
}
