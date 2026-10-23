// SPDX-License-Identifier: AGPL-3.0-only
//! A fresh voltage result checks restoration through the already certified current block.

use crate::phasor_testing::{components, tickets};
use crate::reading_testing::{Fixture, TestResult, ledger, map, matrix, real};
use crate::test_api::{CompanionAdapter, ReadingAdapter, ReadingCoverage};
use nrz_bindings::{
    Budget, CompanionKind, Registry,
    readings::{
        CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily,
        PhasorResultDefinition, PhasorSolveDefinition, ReadingRelation, Representation,
        RepresentationDefinition,
        algebra::{
            readings::{AffineMap, SelectionSpec},
            syntax::{AtomicSort, Field, Sort},
        },
    },
};

/// Every restored voltage view and radius parameter exists before current execution.
pub(crate) struct Declarations {
    /// Checked native `Z*I` result with coherent voltage meaning.
    pub(crate) definition: PhasorResultDefinition,
    /// Whole voltage, component vector and all stored scalar-copy views.
    pub(crate) coverage: ReadingCoverage<usize>,
    /// Both output radius channels, with complete original targets.
    pub(crate) companions: Vec<CompanionAdapter<usize>>,
}

/// A separate fresh output root cannot replace the declared source voltage.
pub(super) fn declare(declarations: &mut Vec<(String, Sort)>) {
    declarations.extend([
        ("restored_root".to_owned(), matrix(2, 1)),
        ("restored_block".to_owned(), matrix(2, 2)),
        (
            "restored".to_owned(),
            Sort::Value(AtomicSort::Scalar(Field::Complex)),
        ),
        ("restored_vector".to_owned(), matrix(2, 1)),
    ]);
    for index in 0..4 {
        declarations.push((format!("restored_{index}"), real()));
        for channel in ["total", "error"] {
            for suffix in ["source", "output"] {
                declarations.push((format!("restored_{index}_{channel}_{suffix}"), matrix(1, 1)));
            }
        }
    }
}

/// The original physical product rule supplies the restored voltage kind.
pub(super) fn prepare(
    fixture: &mut Fixture,
    solve: &PhasorSolveDefinition,
    registry: &Registry,
    budget: &mut Budget,
) -> TestResult<Declarations> {
    let entry = solve.voltage().reading().entries()[0].clone();
    let root = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("restored_root")?,
        vec![entry.clone(); 2],
        &mut ledger()?,
    )?;
    let symbol = fixture.symbol("restored_block")?;
    let family = root.expand(
        &mut fixture.cas,
        &symbol,
        &map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0; 4])?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let whole = fixture.define(
        &family,
        "restored",
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0; 2])?,
        vec![entry; 2],
    )?;
    let representation = Representation::prove(
        &mut fixture.cas,
        &whole,
        RepresentationDefinition::ComplexBlock(symbol),
        &mut ledger()?,
    )?;
    let definition = PhasorResultDefinition::new(
        solve.correspondence().clone(),
        representation.clone(),
        registry,
        budget,
    )?;
    let mut adapters = vec![ReadingAdapter::new(
        representation,
        tickets(0, &[0, 1, 2, 3]),
        &mut ledger()?,
    )?];
    let mut relations = Vec::new();
    components(
        fixture,
        &whole,
        "restored",
        0,
        &mut adapters,
        &mut relations,
    )?;
    vector(fixture, &mut adapters, &mut relations)?;
    let companions = parameters(fixture, &adapters[1..5])?;
    let coverage = ReadingCoverage::new(
        tickets(0, &[0, 1, 2, 3]),
        adapters,
        relations,
        &mut ledger()?,
    )?;
    Ok(Declarations {
        definition,
        coverage,
        companions,
    })
}

/// The restored voltage vector shares the same first-column source entries as its scalar views.
fn vector(
    fixture: &mut Fixture,
    adapters: &mut Vec<ReadingAdapter<usize>>,
    relations: &mut Vec<ReadingRelation>,
) -> TestResult {
    let whole = adapters[0].reading().clone();
    let reading = fixture.define(
        whole.family(),
        "restored_vector",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        whole.entries().to_vec(),
    )?;
    let symbol = fixture.symbol("restored_vector")?;
    let representation = Representation::prove(
        &mut fixture.cas,
        &reading,
        RepresentationDefinition::Defined {
            symbol,
            expansion: AffineMap::identity(2, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    for (original, selected, result) in [
        (0, vec![0, 1], vec![0, 1]),
        (1, vec![0], vec![0]),
        (2, vec![0], vec![1]),
    ] {
        let left = adapters[original].reading().project_selected(
            &mut fixture.cas,
            &selected,
            &mut ledger()?,
        )?;
        let right = reading.project_selected(&mut fixture.cas, &result, &mut ledger()?)?;
        relations.push(ReadingRelation::prove(
            &mut fixture.cas,
            left,
            right,
            &mut ledger()?,
        )?);
    }
    adapters.push(ReadingAdapter::new(
        representation,
        tickets(0, &[0, 1]),
        &mut ledger()?,
    )?);
    Ok(())
}

/// Scalar companion parameters name each restored voltage copy before it receives a numerical value.
fn parameters(
    fixture: &mut Fixture,
    copies: &[ReadingAdapter<usize>],
) -> TestResult<Vec<CompanionAdapter<usize>>> {
    let mut result = Vec::new();
    for (index, target) in copies.iter().enumerate() {
        for (channel, kind, region) in [
            ("total", CompanionKind::DeterministicBound, 1),
            ("error", CompanionKind::NumericalError, 2),
        ] {
            let name = format!("restored_{index}_{channel}");
            let parameters = CompanionParameters::new(
                &fixture.cas,
                &fixture.context,
                &fixture.symbol(&format!("{name}_source"))?,
                CompanionReadingKind::Radius(kind),
                std::slice::from_ref(target.reading()),
                &mut ledger()?,
            )?;
            let symbol = fixture.symbol(&format!("{name}_output"))?;
            let companion = CompanionReading::define(
                &mut fixture.cas,
                &symbol,
                &parameters,
                std::slice::from_ref(target.reading()),
                &mut ledger()?,
            )?;
            result.push(CompanionAdapter::new(
                companion,
                tickets(region, &[u64::try_from(index)?]),
                vec![target.clone()],
                &mut ledger()?,
            )?);
        }
    }
    Ok(result)
}
