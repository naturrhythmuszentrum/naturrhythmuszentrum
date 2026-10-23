// SPDX-License-Identifier: AGPL-3.0-only
//! Reading dependency inventories retain exact syntax while writes follow projected entries.

use crate::testing::common::TestResult;
use crate::testing::readings::{
    ComplexChange, Fixture, complex_case, ledger, map, matrix, state_case,
};
use nrz_cas::proof::{DomainContract, Judgment, Primitive, Statement};
use nrz_cas::readings::{
    AffineMap, DependencyInventory, DependencyPremiseKind, EntryIndex, SelectionSpec,
};
use nrz_cas::{Handle, InvalidCode};

const fn entry(row: u32, column: u32) -> EntryIndex {
    EntryIndex { row, column }
}

fn operand(fixture: &Fixture, value: &Handle, position: usize) -> TestResult<Handle> {
    fixture
        .cas
        .operands(value, &mut ledger()?)?
        .get(position)
        .cloned()
        .ok_or_else(|| "operand".into())
}

fn reads(inventory: &DependencyInventory, symbol: &str) -> TestResult<Vec<EntryIndex>> {
    inventory
        .contents()
        .bases
        .iter()
        .find(|base| base.symbol == symbol)
        .map(|base| base.entries.clone())
        .ok_or_else(|| "inventory base".into())
}

fn declarations(inventory: &DependencyInventory) -> Vec<&str> {
    inventory
        .contents()
        .declarations
        .iter()
        .map(|value| value.declaration.key.as_str())
        .collect()
}

/// Complex readings retain the block family, selections and equations and share the written corner.
/// ´test:integration:dependency-complex-corner-inventories´
#[test]
fn dependency_complex_corner_inventories() -> TestResult {
    let mut case = complex_case(ComplexChange::None)?;
    let complex = operand(&case.fixture, &case.right, 0)?;
    let selection = operand(&case.fixture, &complex, 1)?;
    let block = operand(&case.fixture, &selection, 0)?;
    let real = case
        .fixture
        .cas
        .reading_dependencies(&case.left, &mut ledger()?)?;
    let whole = case
        .fixture
        .cas
        .reading_dependencies(&complex, &mut ledger()?)?;
    assert_eq!(declarations(&real), ["block", "c", "r", "z"]);
    assert_eq!(real.contents().selections.len(), 1);
    assert_eq!(real.contents().definitions.len(), 2);
    assert_eq!(reads(&real, "block")?, [entry(0, 0)]);
    assert_eq!(reads(&real, "z")?, [entry(0, 0)]);
    assert_eq!(
        whole.contents().selections[0].selection.indices(),
        &[entry(0, 0), entry(1, 0), entry(0, 1), entry(1, 1)]
    );
    assert_eq!(reads(&whole, "block")?, [entry(0, 0), entry(1, 0)]);
    let witness = case.prove(&mut ledger()?)?;
    let proof = case
        .fixture
        .cas
        .witness_dependencies(&witness, &mut ledger()?)?;
    assert_eq!(proof.contents().selections.len(), 2);
    assert_eq!(proof.contents().definitions.len(), 3);
    for kind in [
        DependencyPremiseKind::Context,
        DependencyPremiseKind::Domain,
        DependencyPremiseKind::Family,
        DependencyPremiseKind::Statement,
        DependencyPremiseKind::Proof,
    ] {
        assert!(
            proof
                .contents()
                .premises
                .iter()
                .any(|premise| premise.kind == kind)
        );
    }
    let write = case.fixture.cas.select_entries(
        &block,
        &SelectionSpec::Entry(entry(0, 0)),
        &mut ledger()?,
    )?;
    for inventory in [&real, &whole, &proof] {
        assert_eq!(
            case.fixture.cas.dependency_selection_intersection(
                inventory,
                &write,
                &mut ledger()?
            )?,
            [entry(0, 0)]
        );
    }
    Ok(())
}

