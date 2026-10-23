// SPDX-License-Identifier: AGPL-3.0-only
//! Independent mixed-coordinate declarations built through the public physical-reading APIs.

use super::*;
use crate::{CompanionAdapter, EntryTicket, ReadingAdapter, ReadingCoverage, ReadingRegion};
use nrz_bindings::readings::algebra::{
    Rational,
    readings::{AffineMap, SelectionSpec},
};
use nrz_bindings::readings::{
    CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily, EntryMeaning,
    Representation, RepresentationDefinition,
};
use nrz_bindings::{Descriptor, Dimension, Kind, Measure, ReferenceDefinition, Role, Scale};
use std::rc::Rc;

/// Complete exact graph, both metrics and all adapters, prepared before any float is supplied.
pub(super) struct Plan {
    /// Original exact graph and declaration context.
    pub(super) case: Fixture,
    /// Every value entry and its checked reading.
    pub(super) coverage: ReadingCoverage<usize>,
    /// Distinct deterministic and numerical-error parameter outputs.
    pub(super) companions: Vec<CompanionAdapter<usize>>,
    /// Physical state references used by normalization and restoration.
    pub(super) contract: CoordinateContract,
    /// An independently declared metric with doubled positive references.
    pub(super) metric: CoordinateContract,
}

/// Physical voltage and current references are deliberately unequal and non-binary.
fn meanings(doubled: bool) -> TestResult<Vec<EntryMeaning>> {
    let mut entries = Vec::new();
    for (name, dimensions, text) in [
        (
            "voltage",
            [2, 1, -3, -1, 0, 0, 0],
            if doubled { "10" } else { "5" },
        ),
        (
            "current",
            [0, 0, 0, 1, 0, 0, 0],
            if doubled { "1/5" } else { "1/10" },
        ),
    ] {
        let descriptor = Descriptor::new(
            Kind::Named(name.into(), Box::new(Dimension::integers(dimensions)?)),
            Measure::coherent(),
            Role::Difference,
        )?;
        let reference = ReferenceDefinition::new(
            name,
            &descriptor,
            Scale::Exact(Rational::parse(text, &mut ledger()?)?),
        )?;
        entries.push(EntryMeaning::new(
            descriptor,
            reference,
            None,
            &mut ledger()?,
        )?);
    }
    Ok(entries)
}

/// Exact value and companion declarations share one actual selected coordinate family.
pub(super) fn plan() -> TestResult<Plan> {
    build(&meanings(false)?, &meanings(true)?)
}

/// Retain two independently supplied exact reference inventories on one declaration graph.
fn build(entries: &[EntryMeaning], metric_entries: &[EntryMeaning]) -> TestResult<Plan> {
    let mut case = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
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
    let reading = case.define(
        &family,
        "x",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        entries.to_vec(),
    )?;
    let symbol = case.symbol("z")?;
    let representation = Representation::prove(
        &mut case.cas,
        &reading,
        RepresentationDefinition::Defined {
            symbol,
            expansion: AffineMap::identity(2, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    let adapter = ReadingAdapter::new(representation, tickets(0), &mut ledger()?)?;
    let companions = companions(&mut case, &adapter)?;
    let coverage = ReadingCoverage::new(tickets(0), vec![adapter], Vec::new(), &mut ledger()?)?;
    let frame =
        CoordinateNormalization::new(&mut case.cas, &case.context, entries, &mut ledger()?)?;
    let contract = CoordinateContract::new(
        &mut case.cas,
        &frame,
        LiteralLimits::default(),
        &mut ledger()?,
    )?;
    let frame =
        CoordinateNormalization::new(&mut case.cas, &case.context, metric_entries, &mut ledger()?)?;
    let metric = CoordinateContract::new(
        &mut case.cas,
        &frame,
        LiteralLimits::default(),
        &mut ledger()?,
    )?;
    Ok(Plan {
        case,
        coverage,
        companions,
        contract,
        metric,
    })
}

/// Actual parameter outputs name the original value target and its ordered selected copies.
fn companions(
    case: &mut Fixture,
    target: &ReadingAdapter<usize>,
) -> TestResult<Vec<CompanionAdapter<usize>>> {
    let mut result = Vec::new();
    for (region, parameter, output, kind) in [
        (1, "p", "r", nrz_bindings::CompanionKind::DeterministicBound),
        (2, "e", "q", nrz_bindings::CompanionKind::NumericalError),
    ] {
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
            tickets(region),
            vec![target.clone()],
            &mut ledger()?,
        )?);
    }
    Ok(result)
}

/// Explicit complete ordered tickets, without assuming physical homogeneity.
fn tickets(region: usize) -> Vec<EntryTicket<usize>> {
    [0, 1]
        .into_iter()
        .map(|index| EntryTicket { region, index })
        .collect()
}

/// Independent caller-supplied deterministic family and nominal-target numerical error.
pub(super) fn imported<P: Scalar + 'static>(
    bounded: bool,
) -> TestResult<(
    Heap,
    ReadingBinding<P>,
    CoordinateContract,
    CoordinateContract,
)> {
    import(plan()?, bounded, P::from_u8(3))
}

