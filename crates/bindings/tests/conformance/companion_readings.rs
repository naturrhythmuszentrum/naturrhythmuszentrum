// SPDX-License-Identifier: AGPL-3.0-only
//! Exact companion maps preserve stored-copy discrepancies and joint source lineage.

use crate::testing::readings::{Fixture, TestResult, ledger, map, matrix, meaning, real};
use nrz_bindings::readings::{
    CompanionParameters, CompanionReading, CompanionReadingError, CompanionReadingKind,
    CompanionRelation, CoordinateFamily, PhysicalReading, ReadingRelation,
};
use nrz_bindings::{CompanionKind, Descriptor};
use nrz_cas::readings::{AffineMap, EntryIndex, SelectionSpec};

/// Physical block and two independently selected copies of its real coordinate.
struct Block {
    /// Original graph with all companion declarations.
    fixture: Fixture,
    /// Reading of all four expanded storage coordinates.
    whole: PhysicalReading,
    /// The two distinct real-coordinate copies.
    copies: [PhysicalReading; 2],
}

/// Build original parameter symbols without any numerical payload or covariance assumption.
fn block() -> TestResult<Block> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("block", matrix(2, 2)),
        ("value", matrix(2, 2)),
        ("r", real()),
        ("s", real()),
        ("radii", matrix(4, 1)),
        ("bounds", matrix(4, 1)),
        ("parameter", matrix(2, 2)),
        ("covariance", matrix(4, 4)),
        ("parameter_error", matrix(2, 2)),
        ("covariance_error", matrix(4, 4)),
        ("first", matrix(1, 1)),
        ("second", matrix(1, 1)),
    ])?;
    let entry = meaning("impedance", [2, 1, -3, -2, 0, 0, 0])?;
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        vec![entry.clone(); 2],
        &mut ledger()?,
    )?;
    let symbol = fixture.symbol("block")?;
    let expanded = family.expand(
        &mut fixture.cas,
        &symbol,
        &map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0, 0, 0, 0])?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let whole = fixture.define(
        &expanded,
        "value",
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        AffineMap::identity(4, &mut ledger()?)?,
        vec![entry.clone(); 4],
    )?;
    let first = fixture.define(
        &expanded,
        "r",
        SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        AffineMap::identity(1, &mut ledger()?)?,
        vec![entry.clone()],
    )?;
    let second = fixture.define(
        &expanded,
        "s",
        SelectionSpec::Entry(EntryIndex { row: 1, column: 1 }),
        AffineMap::identity(1, &mut ledger()?)?,
        vec![entry],
    )?;
    Ok(Block {
        fixture,
        whole,
        copies: [first, second],
    })
}

/// Covariance includes both repeated-coordinate correlation and the negative imaginary copy.
/// ´test:integration:block-companion-covariance-preserves-lineage´
#[test]
fn block_companion_covariance_preserves_lineage() -> TestResult {
    let mut case = block()?;
    let targets = [case.whole.clone(), case.whole.clone()];
    let parameters = CompanionParameters::new(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("parameter")?,
        CompanionReadingKind::Covariance,
        &targets,
        &mut ledger()?,
    )?;
    let symbol = case.fixture.symbol("covariance")?;
    let covariance = CompanionReading::define(
        &mut case.fixture.cas,
        &symbol,
        &parameters,
        &targets,
        &mut ledger()?,
    )?;
    assert_eq!(covariance.map().inputs(), 4);
    assert_eq!(covariance.map().outputs(), 16);
    let coefficients = covariance.map().coefficients();
    assert_eq!(coefficients[12 * 4], nrz_cas::Rational::one());
    assert_eq!(
        coefficients[9 * 4 + 3],
        nrz_cas::Rational::one().negated(&mut ledger()?)?
    );
    assert!(
        covariance
            .map()
            .offsets()
            .iter()
            .all(nrz_cas::Rational::is_zero)
    );
    Ok(())
}

