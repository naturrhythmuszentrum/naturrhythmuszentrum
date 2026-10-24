// SPDX-License-Identifier: AGPL-3.0-only
//! The original L/C scalar symbols select their own stored coordinates and radius parameters.

use super::{Fixture, TestResult, source, symbol};
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{
        CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily,
        LcSpectralLaw, PhysicalReading, ReadingDefinition, Representation,
        RepresentationDefinition,
        algebra::{
            Handle,
            readings::{AffineMap, EntryIndex, SelectionSpec},
        },
    },
};
use nrz_matrix::{
    CompanionAdapter, DependencyPolicy, EntryTicket, Extent, Heap, ReadingAdapter, ReadingBinding,
    ReadingCoverage, ReadingRegion,
};
use nrz_scalar::Scalar;
use std::rc::Rc;

type Inventory = (ReadingCoverage<usize>, Vec<CompanionAdapter<usize>>);

/// Original formal symbols and meanings are not reconstructed from numerical schemas.
pub(super) fn declarations(
    source: &mut source::Fixture,
    context: &Handle,
    law: &LcSpectralLaw,
    budget: &mut Budget,
) -> TestResult<Inventory> {
    let mut adapters = Vec::new();
    let mut companions = Vec::new();
    for (index, prefix) in ["inductance", "capacitance"].into_iter().enumerate() {
        let storage = symbol(source, &format!("{prefix}_stored"), true, budget)?;
        let meaning = law.operator().frame().entries()[index].clone();
        let family = CoordinateFamily::new(
            &source.cas,
            context,
            &storage,
            vec![meaning.clone()],
            budget,
        )?;
        let reading = PhysicalReading::define(
            &mut source.cas,
            &family,
            ReadingDefinition {
                symbol: law.operator().inputs()[index].clone(),
                selection: SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
                map: AffineMap::identity(1, budget)?,
                entries: vec![meaning],
            },
            budget,
        )?;
        let representation = Representation::prove(
            &mut source.cas,
            &reading,
            RepresentationDefinition::Defined {
                symbol: law.operator().inputs()[index].clone(),
                expansion: AffineMap::identity(1, budget)?,
            },
            budget,
        )?;
        let adapter = ReadingAdapter::new(
            representation,
            vec![EntryTicket {
                region: index,
                index: 0,
            }],
            budget,
        )?;
        parameters(source, context, prefix, &adapter, &mut companions, budget)?;
        adapters.push(adapter);
    }
    let required = vec![
        EntryTicket {
            region: 0,
            index: 0,
        },
        EntryTicket {
            region: 1,
            index: 0,
        },
    ];
    Ok((
        ReadingCoverage::new(required, adapters, Vec::new(), budget)?,
        companions,
    ))
}

/// Distinct target-bound parameters keep numerical error separate from physical uncertainty.
fn parameters(
    source: &mut source::Fixture,
    context: &Handle,
    prefix: &str,
    target: &ReadingAdapter<usize>,
    result: &mut Vec<CompanionAdapter<usize>>,
    budget: &mut Budget,
) -> TestResult {
    for (channel, kind) in [
        ("total", CompanionKind::DeterministicBound),
        ("error", CompanionKind::NumericalError),
    ] {
        let parameter = symbol(source, &format!("{prefix}_{channel}_source"), true, budget)?;
        let parameters = CompanionParameters::new(
            &source.cas,
            context,
            &parameter,
            CompanionReadingKind::Radius(kind),
            std::slice::from_ref(target.reading()),
            budget,
        )?;
        let output = symbol(source, &format!("{prefix}_{channel}_output"), true, budget)?;
        let reading = CompanionReading::define(
            &mut source.cas,
            &output,
            &parameters,
            std::slice::from_ref(target.reading()),
            budget,
        )?;
        result.push(CompanionAdapter::new(
            reading,
            vec![EntryTicket {
                region: 2,
                index: u64::try_from(result.len())?,
            }],
            vec![target.clone()],
            budget,
        )?);
    }
    Ok(())
}

/// One atomic import provides the complete original source and both companion channels.
pub(super) fn import<P: Scalar + 'static>(
    heap: &mut Heap,
    fixture: &Fixture,
    values: [[P; 3]; 2],
    budget: &mut Budget,
) -> TestResult<[ReadingBinding<P>; 2]> {
    let regions = [
        ReadingRegion {
            extent: Extent::new(1, 1)?,
            values: vec![values[0][0]],
        },
        ReadingRegion {
            extent: Extent::new(1, 1)?,
            values: vec![values[1][0]],
        },
        ReadingRegion {
            extent: Extent::new(4, 1)?,
            values: vec![values[0][1], values[0][2], values[1][1], values[1][2]],
        },
    ];
    let imported = heap.import_readings(
        Rc::clone(&fixture.graph),
        fixture.coverage.clone(),
        fixture.companions.clone(),
        &regions,
        DependencyPolicy::WholeBase,
        budget,
    )?;
    Ok(imported
        .bindings()
        .to_vec()
        .try_into()
        .map_err(|_| "two spectral inputs")?)
}
