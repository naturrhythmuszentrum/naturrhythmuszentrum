// SPDX-License-Identifier: AGPL-3.0-only
//! Exact Complex block and explicit scalar copies with distinct numerical-error parameters.

use crate::reading_testing::{Fixture, TestResult, ledger, map, matrix, meaning, real};
use crate::test_api::{
    CompanionAdapter, EntryTicket, Extent, ReadingAdapter, ReadingCoverage, ReadingRegion,
};
use nrz_bindings::readings::algebra::{
    Cas,
    readings::{AffineMap, EntryIndex, SelectionSpec},
    syntax::{AtomicSort, Field, Sort},
};
use nrz_bindings::readings::{
    CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily, ReadingRelation,
    Representation, RepresentationDefinition,
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Immutable complete graph and all output-parameter adapters.
pub(crate) struct Prepared {
    /// Shared exact authority of the expansion and all copy readings.
    pub(crate) graph: Rc<Cas>,
    /// Whole block and both selected scalar copies with their overlap equations.
    pub(crate) coverage: ReadingCoverage<usize>,
    /// Separate total and numerical parameters, with no independence assertion.
    pub(crate) companions: Vec<CompanionAdapter<usize>>,
}

/// Build positive or negative repeated-coordinate readers before supplying any float.
pub(crate) fn plan(negative: bool, errors: bool) -> TestResult<Prepared> {
    let mut declarations = vec![
        ("z", matrix(2, 1)),
        ("block", matrix(2, 2)),
        ("c", Sort::Value(AtomicSort::Scalar(Field::Complex))),
        ("left", real()),
        ("right", real()),
    ];
    declarations.extend(
        [
            "p_left", "r_left", "e_left", "q_left", "p_right", "r_right", "e_right", "q_right",
        ]
        .into_iter()
        .map(|key| (key, matrix(1, 1))),
    );
    let mut case = Fixture::new(&declarations)?;
    let whole = block_reading(&mut case)?;
    let coverage = coverage_plan(&mut case, &whole, negative)?;
    let companions = parameter_adapters(&mut case, &coverage.readings()[1..], errors)?;
    Ok(Prepared {
        graph: Rc::new(case.cas),
        coverage,
        companions,
    })
}

/// The exact two-coordinate expansion retains all signs and repeated entries.
fn block_reading(case: &mut Fixture) -> TestResult<nrz_bindings::readings::PhysicalReading> {
    let entry = meaning("impedance", [2, 1, -3, -2, 0, 0, 0])?;
    let root = CoordinateFamily::new(
        &case.cas,
        &case.context,
        &case.symbol("z")?,
        vec![entry.clone(); 2],
        &mut ledger()?,
    )?;
    let block = case.symbol("block")?;
    let family = root.expand(
        &mut case.cas,
        &block,
        &map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0; 4])?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let whole = case.define(
        &family,
        "c",
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0, 0])?,
        vec![entry; 2],
    )?;
    Ok(whole)
}

/// Every scalar view has its own selected-copy overlap proof against the complete block.
fn coverage_plan(
    case: &mut Fixture,
    whole: &nrz_bindings::readings::PhysicalReading,
    negative: bool,
) -> TestResult<ReadingCoverage<usize>> {
    let block = case.symbol("block")?;
    let entry = whole.entries()[0].clone();
    let representation = Representation::prove(
        &mut case.cas,
        whole,
        RepresentationDefinition::ComplexBlock(block),
        &mut ledger()?,
    )?;
    let mut adapters = vec![ReadingAdapter::new(
        representation,
        tickets(0, &[0, 1, 2, 3]),
        &mut ledger()?,
    )?];
    let mut relations = Vec::new();
    for (key, index) in ["left", "right"]
        .into_iter()
        .zip(if negative { [1, 2] } else { [0, 3] })
    {
        let adapter = component(case, whole, key, index, &entry)?;
        let left = whole.project_selected(&mut case.cas, &[index], &mut ledger()?)?;
        let right = adapter
            .reading()
            .project_selected(&mut case.cas, &[0], &mut ledger()?)?;
        relations.push(ReadingRelation::prove(
            &mut case.cas,
            left,
            right,
            &mut ledger()?,
        )?);
        adapters.push(adapter);
    }
    Ok(ReadingCoverage::new(
        tickets(0, &[0, 1, 2, 3]),
        adapters,
        relations,
        &mut ledger()?,
    )?)
}