/// Radius companions keep all stored copies rather than replacing them with free-coordinate errors.
/// ´test:integration:block-radius-companions-keep-copy-discrepancies´
#[test]
fn block_radius_companions_keep_copy_discrepancies() -> TestResult {
    let mut case = block()?;
    let targets = [case.whole.clone()];
    let parameters = CompanionParameters::new(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("radii")?,
        CompanionReadingKind::Radius(CompanionKind::NumericalError),
        &targets,
        &mut ledger()?,
    )?;
    let symbol = case.fixture.symbol("bounds")?;
    let bound = CompanionReading::define(
        &mut case.fixture.cas,
        &symbol,
        &parameters,
        &targets,
        &mut ledger()?,
    )?;
    assert_eq!(bound.map(), &AffineMap::identity(4, &mut ledger()?)?);
    assert_eq!(
        bound.kind(),
        CompanionReadingKind::Radius(CompanionKind::NumericalError)
    );
    assert!(matches!(
        bound.check_units(&vec![Descriptor::ratio(); 4]),
        Err(CompanionReadingError::Unit)
    ));
    assert!(bound.check_targets(&[case.copies[0].clone()]).is_err());
    Ok(())
}

/// Equal physical copies share covariance only through checked target and parameter equations.
/// ´test:integration:copy-covariance-relation-replays´
#[test]
fn copy_covariance_relation_replays() -> TestResult {
    let (mut sender, endpoints, relations) = covariance_relation()?;
    let relation = CompanionRelation::prove(&mut sender.cas, endpoints, relations, &mut ledger()?)?;
    let bytes = sender
        .cas
        .export_witness(relation.witness(), &mut ledger()?)?;
    let (mut receiver, endpoints, relations) = covariance_relation()?;
    let replayed = CompanionRelation::replay(
        &mut receiver.cas,
        endpoints,
        relations,
        &bytes,
        &mut ledger()?,
    )?;
    assert!(replayed.witness().is_discharged());
    Ok(())
}

/// Independently reconstruct original physical target proofs before the companion proof.
fn covariance_relation() -> TestResult<(Fixture, [CompanionReading; 2], Vec<ReadingRelation>)> {
    let mut case = block()?;
    let first_targets = [case.copies[0].clone(), case.copies[0].clone()];
    let parameters = CompanionParameters::new(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("parameter")?,
        CompanionReadingKind::Covariance,
        &first_targets,
        &mut ledger()?,
    )?;
    let first_symbol = case.fixture.symbol("first")?;
    let first = CompanionReading::define(
        &mut case.fixture.cas,
        &first_symbol,
        &parameters,
        &first_targets,
        &mut ledger()?,
    )?;
    let second_symbol = case.fixture.symbol("second")?;
    let second = CompanionReading::define(
        &mut case.fixture.cas,
        &second_symbol,
        &parameters,
        &[case.copies[1].clone(), case.copies[1].clone()],
        &mut ledger()?,
    )?;
    let left = case.copies[0].project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let right = case.copies[1].project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let relation = ReadingRelation::prove(&mut case.fixture.cas, left, right, &mut ledger()?)?;
    Ok((
        case.fixture,
        [first, second],
        vec![relation.clone(), relation],
    ))
}

/// Covariance evaluation error has its own parameter target and squared/product units.
/// ´test:integration:covariance-error-reading-targets-parameter´
#[test]
fn covariance_error_reading_targets_parameter() -> TestResult {
    let mut case = block()?;
    let targets = [case.whole.clone(), case.whole.clone()];
    let parameters = CompanionParameters::new(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("parameter")?,
        CompanionReadingKind::Covariance,
        &targets,
        &mut ledger()?,
    )?;
    let symbol = case.fixture.symbol("covariance")?;
    let covariance = CompanionReading::define(
        &mut case.fixture.cas,
        &symbol,
        &parameters,
        &targets,
        &mut ledger()?,
    )?;
    let source = case.fixture.symbol("parameter_error")?;
    let output = case.fixture.symbol("covariance_error")?;
    let error = nrz_bindings::readings::ParameterErrorReading::define(
        &mut case.fixture.cas,
        &output,
        &source,
        &covariance,
        &mut ledger()?,
    )?;
    assert_eq!(error.target().node(), covariance.node());
    assert_eq!(error.source_target(), parameters.node());
    assert_eq!(
        error.map().coefficients()[9 * 4 + 3],
        nrz_cas::Rational::one()
    );
    let physical = covariance
        .units()
        .iter()
        .map(|pair| pair.uncertainty_descriptor().cloned().ok_or("unit"))
        .collect::<Result<Vec<_>, _>>()?;
    assert!(error.check_units(&physical).is_err());
    Ok(())
}

