// SPDX-License-Identifier: AGPL-3.0-only
//! Invalid write sets and foreign evidence never produce a disjointness answer.

use crate::testing::builders;
use crate::testing::common::TestResult;
use crate::testing::readings::{Fixture, ledger, matrix, state_case};
use nrz_cas::readings::{AffineMap, EntryIndex, SelectionSpec};
use nrz_cas::syntax::{Declaration, Environment, Operator};
use nrz_cas::{Fault, FaultKind, InvalidCode};

fn refuses<T>(result: Result<T, Fault>, expected: InvalidCode) {
    assert_eq!(
        result.err().map(|fault| fault.kind),
        Some(FaultKind::Invalid(expected))
    );
}

const fn entry(row: u32, column: u32) -> EntryIndex {
    EntryIndex { row, column }
}

/// Inventories, defined readings, witnesses and write handles all retain store ownership.
/// ´test:integration:dependency-foreign-evidence-refuses´
#[test]
fn dependency_foreign_evidence_refuses() -> TestResult {
    let mut source = state_case(0)?;
    let witness = source.prove(&mut ledger()?)?;
    let inventory = source
        .fixture
        .cas
        .witness_dependencies(&witness, &mut ledger()?)?;
    let receiver = state_case(0)?;
    let base = receiver.fixture.symbol("z")?;
    refuses(
        receiver
            .fixture
            .cas
            .reading_dependencies(&source.left, &mut ledger()?),
        InvalidCode::ForeignHandle,
    );
    refuses(
        receiver
            .fixture
            .cas
            .witness_dependencies(&witness, &mut ledger()?),
        InvalidCode::ForeignHandle,
    );
    refuses(
        receiver.fixture.cas.dependency_entry_intersection(
            &inventory,
            &base,
            &[entry(0, 0)],
            &mut ledger()?,
        ),
        InvalidCode::ForeignHandle,
    );
    refuses(
        source.fixture.cas.dependency_selection_intersection(
            &inventory,
            &receiver.right,
            &mut ledger()?,
        ),
        InvalidCode::ForeignHandle,
    );
    refuses(
        source
            .fixture
            .cas
            .dependency_entry_intersection(&inventory, &base, &[], &mut ledger()?),
        InvalidCode::ForeignHandle,
    );
    Ok(())
}

/// Duplicate and out-of-range writes refuse even when their base is absent from the inventory.
/// ´test:integration:dependency-malformed-and-unrelated-writes´
#[test]
fn dependency_malformed_and_unrelated_writes() -> TestResult {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("other", matrix(2, 1)),
    ])?;
    let base = fixture.symbol("z")?;
    let symbol = fixture.symbol("x")?;
    let reading = fixture.cas.define_reading(
        &symbol,
        &base,
        &AffineMap::identity(2, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let inventory = fixture.cas.reading_dependencies(&reading, &mut ledger()?)?;
    let other = fixture.symbol("other")?;
    for source in [&base, &other] {
        refuses(
            fixture.cas.dependency_entry_intersection(
                &inventory,
                source,
                &[entry(0, 0), entry(0, 0)],
                &mut ledger()?,
            ),
            InvalidCode::SelectionDuplicate,
        );
        refuses(
            fixture.cas.dependency_entry_intersection(
                &inventory,
                source,
                &[entry(2, 0)],
                &mut ledger()?,
            ),
            InvalidCode::SelectionOutOfRange,
        );
        refuses(
            fixture.cas.dependency_entry_intersection(
                &inventory,
                source,
                &[entry(0, 1)],
                &mut ledger()?,
            ),
            InvalidCode::SelectionOutOfRange,
        );
    }
    assert_eq!(
        fixture.cas.dependency_entry_intersection(
            &inventory,
            &other,
            &[entry(1, 0)],
            &mut ledger()?
        )?,
        [] as [EntryIndex; 0]
    );
    assert_eq!(
        fixture
            .cas
            .dependency_entry_intersection(&inventory, &base, &[], &mut ledger()?)?,
        [] as [EntryIndex; 0]
    );
    assert_eq!(
        fixture.cas.dependency_entry_intersection(
            &inventory,
            &base,
            &[entry(1, 0), entry(0, 0)],
            &mut ledger()?
        )?,
        [entry(0, 0), entry(1, 0)]
    );
    let selected = fixture
        .cas
        .select_entries(&other, &SelectionSpec::Column(0), &mut ledger()?)?;
    assert_eq!(
        fixture
            .cas
            .dependency_selection_intersection(&inventory, &selected, &mut ledger()?)?,
        [] as [EntryIndex; 0]
    );
    refuses(
        fixture
            .cas
            .dependency_selection_intersection(&inventory, &base, &mut ledger()?),
        InvalidCode::SelectionExpected,
    );
    refuses(
        fixture.cas.reading_dependencies(&base, &mut ledger()?),
        InvalidCode::ReadingDefinition,
    );
    Ok(())
}

/// Changed snapshots of a mentioned symbol refuse instead of being classified as other bases.
/// ´test:integration:dependency-changed-write-declaration-refuses´
#[test]
fn dependency_changed_write_declaration_refuses() -> TestResult {
    let mut case = state_case(0)?;
    let inventory = case
        .fixture
        .cas
        .reading_dependencies(&case.left, &mut ledger()?)?;
    let environment = Environment {
        declarations: vec![Declaration {
            key: "z".into(),
            display_name: "z".into(),
            sort: matrix(3, 1),
        }],
        ..Environment::default()
    };
    let context = builders::core_context(
        &mut case.fixture.cas,
        environment,
        Vec::new(),
        &mut ledger()?,
    )?;
    let changed = builders::expression(
        &mut case.fixture.cas,
        &context,
        Operator::Free("z".into()),
        matrix(3, 1),
        &[],
        &mut ledger()?,
    )?;
    refuses(
        case.fixture.cas.dependency_entry_intersection(
            &inventory,
            &changed,
            &[entry(0, 0)],
            &mut ledger()?,
        ),
        InvalidCode::SelectionDeclaration,
    );
    let selected = case.fixture.cas.select_entries(
        &changed,
        &SelectionSpec::Entry(entry(0, 0)),
        &mut ledger()?,
    )?;
    refuses(
        case.fixture
            .cas
            .dependency_selection_intersection(&inventory, &selected, &mut ledger()?),
        InvalidCode::SelectionDeclaration,
    );
    Ok(())
}

/// A changed definition of a mentioned base cannot reuse its old entry inventory.
/// ´test:integration:dependency-changed-write-definition-refuses´
#[test]
fn dependency_changed_write_definition_refuses() -> TestResult {
    let mut case = state_case(0)?;
    let witness = case.prove(&mut ledger()?)?;
    let inventory = case
        .fixture
        .cas
        .witness_dependencies(&witness, &mut ledger()?)?;
    let base = case.fixture.symbol("z")?;
    let symbol = case.fixture.symbol("x")?;
    let changed = case.fixture.cas.define_reading(
        &symbol,
        &base,
        &crate::testing::readings::map(2, 2, &[-1, 0, 0, 1], &[0, 0])?,
        &mut ledger()?,
    )?;
    refuses(
        case.fixture.cas.dependency_entry_intersection(
            &inventory,
            &changed,
            &[entry(0, 0)],
            &mut ledger()?,
        ),
        InvalidCode::SelectionDefinition,
    );
    Ok(())
}