/// Use an affine temperature point beside a current difference without changing the storage recipe.
pub(super) fn point_imported<P: Scalar + 'static>() -> TestResult<(
    Heap,
    ReadingBinding<P>,
    CoordinateContract,
    CoordinateContract,
)> {
    let mut entries = meanings(false)?;
    let point = Descriptor::new(
        Kind::Named(
            "temperature".into(),
            Box::new(Dimension::integers([0, 0, 0, 0, 1, 0, 0])?),
        ),
        Measure::coherent(),
        Role::Point,
    )?;
    let origin = Descriptor::new(
        point.kind().clone(),
        Measure::new(
            "freezing",
            Scale::Exact(Rational::one()),
            Rational::parse("27315/100", &mut ledger()?)?,
        )?,
        Role::Point,
    )?;
    let reference = ReferenceDefinition::new(
        "temperature-reference",
        &point.difference(),
        Scale::Exact(Rational::parse("10", &mut ledger()?)?),
    )?;
    entries[0] = EntryMeaning::new(
        point,
        reference,
        Some(nrz_bindings::OriginDefinition::new("freezing", &origin)?),
        &mut ledger()?,
    )?;
    let prepared = build(&entries, &entries)?;
    import(prepared, true, P::from_u8(200) + P::from_u8(100))
}

/// The declared plan is complete before any supplied center or companion coordinate is inspected.
fn import<P: Scalar + 'static>(
    prepared: Plan,
    bounded: bool,
    first: P,
) -> TestResult<(
    Heap,
    ReadingBinding<P>,
    CoordinateContract,
    CoordinateContract,
)> {
    let extent = crate::Extent::new(2, 1)?;
    let mut regions = vec![ReadingRegion {
        extent,
        values: vec![first, P::ONE / P::from_u8(4)],
    }];
    let companions = if bounded {
        regions.push(ReadingRegion {
            extent,
            values: vec![P::ONE / P::from_u8(2), P::ONE / P::from_u8(16)],
        });
        regions.push(ReadingRegion {
            extent,
            values: vec![P::ONE / P::from_u8(8), P::ONE / P::from_u8(64)],
        });
        prepared.companions
    } else {
        Vec::new()
    };
    let mut heap = Heap::new(crate::HeapLimits::default())?;
    let admitted = heap.import_readings(
        Rc::new(prepared.case.cas),
        prepared.coverage,
        companions,
        &regions,
        crate::DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    let binding = admitted.bindings().first().ok_or("binding")?.clone();
    Ok((heap, binding, prepared.contract, prepared.metric))
}

/// A valid exact degree reference remains available even when this numerical route cannot export pi.
pub(super) fn angle_imported() -> TestResult<(
    Heap,
    ReadingBinding<f64>,
    CoordinateContract,
    CoordinateContract,
)> {
    let mut entries = meanings(false)?;
    let registry =
        nrz_bindings::Registry::with_angle_frequency(nrz_bindings::ExponentPolicy::Rational)?;
    let degree = registry.measure("degree").ok_or("degree")?;
    let reference = ReferenceDefinition::unit("degree-reference", degree)?;
    entries[0] = EntryMeaning::new(degree.coherent(), reference, None, &mut ledger()?)?;
    import(build(&entries, &entries)?, false, 3.0)
}