/// Each scalar reader names one actual rounded copy and retains the common expanded family.
fn component(
    case: &mut Fixture,
    whole: &nrz_bindings::readings::PhysicalReading,
    key: &str,
    index: u32,
    entry: &nrz_bindings::readings::EntryMeaning,
) -> TestResult<ReadingAdapter<usize>> {
    let reading = case.define(
        whole.family(),
        key,
        SelectionSpec::Entry(EntryIndex {
            row: index % 2,
            column: index / 2,
        }),
        AffineMap::identity(1, &mut ledger()?)?,
        vec![entry.clone()],
    )?;
    let symbol = case.symbol(key)?;
    let representation = Representation::prove(
        &mut case.cas,
        &reading,
        RepresentationDefinition::Defined {
            symbol,
            expansion: AffineMap::identity(1, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    Ok(ReadingAdapter::new(
        representation,
        tickets(0, &[u64::from(index)]),
        &mut ledger()?,
    )?)
}

/// Distinct output parameter symbols keep each copy's total and nominal-error coverage separate.
fn parameter_adapters(
    case: &mut Fixture,
    targets: &[ReadingAdapter<usize>],
    errors: bool,
) -> TestResult<Vec<CompanionAdapter<usize>>> {
    let mut result = Vec::new();
    for (target, names) in targets.iter().zip([
        [("p_left", "r_left"), ("e_left", "q_left")],
        [("p_right", "r_right"), ("e_right", "q_right")],
    ]) {
        for ((parameter, output), kind) in names.into_iter().zip([
            nrz_bindings::CompanionKind::DeterministicBound,
            nrz_bindings::CompanionKind::NumericalError,
        ]) {
            if !errors && kind == nrz_bindings::CompanionKind::NumericalError {
                continue;
            }
            let parameters = CompanionParameters::new(
                &case.cas,
                &case.context,
                &case.symbol(parameter)?,
                CompanionReadingKind::Radius(kind),
                std::slice::from_ref(target.reading()),
                &mut ledger()?,
            )?;
            let symbol = case.symbol(output)?;
            let companion = CompanionReading::define(
                &mut case.cas,
                &symbol,
                &parameters,
                std::slice::from_ref(target.reading()),
                &mut ledger()?,
            )?;
            result.push(CompanionAdapter::new(
                companion,
                tickets(1, &[u64::try_from(result.len())?]),
                vec![target.clone()],
                &mut ledger()?,
            )?);
        }
    }
    Ok(result)
}

/// Storage tickets use literal base indices, never logical result coordinates.
fn tickets(region: usize, indices: &[u64]) -> Vec<EntryTicket<usize>> {
    indices
        .iter()
        .map(|&index| EntryTicket { region, index })
        .collect()
}

/// The quarter-unit declared errors cover an eighth-unit copy discrepancy with strict margin.
pub(crate) fn regions<P: Scalar>(
    negative: bool,
    errors: bool,
) -> TestResult<Vec<ReadingRegion<P>>> {
    let eighth = P::ONE / P::from_u8(8);
    let mut values = vec![P::from_u8(2), P::from_u8(3), -P::from_u8(3), P::from_u8(2)];
    if negative {
        values[2] = values[2] - eighth;
    } else {
        values[3] = values[3] + eighth;
    }
    let radii = if errors {
        vec![
            P::ONE,
            P::ONE / P::from_u8(4),
            P::ONE,
            P::ONE / P::from_u8(4),
        ]
    } else {
        vec![P::ONE; 2]
    };
    Ok(vec![
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values,
        },
        ReadingRegion {
            extent: Extent::new(radii.len(), 1)?,
            values: radii,
        },
    ])
}
