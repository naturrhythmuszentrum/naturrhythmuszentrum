// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-family representations never infer structure from centers or absent storage.

use crate::testing::readings::{
    Fixture, TestResult, complex_case, ledger, map, matrix, meaning, state_case,
};
use nrz_bindings::readings::{
    CoordinateFamily, Representation, RepresentationDefinition, RepresentationKind,
};
use nrz_cas::{
    readings::{AffineMap, EntryIndex, SelectionSpec},
    syntax::{AtomicSort, Field, Sort},
};

/// Complete Complex blocks replay with no proof in the independently built receiving graph.
/// ´test:integration:complex-block-representation-replays´
#[test]
fn complex_block_representation_replays() -> TestResult {
    let mut source = complex_case(0, 1, true)?;
    let symbol = source.fixture.symbol("block")?;
    let representation = Representation::prove(
        &mut source.fixture.cas,
        &source.whole,
        RepresentationDefinition::ComplexBlock(symbol),
        &mut ledger()?,
    )?;
    let bytes = source
        .fixture
        .cas
        .export_witness(representation.witness(), &mut ledger()?)?;
    let mut receiver = complex_case(0, 1, true)?;
    let symbol = receiver.fixture.symbol("block")?;
    let checked = Representation::replay(
        &mut receiver.fixture.cas,
        &receiver.whole,
        RepresentationDefinition::ComplexBlock(symbol),
        &bytes,
        &mut ledger()?,
    )?;
    assert_eq!(checked.kind(), RepresentationKind::ComplexBlock);
    assert!(checked.witness().is_discharged());
    Ok(())
}

/// An unstructured block cannot gain negative/repeated-coordinate equations from its center.
/// ´test:integration:complex-block-representation-refuses-unstructured-family´
#[test]
fn complex_block_representation_refuses_unstructured_family() -> TestResult {
    let mut case = complex_case(0, 1, false)?;
    let symbol = case.fixture.symbol("block")?;
    assert!(
        Representation::prove(
            &mut case.fixture.cas,
            &case.whole,
            RepresentationDefinition::ComplexBlock(symbol),
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// A pair has an explicit checked complete reconstruction and keeps the Complex sort.
/// ´test:integration:complex-pair-representation-replays´
#[test]
fn complex_pair_representation_replays() -> TestResult {
    let (mut sender, source) = pair()?;
    let symbol = sender.symbol("z")?;
    let checked = Representation::prove(
        &mut sender.cas,
        &source,
        RepresentationDefinition::ComponentPair(symbol),
        &mut ledger()?,
    )?;
    let bytes = sender
        .cas
        .export_witness(checked.witness(), &mut ledger()?)?;
    let (mut receiver, target) = pair()?;
    let symbol = receiver.symbol("z")?;
    let replayed = Representation::replay(
        &mut receiver.cas,
        &target,
        RepresentationDefinition::ComponentPair(symbol),
        &bytes,
        &mut ledger()?,
    )?;
    assert_eq!(replayed.kind(), RepresentationKind::ComponentPair);
    Ok(())
}

/// Independent component-pair declarations and physical meanings.
fn pair() -> TestResult<(Fixture, nrz_bindings::readings::PhysicalReading)> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("c", Sort::Value(AtomicSort::Scalar(Field::Complex))),
    ])?;
    let entry = meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        vec![entry.clone(); 2],
        &mut ledger()?,
    )?;
    let value = fixture.define(
        &family,
        "c",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        vec![entry; 2],
    )?;
    Ok((fixture, value))
}

/// Only exact constancy can remove numerical storage; a pending varying input still refuses.
/// ´test:integration:exact-representation-distinguishes-missing-input´
#[test]
fn exact_representation_distinguishes_missing_input() -> TestResult {
    let mut case = state_case(0)?;
    assert!(
        Representation::prove(
            &mut case.fixture.cas,
            &case.component,
            RepresentationDefinition::Exact,
            &mut ledger()?
        )
        .is_err()
    );
    let fixed = case.fixture.define(
        case.whole.family(),
        "v",
        SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        map(1, 1, &[0], &[5])?,
        case.component.entries().to_vec(),
    )?;
    let checked = Representation::prove(
        &mut case.fixture.cas,
        &fixed,
        RepresentationDefinition::Exact,
        &mut ledger()?,
    )?;
    assert_eq!(checked.kind(), RepresentationKind::None);
    assert_eq!(
        checked.expansion().offsets()[0],
        nrz_cas::Rational::from_ratio(5, 1, &mut ledger()?)?
    );
    Ok(())
}

/// Another layout must reconstruct every selected coordinate through its own checked map.
/// ´test:integration:defined-representation-refuses-incomplete-expansion´
#[test]
fn defined_representation_refuses_incomplete_expansion() -> TestResult {
    let mut case = state_case(0)?;
    let symbol = case.fixture.symbol("z")?;
    let definition = RepresentationDefinition::Defined {
        symbol: symbol.clone(),
        expansion: AffineMap::identity(2, &mut ledger()?)?,
    };
    Representation::prove(
        &mut case.fixture.cas,
        &case.whole,
        definition,
        &mut ledger()?,
    )?;
    let definition = RepresentationDefinition::Defined {
        symbol,
        expansion: map(2, 2, &[1, 0, 0, 0], &[0, 0])?,
    };
    assert!(
        Representation::prove(
            &mut case.fixture.cas,
            &case.whole,
            definition,
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Representation proof and independent replay preserve exact Work and payload ceilings.
/// ´test:integration:representation-resource-boundaries´
#[test]
fn representation_resource_boundaries() -> TestResult {
    for replay in [false, true] {
        crate::testing::readings::boundaries(|limits, precharge| {
            let mut case = complex_case(0, 1, true)?;
            let symbol = case.fixture.symbol("block")?;
            let mut budget = nrz_bindings::Budget::new(limits)?;
            budget.work(
                precharge,
                nrz_cas::Location::new("representation boundary precharge"),
            )?;
            if replay {
                let mut sender = complex_case(0, 1, true)?;
                let declaration = sender.fixture.symbol("block")?;
                let proof = Representation::prove(
                    &mut sender.fixture.cas,
                    &sender.whole,
                    RepresentationDefinition::ComplexBlock(declaration),
                    &mut ledger()?,
                )?;
                let bytes = sender
                    .fixture
                    .cas
                    .export_witness(proof.witness(), &mut ledger()?)?;
                Representation::replay(
                    &mut case.fixture.cas,
                    &case.whole,
                    RepresentationDefinition::ComplexBlock(symbol),
                    &bytes,
                    &mut budget,
                )?;
            } else {
                Representation::prove(
                    &mut case.fixture.cas,
                    &case.whole,
                    RepresentationDefinition::ComplexBlock(symbol),
                    &mut budget,
                )?;
            }
            Ok(budget.usage().work)
        })?;
    }
    Ok(())
}
