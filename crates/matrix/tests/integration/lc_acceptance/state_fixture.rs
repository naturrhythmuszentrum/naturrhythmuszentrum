// SPDX-License-Identifier: AGPL-3.0-only
//! Public mixed-state declarations keep shared references, selected coordinates and complete coverage.

use crate::reading_testing::{Fixture, TestResult, ledger, matrix, real};
use nrz_bindings::{
    CompanionKind,
    readings::{
        CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily,
        CoordinateNormalization, EntryMeaning, PhysicalReading, ReadingRelation, Representation,
        RepresentationDefinition,
        algebra::{
            Cas,
            readings::{AffineMap, EntryIndex, SelectionSpec},
        },
    },
};
use nrz_matrix::{
    CompanionAdapter, CoordinateContract, DependencyPolicy, EntryTicket, Extent, Heap,
    ReadingAdapter, ReadingCoverage, ReadingImport, ReadingRegion,
};
use nrz_scalar::LiteralLimits;
use std::rc::Rc;

/// Every exact value, overlap and radius declaration exists before numerical input access.
pub(super) struct Plan {
    pub(super) graph: Rc<Cas>,
    pub(super) coverage: ReadingCoverage<usize>,
    pub(super) companions: Vec<CompanionAdapter<usize>>,
    pub(super) contract: CoordinateContract,
}

/// The state takes the spectral law's original voltage/current reference identities.
pub(super) fn plan(entries: &[EntryMeaning]) -> TestResult<Plan> {
    plan_with_budget(entries, &mut ledger()?)
}

/// Relation proofs, frame construction and lowering share a caller ledger; declarations are separate.
pub(super) fn plan_with_budget(
    entries: &[EntryMeaning],
    resources: &mut nrz_bindings::Budget,
) -> TestResult<Plan> {
    let mut case = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("v", real()),
        ("i", real()),
        ("p", matrix(2, 1)),
        ("r", matrix(2, 1)),
        ("e", matrix(2, 1)),
        ("q", matrix(2, 1)),
    ])?;
    let family = CoordinateFamily::new(
        &case.cas,
        &case.context,
        &case.symbol("z")?,
        entries.to_vec(),
        &mut ledger()?,
    )?;
    let whole = case.define(
        &family,
        "x",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        entries.to_vec(),
    )?;
    let voltage = component(&mut case, &whole, "v", 0)?;
    let current = component(&mut case, &whole, "i", 1)?;
    let readings = vec![
        adapter(&mut case, &whole, "z", &[0, 1])?,
        adapter(&mut case, &voltage, "v", &[0])?,
        adapter(&mut case, &current, "i", &[1])?,
    ];
    let first = relation(&mut case, &whole, &voltage, 0, resources)?;
    let second = relation(&mut case, &whole, &current, 1, resources)?;
    let overlaps = vec![first, second];
    let companions = companions(&mut case, &readings[0])?;
    let coverage = ReadingCoverage::new(tickets(0, &[0, 1]), readings, overlaps, &mut ledger()?)?;
    let frame = CoordinateNormalization::new(&mut case.cas, &case.context, entries, resources)?;
    let contract =
        CoordinateContract::new(&mut case.cas, &frame, LiteralLimits::default(), resources)?;
    Ok(Plan {
        graph: Rc::new(case.cas),
        coverage,
        companions,
        contract,
    })
}

fn component(
    case: &mut Fixture,
    whole: &PhysicalReading,
    key: &str,
    row: u32,
) -> TestResult<PhysicalReading> {
    case.define(
        whole.family(),
        key,
        SelectionSpec::Entry(EntryIndex { row, column: 0 }),
        AffineMap::identity(1, &mut ledger()?)?,
        vec![whole.entries()[usize::try_from(row)?].clone()],
    )
}

fn adapter(
    case: &mut Fixture,
    reading: &PhysicalReading,
    key: &str,
    indices: &[u64],
) -> TestResult<ReadingAdapter<usize>> {
    let symbol = case.symbol(key)?;
    let representation = Representation::prove(
        &mut case.cas,
        reading,
        RepresentationDefinition::Defined {
            symbol,
            expansion: AffineMap::identity(u32::try_from(indices.len())?, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    Ok(ReadingAdapter::new(
        representation,
        tickets(0, indices),
        &mut ledger()?,
    )?)
}

fn relation(
    case: &mut Fixture,
    whole: &PhysicalReading,
    part: &PhysicalReading,
    index: u32,
    resources: &mut nrz_bindings::Budget,
) -> TestResult<ReadingRelation> {
    let left = whole.project_selected(&mut case.cas, &[index], &mut ledger()?)?;
    let right = part.project_selected(&mut case.cas, &[0], &mut ledger()?)?;
    Ok(ReadingRelation::prove(
        &mut case.cas,
        left,
        right,
        resources,
    )?)
}

fn companions(
    case: &mut Fixture,
    target: &ReadingAdapter<usize>,
) -> TestResult<Vec<CompanionAdapter<usize>>> {
    let mut result = Vec::new();
    for (region, input, output, kind) in [
        (1, "p", "r", CompanionKind::DeterministicBound),
        (2, "e", "q", CompanionKind::NumericalError),
    ] {
        let parameters = CompanionParameters::new(
            &case.cas,
            &case.context,
            &case.symbol(input)?,
            CompanionReadingKind::Radius(kind),
            std::slice::from_ref(target.reading()),
            &mut ledger()?,
        )?;
        let symbol = case.symbol(output)?;
        let reading = CompanionReading::define(
            &mut case.cas,
            &symbol,
            &parameters,
            std::slice::from_ref(target.reading()),
            &mut ledger()?,
        )?;
        result.push(CompanionAdapter::new(
            reading,
            tickets(region, &[0, 1]),
            vec![target.clone()],
            &mut ledger()?,
        )?);
    }
    Ok(result)
}

fn tickets(region: usize, indices: &[u64]) -> Vec<EntryTicket<usize>> {
    indices
        .iter()
        .map(|index| EntryTicket {
            region,
            index: *index,
        })
        .collect()
}

/// Companion radii describe a source-free state family, independent of driven voltage statistics.
pub(super) fn import(
    plan: &Plan,
    heap: &mut Heap,
    policy: DependencyPolicy,
) -> TestResult<ReadingImport<f64>> {
    import_with_budget(plan, heap, policy, &mut ledger()?)
}

/// Measure storage admission independently from exact declarations and numerical restoration.
pub(super) fn import_with_budget(
    plan: &Plan,
    heap: &mut Heap,
    policy: DependencyPolicy,
    budget: &mut nrz_bindings::Budget,
) -> TestResult<ReadingImport<f64>> {
    let extent = Extent::new(2, 1)?;
    let regions = [[3.0, 0.25], [0.5, 0.0625], [0.125, 0.015_625]]
        .into_iter()
        .map(|values| ReadingRegion {
            extent,
            values: values.to_vec(),
        })
        .collect::<Vec<_>>();
    Ok(heap.import_readings(
        Rc::clone(&plan.graph),
        plan.coverage.clone(),
        plan.companions.clone(),
        &regions,
        policy,
        budget,
    )?)
}
