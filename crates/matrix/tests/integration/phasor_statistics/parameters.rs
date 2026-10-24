// SPDX-License-Identifier: AGPL-3.0-only
//! One original voltage variance and its own numerical-error source supply every current moment.

use super::reading_testing::{Fixture, TestResult, ledger, matrix};
use nrz_bindings::{
    Budget, PairingWitness, ReferenceDefinition, UncertaintyMode,
    readings::{
        CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily,
        EntryMeaning, ParameterErrorReading, PhasorCovarianceLaw, PhasorSolveDefinition,
        PhysicalReading, Representation, RepresentationDefinition,
        algebra::{
            readings::{AffineMap, SelectionSpec},
            syntax::Sort,
        },
    },
};
use nrz_matrix::{EntryTicket, ReadingAdapter};
use std::rc::Rc;

/// The original variance/error and all three output covariance/error symbols are distinct.
pub(super) fn declare(declarations: &mut Vec<(String, Sort)>, full: bool) {
    for key in [
        "variance",
        "variance_error",
        "below_covariance",
        "cross_covariance",
        "above_covariance",
        "below_error",
        "cross_error",
        "above_error",
    ] {
        let source = matches!(key, "variance" | "variance_error");
        let side = if full && !source { 2 } else { 1 };
        declarations.push((key.to_owned(), matrix(side, side)));
    }
}

/// Complete original parameter adapters and ordered covariance-error definitions.
type Prepared = ([ReadingAdapter<usize>; 2], [Rc<ParameterErrorReading>; 3]);

/// Original source parameter readings and every output companion precede numerical import.
pub(super) fn prepare(
    fixture: &mut Fixture,
    definitions: &[PhasorSolveDefinition; 2],
    targets: &[PhysicalReading; 2],
    budget: &mut Budget,
) -> TestResult<Prepared> {
    let voltage = definitions[0].voltage().reading();
    let parameters = CompanionParameters::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("variance")?,
        CompanionReadingKind::Covariance,
        &[voltage.clone(), voltage.clone()],
        budget,
    )?;
    let values = source_reading(fixture, "variance", &parameters.units()[0], 3)?;
    let descriptor = parameters.units()[0]
        .uncertainty_descriptor()
        .ok_or("variance units")?;
    let pairing = PairingWitness::uncertainty(UncertaintyMode::Radius, descriptor, None, None)?;
    let error = source_reading(fixture, "variance_error", &pairing, 4)?;
    let mut outputs = Vec::new();
    for (left, right, key) in [(0, 0, "below"), (0, 1, "cross"), (1, 1, "above")] {
        let law = PhasorCovarianceLaw::new(
            &fixture.cas,
            [definitions[left].clone(), definitions[right].clone()],
            [targets[left].clone(), targets[right].clone()],
            budget,
        )?;
        let symbol = fixture.symbol(&format!("{key}_covariance"))?;
        let companion = CompanionReading::phasor_covariance(
            &mut fixture.cas,
            &symbol,
            &parameters,
            law,
            budget,
        )?;
        let destination = fixture.symbol(&format!("{key}_error"))?;
        let prior = fixture.symbol("variance_error")?;
        outputs.push(Rc::new(ParameterErrorReading::define(
            &mut fixture.cas,
            &destination,
            &prior,
            &companion,
            budget,
        )?));
    }
    Ok((
        [values, error],
        outputs.try_into().map_err(|_| "three covariance blocks")?,
    ))
}

/// Complete identity readings authenticate original parameter symbols and their exact quantity kinds.
fn source_reading(
    fixture: &mut Fixture,
    key: &str,
    pairing: &PairingWitness,
    region: usize,
) -> TestResult<ReadingAdapter<usize>> {
    let descriptor = pairing
        .uncertainty_descriptor()
        .ok_or("parameter units")?
        .clone();
    let reference = ReferenceDefinition::unit(key, &descriptor)?;
    let entry = EntryMeaning::new(descriptor, reference, None, &mut ledger()?)?;
    let symbol = fixture.symbol(key)?;
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &symbol,
        vec![entry.clone()],
        &mut ledger()?,
    )?;
    let reading = fixture.define(
        &family,
        key,
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 1,
            columns: 1,
        },
        AffineMap::identity(1, &mut ledger()?)?,
        vec![entry],
    )?;
    let representation = Representation::prove(
        &mut fixture.cas,
        &reading,
        RepresentationDefinition::Defined {
            symbol,
            expansion: AffineMap::identity(1, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    Ok(ReadingAdapter::new(
        representation,
        vec![EntryTicket { region, index: 0 }],
        &mut ledger()?,
    )?)
}
