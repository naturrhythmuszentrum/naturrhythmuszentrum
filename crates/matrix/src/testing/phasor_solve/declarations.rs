// SPDX-License-Identifier: AGPL-3.0-only
//! Physical source meanings and complete current reconstruction precede every numerical operation.

use crate::phasor_testing::{components, output, tickets};
use crate::reading_testing::{Fixture, TestResult, ledger, map, matrix, meaning, real};
use crate::test_api::{CompanionAdapter, ReadingAdapter, ReadingCoverage};
use nrz_bindings::{
    BinaryOp, Budget, ExponentPolicy, ReferenceDefinition, Registry, Scale,
    readings::{
        CoordinateFamily, CoordinateNormalization, EntryMeaning, PhasorSolveDefinition,
        ReadingRelation, Representation, RepresentationDefinition,
        algebra::{
            Rational,
            readings::SelectionSpec,
            syntax::{AtomicSort, Field, Sort},
        },
    },
};

/// Complete prepared source and result inventories.
type Inventory = (
    Vec<ReadingAdapter<usize>>,
    Vec<ReadingRelation>,
    Representation,
    ReadingCoverage<usize>,
    Vec<CompanionAdapter<usize>>,
);

/// Original symbols include both source blocks and all result/component/parameter declarations.
pub(super) fn fixture() -> TestResult<Fixture> {
    let declarations = inventory();
    let borrowed = declarations
        .iter()
        .map(|(key, sort)| (key.as_str(), sort.clone()))
        .collect::<Vec<_>>();
    Fixture::with_limits(&borrowed, super::policy())
}

/// Count original declarations before any numerical or exact operation starts.
fn inventory() -> Vec<(String, Sort)> {
    let mut declarations = Vec::new();
    for prefix in ["impedance", "voltage"] {
        declarations.extend([
            (format!("{prefix}_root"), matrix(1, 1)),
            (format!("{prefix}_block"), matrix(2, 2)),
            (
                prefix.to_owned(),
                Sort::Value(AtomicSort::Scalar(Field::Complex)),
            ),
        ]);
        for index in 0..4 {
            declarations.push((format!("{prefix}_{index}"), real()));
            for channel in ["total", "error"] {
                for suffix in ["source", "output"] {
                    declarations
                        .push((format!("{prefix}_{index}_{channel}_{suffix}"), matrix(1, 1)));
                }
            }
        }
    }
    output::declare(&mut declarations);
    super::voltage::declare(&mut declarations);
    declarations
}

/// Independent original reactance/voltage families and one fresh two-coordinate current slot.
pub(super) fn readings(fixture: &mut Fixture) -> TestResult<Inventory> {
    let mut adapters = Vec::new();
    let mut relations = Vec::new();
    for (region, prefix, dimensions, coefficients) in [
        (0, "impedance", [2, 1, -3, -2, 0, 0, 0], [0, 1, -1, 0]),
        (1, "voltage", [2, 1, -3, -1, 0, 0, 0], [1, 0, 0, 1]),
    ] {
        let entry = meaning(prefix, dimensions)?;
        let representation = block(fixture, prefix, entry, 1, &coefficients)?;
        let whole = representation.reading().clone();
        adapters.push(ReadingAdapter::new(
            representation,
            tickets(region, &[0, 1, 2, 3]),
            &mut ledger()?,
        )?);
        components(
            fixture,
            &whole,
            prefix,
            region,
            &mut adapters,
            &mut relations,
        )?;
    }
    let current = block(
        fixture,
        "output",
        meaning("current", [0, 0, 0, 1, 0, 0, 0])?,
        2,
        &[1, 0, 0, 1, 0, -1, 1, 0],
    )?;
    let whole = current.reading();
    let mut outputs = vec![ReadingAdapter::new(
        current.clone(),
        tickets(0, &[0, 1, 2, 3]),
        &mut ledger()?,
    )?];
    let mut overlaps = Vec::new();
    components(fixture, whole, "output", 0, &mut outputs, &mut overlaps)?;
    output::vector(fixture, whole, &mut outputs, &mut overlaps)?;
    let companions = output::parameters(fixture, &outputs[1..5])?;
    let coverage =
        ReadingCoverage::new(tickets(0, &[0, 1, 2, 3]), outputs, overlaps, &mut ledger()?)?;
    Ok((adapters, relations, current, coverage, companions))
}

/// The complete affine expansion fixes both zero diagonals or repeated source copies exactly.
fn block(
    fixture: &mut Fixture,
    prefix: &str,
    entry: EntryMeaning,
    width: u32,
    coefficients: &[i64],
) -> TestResult<Representation> {
    let root = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol(&format!("{prefix}_root"))?,
        vec![entry.clone(); usize::try_from(width)?],
        &mut ledger()?,
    )?;
    let symbol = fixture.symbol(&format!("{prefix}_block"))?;
    let family = root.expand(
        &mut fixture.cas,
        &symbol,
        &map(width, 4, coefficients, &[0; 4])?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let whole = fixture.define(
        &family,
        prefix,
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0; 2])?,
        vec![entry; 2],
    )?;
    Ok(Representation::prove(
        &mut fixture.cas,
        &whole,
        RepresentationDefinition::ComplexBlock(symbol),
        &mut ledger()?,
    )?)
}

/// `ohm * ampere = volt` is checked by the explicit physical registry.
pub(super) fn registry(
    adapters: &[ReadingAdapter<usize>],
    current: &Representation,
) -> TestResult<Registry> {
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        adapters[0].reading().entries()[0]
            .descriptor()
            .kind()
            .clone(),
        current.reading().entries()[0].descriptor().kind().clone(),
        adapters[5].reading().entries()[0]
            .descriptor()
            .kind()
            .clone(),
    )?;
    Ok(registry)
}

/// Both unit metrics and a deliberate two-volt reference retain their own exact definitions.
pub(super) fn frames(
    fixture: &mut Fixture,
    definition: &PhasorSolveDefinition,
    budget: &mut Budget,
) -> TestResult<[CoordinateNormalization; 3]> {
    let voltage = definition.voltage().reading().entries();
    let current = definition.output().reading().entries();
    let descriptor = voltage[0].descriptor().clone();
    let reference = ReferenceDefinition::new(
        "two volts",
        &descriptor,
        Scale::Exact(Rational::from_ratio(2, 1, budget)?),
    )?;
    let alternative = EntryMeaning::new(descriptor, reference, None, budget)?;
    Ok([
        CoordinateNormalization::new(&mut fixture.cas, &fixture.context, voltage, budget)?,
        CoordinateNormalization::new(&mut fixture.cas, &fixture.context, current, budget)?,
        CoordinateNormalization::new(
            &mut fixture.cas,
            &fixture.context,
            &[alternative.clone(), alternative],
            budget,
        )?,
    ])
}
