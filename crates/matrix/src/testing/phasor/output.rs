// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh result coordinates, vector/scalar views and parameter declarations precede numerical input.

use super::{
    AffineMap, AtomicSort, CompanionAdapter, CompanionKind, CompanionParameters, CompanionReading,
    CompanionReadingKind, CoordinateFamily, Field, Fixture, PhasorBlockCorrespondence,
    PhysicalReading, ReadingAdapter, ReadingCoverage, ReadingRelation, Representation,
    RepresentationDefinition, Scalar, SelectionSpec, Sort, TestResult, components, ledger, map,
    matrix, meaning, real, tickets,
};
use crate::test_api::{PhasorProduct, PhasorPublication};
use nrz_bindings::{BinaryOp, ExponentPolicy, Registry, readings::PhasorResultDefinition};

/// Complete exact result inventory, before any result has storage or values.
pub(crate) struct Declarations {
    /// The actual native product and its fresh result interpretation.
    pub(crate) definition: PhasorResultDefinition,
    /// Whole phasor, four selected copies and a component vector.
    pub(crate) coverage: ReadingCoverage<usize>,
    /// Each stored copy has its own total and numerical-error output parameter.
    pub(crate) companions: Vec<CompanionAdapter<usize>>,
}

/// Add every original result and output-parameter declaration to the shared header.
pub(crate) fn declare(declarations: &mut Vec<(String, Sort)>) {
    declarations.extend([
        ("output_root".to_owned(), matrix(2, 1)),
        ("output_block".to_owned(), matrix(2, 2)),
        (
            "output".to_owned(),
            Sort::Value(AtomicSort::Scalar(Field::Complex)),
        ),
        ("output_vector".to_owned(), matrix(2, 1)),
    ]);
    for index in 0..4 {
        declarations.push((format!("output_{index}"), real()));
        for channel in ["total", "error"] {
            for suffix in ["source", "output"] {
                declarations.push((format!("output_{index}_{channel}_{suffix}"), matrix(1, 1)));
            }
        }
    }
}

/// A named physical product rule checks the new coherent output kind explicitly.
pub(super) fn prepare(
    fixture: &mut Fixture,
    law: &PhasorBlockCorrespondence,
    budget: &mut nrz_bindings::Budget,
) -> TestResult<Declarations> {
    let entry = meaning("coordinate-product", [0; 7])?;
    let root = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("output_root")?,
        vec![entry.clone(); 2],
        &mut ledger()?,
    )?;
    let symbol = fixture.symbol("output_block")?;
    let family = root.expand(
        &mut fixture.cas,
        &symbol,
        &map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0; 4])?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let whole = fixture.define(
        &family,
        "output",
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0; 2])?,
        vec![entry.clone(); 2],
    )?;
    let representation = Representation::prove(
        &mut fixture.cas,
        &whole,
        RepresentationDefinition::ComplexBlock(symbol),
        &mut ledger()?,
    )?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    let first = law.operands()[0].reading().entries()[0]
        .descriptor()
        .kind()
        .clone();
    let second = law.operands()[1].reading().entries()[0]
        .descriptor()
        .kind()
        .clone();
    registry.register_binary(
        BinaryOp::Multiply,
        first,
        second,
        entry.descriptor().kind().clone(),
    )?;
    let definition =
        PhasorResultDefinition::new(law.clone(), representation.clone(), &registry, budget)?;
    let mut adapters = vec![ReadingAdapter::new(
        representation,
        tickets(0, &[0, 1, 2, 3]),
        &mut ledger()?,
    )?];
    let mut relations = Vec::new();
    components(fixture, &whole, "output", 0, &mut adapters, &mut relations)?;
    vector(fixture, &whole, &mut adapters, &mut relations)?;
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

/// The two-coordinate current-style vector shares the actual first-column values.
pub(crate) fn vector(
    fixture: &mut Fixture,
    whole: &PhysicalReading,
    adapters: &mut Vec<ReadingAdapter<usize>>,
    relations: &mut Vec<ReadingRelation>,
) -> TestResult {
    let reading = fixture.define(
        whole.family(),
        "output_vector",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        whole.entries().to_vec(),
    )?;
    let symbol = fixture.symbol("output_vector")?;
    let representation = Representation::prove(
        &mut fixture.cas,
        &reading,
        RepresentationDefinition::Defined {
            symbol,
            expansion: AffineMap::identity(2, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    let left = whole.project_selected(&mut fixture.cas, &[0, 1], &mut ledger()?)?;
    let right = reading.project_selected(&mut fixture.cas, &[0, 1], &mut ledger()?)?;
    relations.push(ReadingRelation::prove(
        &mut fixture.cas,
        left,
        right,
        &mut ledger()?,
    )?);
    for (index, copy) in adapters[1..3].iter().enumerate() {
        let left =
            reading.project_selected(&mut fixture.cas, &[u32::try_from(index)?], &mut ledger()?)?;
        let right = copy
            .reading()
            .project_selected(&mut fixture.cas, &[0], &mut ledger()?)?;
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

/// Every radius region and copy index is fixed before numerical values are produced.
pub(crate) fn parameters(
    fixture: &mut Fixture,
    copies: &[ReadingAdapter<usize>],
) -> TestResult<Vec<CompanionAdapter<usize>>> {
    let mut result = Vec::new();
    for (index, target) in copies.iter().enumerate() {
        for (channel, kind, region) in [
            ("total", CompanionKind::DeterministicBound, 1),
            ("error", CompanionKind::NumericalError, 2),
        ] {
            let name = format!("output_{index}_{channel}");
            let source = fixture.symbol(&format!("{name}_source"))?;
            let parameters = CompanionParameters::new(
                &fixture.cas,
                &fixture.context,
                &source,
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

/// A missing vector is a valid ordinary coverage but an incomplete phasor result publication.
pub(crate) fn missing_vector<P: Scalar>(
    product: PhasorProduct<P>,
    declarations: &Declarations,
) -> TestResult<PhasorPublication<P>> {
    let coverage = ReadingCoverage::new(
        declarations.coverage.entries().to_vec(),
        declarations.coverage.readings()[..5].to_vec(),
        declarations.coverage.overlaps()[..4].to_vec(),
        &mut ledger()?,
    )?;
    Ok(PhasorPublication::new(
        product,
        declarations.definition.clone(),
        coverage,
        declarations.companions.clone(),
        &mut ledger()?,
    )?)
}
