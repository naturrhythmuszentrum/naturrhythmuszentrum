// SPDX-License-Identifier: AGPL-3.0-only
//! Domain dependencies and exact affine support remain visible independently of endpoint projections.

use crate::testing::builders::admit;
use crate::testing::common::{TestResult, real};
use crate::testing::readings::{Fixture, ReadingCase, ledger, map, matrix, state_case};
use nrz_cas::Reference;
use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use nrz_cas::readings::{DependencyPremiseKind, EntryIndex, SelectionSpec};
use nrz_cas::syntax::{Draft, PredicateKind};

const fn entry(row: u32) -> EntryIndex {
    EntryIndex { row, column: 0 }
}

fn guarded_state() -> TestResult<ReadingCase> {
    let mut case = state_case(0)?;
    let base = case.fixture.symbol("z")?;
    let selected =
        case.fixture
            .cas
            .select_entries(&base, &SelectionSpec::Entry(entry(1)), &mut ledger()?)?;
    let header = case.fixture.cas.context_header(&case.fixture.context)?;
    let domain = case.fixture.cas.admissible_region(&case.fixture.context)?;
    let predicate = admit(
        &mut case.fixture.cas,
        Draft::Condition {
            kind: PredicateKind::NonNegative,
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(selected)],
        },
        &mut ledger()?,
    )?;
    let domain = admit(
        &mut case.fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(domain),
                predicate: Reference::Existing(predicate),
            },
        }),
        &mut ledger()?,
    )?;
    case.fixture.context = admit(
        &mut case.fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(domain),
            assumptions: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    Ok(case)
}

/// A predicate on the other component contributes its entry even when neither endpoint reads it.
/// ´test:integration:dependency-domain-premises-add-entry-reads´
#[test]
fn dependency_domain_premises_add_entry_reads() -> TestResult {
    let mut case = guarded_state()?;
    let witness = case.prove(&mut ledger()?)?;
    let inventory = case
        .fixture
        .cas
        .witness_dependencies(&witness, &mut ledger()?)?;
    assert!(
        inventory
            .contents()
            .premises
            .iter()
            .any(|premise| premise.kind == DependencyPremiseKind::Predicate)
    );
    let base = case.fixture.symbol("z")?;
    assert_eq!(
        case.fixture.cas.dependency_entry_intersection(
            &inventory,
            &base,
            &[entry(1)],
            &mut ledger()?
        )?,
        [entry(1)]
    );
    let bytes = case.fixture.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = guarded_state()?;
    let expected = receiver.expected()?;
    let replayed = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(
        inventory,
        receiver
            .fixture
            .cas
            .witness_dependencies(&replayed, &mut ledger()?)?
    );
    Ok(())
}

/// Exact composition cancels a source coordinate while retaining both intermediate entry reads.
/// ´test:integration:dependency-composed-support-cancels-exactly´
#[test]
fn dependency_composed_support_cancels_exactly() -> TestResult {
    let mut fixture = Fixture::new(&[("z", matrix(2, 1)), ("x", matrix(2, 1)), ("r", real())])?;
    let base = fixture.symbol("z")?;
    let symbol = fixture.symbol("x")?;
    let intermediate = fixture.cas.define_reading(
        &symbol,
        &base,
        &map(2, 2, &[1, 1, 0, 1], &[2, 3])?,
        &mut ledger()?,
    )?;
    let symbol = fixture.symbol("r")?;
    let reading = fixture.cas.define_reading(
        &symbol,
        &intermediate,
        &map(2, 1, &[1, -1], &[1])?,
        &mut ledger()?,
    )?;
    let inventory = fixture.cas.reading_dependencies(&reading, &mut ledger()?)?;
    assert_eq!(inventory.contents().definitions.len(), 2);
    assert_eq!(
        fixture.cas.dependency_entry_intersection(
            &inventory,
            &base,
            &[entry(0), entry(1)],
            &mut ledger()?
        )?,
        [entry(0)]
    );
    assert_eq!(
        fixture.cas.dependency_entry_intersection(
            &inventory,
            &intermediate,
            &[entry(0), entry(1)],
            &mut ledger()?
        )?,
        [entry(0), entry(1)]
    );
    Ok(())
}

/// Zero coefficients remove entry reads without dropping any original selection or equation.
/// ´test:integration:dependency-zero-map-retains-structural-closure´
#[test]
fn dependency_zero_map_retains_structural_closure() -> TestResult {
    let mut fixture = Fixture::new(&[("z", matrix(2, 1)), ("r", real())])?;
    let base = fixture.symbol("z")?;
    let selected = fixture
        .cas
        .select_entries(&base, &SelectionSpec::Column(0), &mut ledger()?)?;
    let symbol = fixture.symbol("r")?;
    let reading = fixture.cas.define_reading(
        &symbol,
        &selected,
        &map(2, 1, &[0, 0], &[7])?,
        &mut ledger()?,
    )?;
    let inventory = fixture.cas.reading_dependencies(&reading, &mut ledger()?)?;
    assert_eq!(inventory.contents().definitions.len(), 1);
    assert_eq!(
        inventory.contents().selections[0].selection.indices(),
        &[entry(0), entry(1)]
    );
    assert_eq!(
        fixture.cas.dependency_entry_intersection(
            &inventory,
            &base,
            &[entry(0), entry(1)],
            &mut ledger()?
        )?,
        [] as [EntryIndex; 0]
    );
    assert_eq!(
        fixture.cas.dependency_entry_intersection(
            &inventory,
            &reading,
            &[entry(0)],
            &mut ledger()?
        )?,
        [entry(0)]
    );
    Ok(())
}

/// Empty transposed readings do no work proportional to an unused huge axis.
/// ´test:integration:dependency-empty-transpose-is-bounded´
#[test]
fn dependency_empty_transpose_is_bounded() -> TestResult {
    for shape in [(0, u32::MAX), (u32::MAX, 0)] {
        let mut fixture = Fixture::new(&[
            ("z", matrix(shape.0, shape.1)),
            ("x", matrix(shape.0, shape.1)),
        ])?;
        let source = fixture.symbol("z")?;
        let symbol = fixture.symbol("x")?;
        let reading = fixture.cas.define_reading(
            &symbol,
            &source,
            &nrz_cas::readings::AffineMap::identity(0, &mut ledger()?)?,
            &mut ledger()?,
        )?;
        let transposed = fixture.term(
            nrz_cas::syntax::Operator::Transpose,
            matrix(shape.1, shape.0),
            &[reading],
        )?;
        let inventory = fixture
            .cas
            .reading_dependencies(&transposed, &mut ledger()?)?;
        for base in &inventory.contents().bases {
            assert_eq!(base.entries, [] as [EntryIndex; 0]);
        }
    }
    Ok(())
}
