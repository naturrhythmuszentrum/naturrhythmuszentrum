// SPDX-License-Identifier: AGPL-3.0-only
//! Physical overlap witnesses replay in independently constructed CAS graphs.

use crate::testing::readings::{Case, TestResult, complex_case, ledger, state_case};
use nrz_bindings::readings::ReadingRelation;

/// Reconstruct both expected endpoints before replaying the complete CAS closure.
fn portable(mut source: Case, mut receiver: Case, coordinate: u32) -> TestResult {
    let left = source
        .component
        .project(&mut source.fixture.cas, &[0], &mut ledger()?)?;
    let right = source
        .whole
        .project(&mut source.fixture.cas, &[coordinate], &mut ledger()?)?;
    let relation = ReadingRelation::prove(&mut source.fixture.cas, left, right, &mut ledger()?)?;
    let bytes = relation.export(&source.fixture.cas, &mut ledger()?)?;
    let left = receiver
        .component
        .project(&mut receiver.fixture.cas, &[0], &mut ledger()?)?;
    let right = receiver
        .whole
        .project(&mut receiver.fixture.cas, &[coordinate], &mut ledger()?)?;
    let replayed = ReadingRelation::replay(
        &mut receiver.fixture.cas,
        left.clone(),
        right.clone(),
        &bytes,
        &mut ledger()?,
    )?;
    replayed.check_targets(&left, &right)?;
    assert!(replayed.witness().is_discharged());
    assert_eq!(
        replayed.export(&receiver.fixture.cas, &mut ledger()?)?,
        bytes
    );
    Ok(())
}

/// The voltage and current components retain unlike units in one free family.
/// ´test:integration:mixed-state-physical-projections-replay´
#[test]
fn mixed_state_physical_projections_replay() -> TestResult {
    for coordinate in 0..2 {
        portable(state_case(coordinate)?, state_case(coordinate)?, coordinate)?;
    }
    Ok(())
}

/// Either real copy in a Complex block has its own portable whole-family claim.
/// ´test:integration:complex-physical-readings-replay´
#[test]
fn complex_physical_readings_replay() -> TestResult {
    for copy in 0..2 {
        portable(
            complex_case(copy, 1, true)?,
            complex_case(copy, 1, true)?,
            0,
        )?;
    }
    Ok(())
}

/// Two two-entry readings compare only the physical component in their overlap.
/// ´test:integration:partial-physical-overlap-replays´
#[test]
fn partial_physical_overlap_replays() -> TestResult {
    let (mut sender, endpoints) = partial()?;
    let relation = ReadingRelation::prove(
        &mut sender.cas,
        endpoints[0].clone(),
        endpoints[1].clone(),
        &mut ledger()?,
    )?;
    let bytes = relation.export(&sender.cas, &mut ledger()?)?;
    let (mut receiver, endpoints) = partial()?;
    ReadingRelation::replay(
        &mut receiver.cas,
        endpoints[0].clone(),
        endpoints[1].clone(),
        &bytes,
        &mut ledger()?,
    )?;
    Ok(())
}

/// Build each endpoint through its original ordered selection, never an asserted equation.
fn partial() -> TestResult<(
    crate::testing::readings::Fixture,
    [nrz_bindings::readings::ReadingProjection; 2],
)> {
    use crate::testing::readings::{Fixture, matrix, meaning};
    use nrz_bindings::readings::CoordinateFamily;
    use nrz_cas::readings::{AffineMap, SelectionSpec};
    let mut fixture = Fixture::new(&[
        ("z", matrix(3, 1)),
        ("x", matrix(2, 1)),
        ("y", matrix(2, 1)),
    ])?;
    let entry = meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        vec![entry.clone(); 3],
        &mut ledger()?,
    )?;
    let mut endpoints = Vec::new();
    for (key, row, coordinate) in [("x", 0, 1), ("y", 1, 0)] {
        let reading = fixture.define(
            &family,
            key,
            SelectionSpec::Block {
                row,
                column: 0,
                rows: 2,
                columns: 1,
            },
            AffineMap::identity(2, &mut ledger()?)?,
            vec![entry.clone(); 2],
        )?;
        endpoints.push(reading.project(&mut fixture.cas, &[coordinate], &mut ledger()?)?);
    }
    Ok((fixture, endpoints.try_into().map_err(|_| "endpoint pair")?))
}

/// Three readings compose affine translations and inverse maps through one common family.
/// ´test:integration:physical-readings-compose-through-common-family´
#[test]
fn physical_readings_compose_through_common_family() -> TestResult {
    for (left, right) in [(0, 1), (1, 2), (0, 2)] {
        let (mut sender, endpoints) = triple()?;
        let relation = ReadingRelation::prove(
            &mut sender.cas,
            endpoints[left].clone(),
            endpoints[right].clone(),
            &mut ledger()?,
        )?;
        let bytes = relation.export(&sender.cas, &mut ledger()?)?;
        let (mut receiver, endpoints) = triple()?;
        ReadingRelation::replay(
            &mut receiver.cas,
            endpoints[left].clone(),
            endpoints[right].clone(),
            &bytes,
            &mut ledger()?,
        )?;
    }
    Ok(())
}

/// Explicit maps have independently specified inverse coefficients and both translations.
fn triple() -> TestResult<(
    crate::testing::readings::Fixture,
    Vec<nrz_bindings::readings::ReadingProjection>,
)> {
    use crate::testing::readings::{Fixture, map, matrix, meaning};
    use nrz_bindings::readings::CoordinateFamily;
    use nrz_cas::readings::{AffineMap, SelectionSpec};
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("shifted", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("y", matrix(2, 1)),
        ("w", matrix(2, 1)),
    ])?;
    let entry = meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let entries = vec![entry; 2];
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        entries.clone(),
        &mut ledger()?,
    )?;
    let shifted_symbol = fixture.symbol("shifted")?;
    let shifted = family.expand(
        &mut fixture.cas,
        &shifted_symbol,
        &map(2, 2, &[1, 1, 0, 1], &[2, 3])?,
        entries.clone(),
        &mut ledger()?,
    )?;
    let mut endpoints = Vec::new();
    for key in ["x", "y", "w"] {
        let (source, affine) = if key == "y" {
            (&shifted, map(2, 2, &[1, -1, 0, 1], &[1, -3])?)
        } else {
            (&family, AffineMap::identity(2, &mut ledger()?)?)
        };
        let reading = fixture.define(
            source,
            key,
            SelectionSpec::Column(0),
            affine,
            entries.clone(),
        )?;
        endpoints.push(reading.project(&mut fixture.cas, &[0, 1], &mut ledger()?)?);
    }
    Ok((fixture, endpoints))
}