/// A current write touches the whole state and current but preserves voltage and its projection proof.
/// ´test:integration:dependency-state-projection-invalidation´
#[test]
fn dependency_state_projection_invalidation() -> TestResult {
    let mut case = state_case(0)?;
    let whole = operand(&case.fixture, &case.right, 0)?;
    let base = case.fixture.symbol("z")?;
    let write = case.fixture.cas.select_entries(
        &base,
        &SelectionSpec::Entry(entry(1, 0)),
        &mut ledger()?,
    )?;
    let symbol = case.fixture.symbol("i")?;
    let current = case.fixture.cas.define_reading(
        &symbol,
        &write,
        &AffineMap::identity(1, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let voltage = case
        .fixture
        .cas
        .reading_dependencies(&case.left, &mut ledger()?)?;
    let state = case
        .fixture
        .cas
        .reading_dependencies(&whole, &mut ledger()?)?;
    let current = case
        .fixture
        .cas
        .reading_dependencies(&current, &mut ledger()?)?;
    let witness = case.prove(&mut ledger()?)?;
    let proof = case
        .fixture
        .cas
        .witness_dependencies(&witness, &mut ledger()?)?;
    assert_eq!(declarations(&proof), ["i", "v", "x", "z"]);
    assert_eq!(proof.contents().definitions.len(), 2);
    assert_eq!(proof.contents().selections.len(), 3);
    assert_eq!(reads(&proof, "z")?, [entry(0, 0)]);
    assert_eq!(reads(&proof, "x")?, [entry(0, 0)]);
    assert_eq!(reads(&state, "z")?, [entry(0, 0), entry(1, 0)]);
    for inventory in [&voltage, &proof] {
        assert_eq!(
            case.fixture.cas.dependency_selection_intersection(
                inventory,
                &write,
                &mut ledger()?
            )?,
            [] as [EntryIndex; 0]
        );
    }
    for inventory in [&state, &current] {
        assert_eq!(
            case.fixture.cas.dependency_entry_intersection(
                inventory,
                &base,
                &[entry(1, 0)],
                &mut ledger()?
            )?,
            [entry(1, 0)]
        );
    }
    Ok(())
}

/// The producer and fresh replayer inventory exactly the same retained closure and projected reads.
/// ´test:integration:dependency-inventory-fresh-replay-equality´
#[test]
fn dependency_inventory_fresh_replay_equality() -> TestResult {
    for complex in [false, true] {
        let mut source = if complex {
            complex_case(ComplexChange::None)?
        } else {
            state_case(0)?
        };
        let witness = source.prove(&mut ledger()?)?;
        let original = source
            .fixture
            .cas
            .witness_dependencies(&witness, &mut ledger()?)?;
        let bytes = source
            .fixture
            .cas
            .export_witness(&witness, &mut ledger()?)?;
        let mut receiver = if complex {
            complex_case(ComplexChange::None)?
        } else {
            state_case(0)?
        };
        let expected = receiver.expected()?;
        let replayed = receiver
            .fixture
            .cas
            .replay(&bytes, &expected, &mut ledger()?)?;
        let copied = receiver
            .fixture
            .cas
            .witness_dependencies(&replayed, &mut ledger()?)?;
        assert_eq!(original, copied);
        assert_eq!(
            source
                .fixture
                .cas
                .reading_dependencies(&source.left, &mut ledger()?)?,
            receiver
                .fixture
                .cas
                .reading_dependencies(&receiver.left, &mut ledger()?)?
        );
    }
    Ok(())
}

/// Removing a defining equation changes the retained inventory and cannot accept the old witness.
/// ´test:integration:dependency-omitted-equation-changes-inventory´
#[test]
fn dependency_omitted_equation_changes_inventory() -> TestResult {
    let mut source = complex_case(ComplexChange::None)?;
    let complete = source
        .fixture
        .cas
        .reading_dependencies(&source.right, &mut ledger()?)?;
    let mut receiver = complex_case(ComplexChange::MissingDefinition)?;
    let omitted = receiver
        .fixture
        .cas
        .reading_dependencies(&receiver.right, &mut ledger()?)?;
    assert_ne!(complete, omitted);
    assert_eq!(omitted.contents().definitions.len(), 0);
    let witness = source.prove(&mut ledger()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let expected = receiver.expected()?;
    let refusal = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?);
    assert_eq!(
        refusal.err().map(|fault| fault.kind),
        Some(nrz_cas::FaultKind::Invalid(InvalidCode::EndpointMismatch))
    );
    Ok(())
}

/// Transitivity through a third reading inventories every defining equation in the proof closure.
/// ´test:integration:dependency-composed-proof-retains-union´
#[test]
fn dependency_composed_proof_retains_union() -> TestResult {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("a", matrix(2, 1)),
        ("b", matrix(2, 1)),
        ("c", matrix(2, 1)),
    ])?;
    let base = fixture.symbol("z")?;
    let mut values = Vec::new();
    for name in ["a", "b", "c"] {
        let symbol = fixture.symbol(name)?;
        values.push(fixture.cas.define_reading(
            &symbol,
            &base,
            &map(2, 2, &[1, 0, 0, 1], &[0, 0])?,
            &mut ledger()?,
        )?);
    }
    let [first, middle, last]: [Handle; 3] = values.try_into().map_err(|_| "three readings")?;
    let domain = fixture.cas.admissible_region(&fixture.context)?;
    let first_step = crate::testing::witness::equality(
        &fixture.context,
        &domain,
        Primitive::Jets,
        &first,
        &middle,
    );
    let second_step = crate::testing::witness::equality(
        &fixture.context,
        &domain,
        Primitive::Jets,
        &middle,
        &last,
    );
    let mut final_step = crate::testing::witness::equality(
        &fixture.context,
        &domain,
        Primitive::Transitivity,
        &first,
        &last,
    );
    final_step.premises = vec![0, 1];
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[first_step, second_step, final_step],
        &mut ledger()?,
    )?;
    let inventory = fixture.cas.witness_dependencies(&witness, &mut ledger()?)?;
    assert_eq!(inventory.contents().definitions.len(), 3);
    assert_eq!(reads(&inventory, "z")?, [entry(0, 0), entry(1, 0)]);
    let expected = Judgment {
        context: fixture.context.clone(),
        layer: nrz_cas::domain::Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: first,
            right: last,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    };
    let bytes = fixture.cas.export_witness(&witness, &mut ledger()?)?;
    let replayed = fixture.cas.replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(
        inventory,
        fixture
            .cas
            .witness_dependencies(&replayed, &mut ledger()?)?
    );
    Ok(())
}