/// Each companion constructor and composed lineage map has exact resource boundaries.
/// ´test:integration:companion-reading-resource-boundaries´
#[test]
fn companion_reading_resource_boundaries() -> TestResult {
    for operation in 0..6 {
        crate::testing::readings::boundaries(|limits, precharge| {
            companion_attempt(operation, limits, precharge)
        })?;
    }
    Ok(())
}

/// Build all unmeasured original declarations before constraining one tested operation.
fn companion_attempt(
    operation: u32,
    limits: nrz_bindings::Limits,
    precharge: u64,
) -> TestResult<u64> {
    let mut case = block()?;
    let mut budget = nrz_bindings::Budget::new(limits)?;
    budget.work(
        precharge,
        nrz_cas::Location::new("companion boundary precharge"),
    )?;
    if operation == 0 {
        case.whole.family_map(&case.fixture.cas, &mut budget)?;
    } else {
        let targets = [case.whole.clone(), case.whole.clone()];
        let symbol = case.fixture.symbol("parameter")?;
        if operation == 1 {
            CompanionParameters::new(
                &case.fixture.cas,
                &case.fixture.context,
                &symbol,
                CompanionReadingKind::Covariance,
                &targets,
                &mut budget,
            )?;
        } else {
            let parameters = CompanionParameters::new(
                &case.fixture.cas,
                &case.fixture.context,
                &symbol,
                CompanionReadingKind::Covariance,
                &targets,
                &mut ledger()?,
            )?;
            let output = case.fixture.symbol("covariance")?;
            if operation == 2 {
                CompanionReading::define(
                    &mut case.fixture.cas,
                    &output,
                    &parameters,
                    &targets,
                    &mut budget,
                )?;
            } else {
                companion_tail(operation, &mut case, &parameters, &targets, &mut budget)?;
            }
        }
    }
    Ok(budget.usage().work)
}

/// Parameter-error and exact companion equality have separately measured construction ledgers.
fn companion_tail(
    operation: u32,
    case: &mut Block,
    parameters: &CompanionParameters,
    targets: &[PhysicalReading],
    budget: &mut nrz_bindings::Budget,
) -> TestResult {
    if operation == 3 {
        let output = case.fixture.symbol("covariance")?;
        let covariance = CompanionReading::define(
            &mut case.fixture.cas,
            &output,
            parameters,
            targets,
            &mut ledger()?,
        )?;
        let source = case.fixture.symbol("parameter_error")?;
        let output = case.fixture.symbol("covariance_error")?;
        nrz_bindings::readings::ParameterErrorReading::define(
            &mut case.fixture.cas,
            &output,
            &source,
            &covariance,
            budget,
        )?;
    } else {
        let (mut fixture, endpoints, relations) = covariance_relation()?;
        if operation == 4 {
            CompanionRelation::prove(&mut fixture.cas, endpoints, relations, budget)?;
        } else {
            let (mut sender, first, targets) = covariance_relation()?;
            let relation =
                CompanionRelation::prove(&mut sender.cas, first, targets, &mut ledger()?)?;
            let bytes = sender
                .cas
                .export_witness(relation.witness(), &mut ledger()?)?;
            CompanionRelation::replay(&mut fixture.cas, endpoints, relations, &bytes, budget)?;
        }
    }
    Ok(())
}

/// Radius transport takes absolute coefficients and removes every value offset.
/// ´test:integration:radius-reading-uses-absolute-map-without-origin´
#[test]
fn radius_reading_uses_absolute_map_without_origin() -> TestResult {
    let mut case = block()?;
    let transformed = case.fixture.define(
        case.whole.family(),
        "r",
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 1, &[-2, 0, 0, 0], &[7])?,
        case.copies[0].entries().to_vec(),
    )?;
    let targets = [transformed];
    let parameters = CompanionParameters::new(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("radii")?,
        CompanionReadingKind::Radius(CompanionKind::DeterministicBound),
        &targets,
        &mut ledger()?,
    )?;
    let symbol = case.fixture.symbol("first")?;
    let bounds = CompanionReading::define(
        &mut case.fixture.cas,
        &symbol,
        &parameters,
        &targets,
        &mut ledger()?,
    )?;
    assert_eq!(bounds.map(), &map(4, 1, &[2, 0, 0, 0], &[0])?);
    Ok(())
}
