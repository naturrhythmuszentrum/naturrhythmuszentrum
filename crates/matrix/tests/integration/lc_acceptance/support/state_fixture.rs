// SPDX-License-Identifier: AGPL-3.0-only
//! Public mixed-state declarations keep shared references, selected coordinates and complete coverage.

use super::reading_testing::{Fixture, TestResult, ledger, matrix, real};
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
pub(crate) struct Plan {
    /// Exact declaration authority retained by every imported reader.
    pub(crate) graph: Rc<Cas>,
    /// Complete value-entry universe and all selected overlaps.
    pub(crate) coverage: ReadingCoverage<usize>,
    /// Original total and numerical-only radius declarations.
    pub(crate) companions: Vec<CompanionAdapter<usize>>,
    /// Named state references and both exact coordinate images.
    pub(crate) contract: CoordinateContract,
    /// Cumulative relation, frame and image-preparation Work.
    pub(crate) construction: [u64; 4],
}

/// The state takes the spectral law's original voltage/current reference identities.
pub(crate) fn plan(entries: &[EntryMeaning]) -> TestResult<Plan> {
    plan_with_budget(entries, &mut ledger()?)
}

/// Relation proofs, frame construction and exact images share a caller ledger; declarations are separate.
pub(crate) fn plan_with_budget(
    entries: &[EntryMeaning],
    resources: &mut nrz_bindings::Budget,
) -> TestResult<Plan> {
    build(entries, resources, false)
}

/// A mixed cross-covariance fixture retains a second independent state on both ordered axes.
pub(crate) fn covariance_plan(entries: &[EntryMeaning]) -> TestResult<Plan> {
    build(entries, &mut ledger()?, true)
}

/// Extend the original deterministic state declaration only when ordered covariance is requested.
fn build(
    entries: &[EntryMeaning],
    resources: &mut nrz_bindings::Budget,
    covariance: bool,
) -> TestResult<Plan> {
    let mut case = declarations(covariance)?;
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
    let mut readings = vec![
        adapter(&mut case, &whole, "z", &[0, 1])?,
        adapter(&mut case, &voltage, "v", &[0])?,
        adapter(&mut case, &current, "i", &[1])?,
    ];
    let first = relation(&mut case, &whole, &voltage, 0, resources)?;
    let first_work = resources.usage().work;
    let second = relation(&mut case, &whole, &current, 1, resources)?;
    let second_work = resources.usage().work;
    let overlaps = vec![first, second];
    let mut companions = prepare_companions(&mut case, &readings[0])?;
    let mut universe = tickets(0, &[0, 1]);
    if covariance {
        covariance_inventory(&mut case, entries, &mut readings, &mut companions)?;
        universe.extend(tickets(3, &[0, 1]));
    }
    let coverage = ReadingCoverage::new(universe, readings, overlaps, &mut ledger()?)?;
    let frame = CoordinateNormalization::new(&mut case.cas, &case.context, entries, resources)?;
    let frame_work = resources.usage().work;
    let contract =
        CoordinateContract::new(&mut case.cas, &frame, LiteralLimits::default(), resources)?;
    let construction = [first_work, second_work, frame_work, resources.usage().work];
    Ok(Plan {
        graph: Rc::new(case.cas),
        coverage,
        companions,
        contract,
        construction,
    })
}

/// Optional statistical declarations leave the deterministic fixture header unchanged.
fn declarations(covariance: bool) -> TestResult<Fixture> {
    let mut declarations = vec![
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("v", real()),
        ("i", real()),
        ("p", matrix(2, 1)),
        ("r", matrix(2, 1)),
        ("e", matrix(2, 1)),
        ("q", matrix(2, 1)),
    ];
    if covariance {
        declarations.extend([
            ("w", matrix(2, 1)),
            ("y", matrix(2, 1)),
            ("u", matrix(2, 2)),
            ("s", matrix(2, 2)),
            ("t", matrix(2, 2)),
            ("h", matrix(2, 2)),
            ("j", matrix(2, 2)),
            ("k", matrix(2, 2)),
        ]);
    }
    Fixture::new(&declarations)
}

/// Select one original state coordinate without creating an independent source.
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

/// Authenticate every selected storage copy against its exact representation.
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

/// Prove a whole-to-component overlap under the retained state context.
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

/// Declare separate complete total and numerical-only radius maps.
fn prepare_companions(
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

/// Enumerate the exact ordered positions owned by one numerical region.
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
pub(crate) fn import(
    plan: &Plan,
    heap: &mut Heap,
    policy: DependencyPolicy,
) -> TestResult<ReadingImport<f64>> {
    import_with_budget(plan, heap, policy, &mut ledger()?)
}

/// Measure storage admission independently from exact declarations and numerical restoration.
pub(crate) fn import_with_budget(
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

/// Distinct covariance parameters preserve shared and cross-source target order.
fn covariance_inventory(
    case: &mut Fixture,
    entries: &[EntryMeaning],
    readings: &mut Vec<ReadingAdapter<usize>>,
    companions: &mut Vec<CompanionAdapter<usize>>,
) -> TestResult {
    let family = CoordinateFamily::new(
        &case.cas,
        &case.context,
        &case.symbol("w")?,
        entries.to_vec(),
        &mut ledger()?,
    )?;
    let other = case.define(
        &family,
        "y",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        entries.to_vec(),
    )?;
    let symbol = case.symbol("w")?;
    let representation = Representation::prove(
        &mut case.cas,
        &other,
        RepresentationDefinition::Defined {
            symbol,
            expansion: AffineMap::identity(2, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    let other = ReadingAdapter::new(representation, tickets(3, &[0, 1]), &mut ledger()?)?;
    for (region, input, output, targets) in [
        (4, "u", "s", [readings[0].clone(), readings[0].clone()]),
        (5, "t", "h", [readings[0].clone(), other.clone()]),
        (6, "j", "k", [other.clone(), readings[0].clone()]),
    ] {
        let values = targets
            .iter()
            .map(|target| target.reading().clone())
            .collect::<Vec<_>>();
        let parameters = CompanionParameters::new(
            &case.cas,
            &case.context,
            &case.symbol(input)?,
            CompanionReadingKind::Covariance,
            &values,
            &mut ledger()?,
        )?;
        let symbol = case.symbol(output)?;
        let covariance =
            CompanionReading::define(&mut case.cas, &symbol, &parameters, &values, &mut ledger()?)?;
        companions.push(CompanionAdapter::new(
            covariance,
            tickets(region, &[0, 1, 2, 3]),
            targets.to_vec(),
            &mut ledger()?,
        )?);
    }
    readings.push(other);
    Ok(())
}