/// The inventory table is exactly the existing witness record table, with no separate wire form.
/// ´test:integration:dependency-records-match-retained-witness´
#[test]
fn dependency_records_match_retained_witness() -> TestResult {
    let mut case = state_case(0)?;
    let witness = case.prove(&mut ledger()?)?;
    let bytes = case.fixture.cas.export_witness(&witness, &mut ledger()?)?;
    let inventory = case
        .fixture
        .cas
        .witness_dependencies(&witness, &mut ledger()?)?;
    let records = crate::testing::answer::records(&bytes)?;
    assert_eq!(records.len(), inventory.contents().records.len());
    for ((_, range), original) in records.iter().zip(&inventory.contents().records) {
        let start = range.start.checked_sub(5).ok_or("record frame")?;
        assert_eq!(
            bytes.get(start..range.end).ok_or("retained record")?,
            original
        );
    }
    Ok(())
}

/// Reordered declaration admissions change local handles without changing portable inventories.
/// ´test:integration:dependency-inventory-allocation-order-independent´
#[test]
fn dependency_inventory_allocation_order_independent() -> TestResult {
    let mut source = state_case(0)?;
    let mut receiver = reordered_state()?;
    assert_ne!(
        source.fixture.symbol("z")?.logical_id(),
        receiver.fixture.symbol("z")?.logical_id()
    );
    let witness = source.prove(&mut ledger()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let expected = receiver.expected()?;
    let replayed = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(
        source
            .fixture
            .cas
            .witness_dependencies(&witness, &mut ledger()?)?,
        receiver
            .fixture
            .cas
            .witness_dependencies(&replayed, &mut ledger()?)?
    );
    Ok(())
}

fn reordered_state() -> TestResult<crate::testing::readings::ReadingCase> {
    let mut fixture = Fixture::new(&[
        ("i", crate::testing::common::real()),
        ("v", crate::testing::common::real()),
        ("x", matrix(2, 1)),
        ("z", matrix(2, 1)),
    ])?;
    let base = fixture.symbol("z")?;
    let symbol = fixture.symbol("x")?;
    let selected = fixture
        .cas
        .select_entries(&base, &SelectionSpec::Column(0), &mut ledger()?)?;
    let whole = fixture.cas.define_reading(
        &symbol,
        &selected,
        &AffineMap::identity(2, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let spec = SelectionSpec::Entry(entry(0, 0));
    let selected = fixture.cas.select_entries(&base, &spec, &mut ledger()?)?;
    let symbol = fixture.symbol("v")?;
    let left = fixture.cas.define_reading(
        &symbol,
        &selected,
        &AffineMap::identity(1, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let right = fixture.cas.select_entries(&whole, &spec, &mut ledger()?)?;
    Ok(crate::testing::readings::ReadingCase {
        fixture,
        left,
        right,
    })
}
