// SPDX-License-Identifier: AGPL-3.0-only
//! Public complete block fixtures retain selected copies and explicit original parameter bindings.

use crate::reading_testing::{Fixture, TestResult, ledger, map, matrix, meaning};
use nrz_bindings::readings::algebra::{
    Rational,
    readings::{AffineMap, SelectionSpec},
    syntax::{AtomicSort, Field, Sort},
};
use nrz_bindings::readings::{
    CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily, EntryMeaning,
    ParameterErrorReading, PhysicalReading, ReadingRelation, Representation,
    RepresentationDefinition,
};
use nrz_bindings::{CompanionKind, PairingWitness, ReferenceDefinition, UncertaintyMode};
use nrz_matrix::{
    CompanionExecution, DependencyPolicy, EntryTicket, Extent, Heap, HeapLimits, ReadingAdapter,
    ReadingBinding, ReadingCoverage, ReadingRegion,
};
use nrz_scalar::{LiteralLimits, Scalar};
use std::rc::Rc;

/// Original inputs and independently addressed transformed target bindings.
pub(super) struct Case<P: Scalar> {
    /// Own all live source parameters and target entries.
    pub(super) heap: Heap,
    /// Exact error transport retains the output companion and both original parameter symbols.
    pub(super) exact: Rc<ParameterErrorReading>,
    /// All checked reading bindings in whole, left, right, parameters and errors order.
    pub(super) bindings: Vec<ReadingBinding<P>>,
}

impl<P: Scalar> Case<P> {
    /// Build a fresh metadata plan without reading any numerical source.
    pub(super) fn execution(
        &self,
        budget: &mut nrz_bindings::Budget,
    ) -> TestResult<CompanionExecution<P>> {
        let targets = if self.exact.target().kind() == CompanionReadingKind::Covariance {
            self.bindings[1..3].to_vec()
        } else {
            vec![self.bindings[1].clone()]
        };
        Ok(CompanionExecution::new(
            Rc::clone(&self.exact),
            [self.bindings[3].clone(), self.bindings[4].clone()],
            targets,
            LiteralLimits::default(),
            budget,
        )?)
    }
}

/// Both covariance factors scale and select distinct repeated/negative copies of the same block.
pub(super) fn case<P: Scalar + 'static>(covariance: bool) -> TestResult<Case<P>> {
    from_regions(covariance, &regions(covariance)?)
}

/// Invalid entries include zero-weight source parameters that numerical execution must still inspect.
pub(super) fn invalid<P: Scalar + 'static>(covariance: bool, error: bool) -> TestResult<Case<P>> {
    let mut payloads = regions(covariance)?;
    let region = if error { 2 } else { 1 };
    payloads[region].values[3] = -P::ONE;
    from_regions(covariance, &payloads)
}

/// Admit one complete source/target inventory with independently supplied literal source arrays.
fn from_regions<P: Scalar + 'static>(
    covariance: bool,
    payloads: &[ReadingRegion<P>],
) -> TestResult<Case<P>> {
    let mut fixture = declarations(covariance)?;
    let (whole, left, right) = targets(&mut fixture)?;
    let (exact, parameter_adapter, error_adapter) =
        parameters(&mut fixture, [&left, &right], covariance)?;
    let adapters = vec![whole, left, right, parameter_adapter, error_adapter];
    let relations = overlaps(&mut fixture, &adapters[..3])?;
    let universe = (0..3).flat_map(|region| tickets(region, 4)).collect();
    let coverage = ReadingCoverage::new(universe, adapters, relations, &mut ledger()?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = heap.import_readings(
        Rc::new(fixture.cas),
        coverage,
        Vec::new(),
        payloads,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    )?;
    Ok(Case {
        heap,
        exact: Rc::new(exact),
        bindings: imported.bindings().to_vec(),
    })
}

/// Declare every result and parameter before constructing any reading or numerical input.
fn declarations(covariance: bool) -> TestResult<Fixture> {
    let source_shape = if covariance {
        matrix(2, 2)
    } else {
        matrix(4, 1)
    };
    let output_shape = if covariance {
        matrix(2, 2)
    } else {
        matrix(2, 1)
    };
    Fixture::new(&[
        ("root", matrix(2, 1)),
        ("block", matrix(2, 2)),
        ("whole", Sort::Value(AtomicSort::Scalar(Field::Complex))),
        ("left", matrix(2, 1)),
        ("right", matrix(2, 1)),
        ("parameters", source_shape.clone()),
        ("errors", source_shape),
        ("output", output_shape.clone()),
        ("output_error", output_shape),
    ])
}

/// The exact output law and both source channels form one construction inventory.
fn parameters(
    fixture: &mut Fixture,
    targets: [&ReadingAdapter<usize>; 2],
    covariance: bool,
) -> TestResult<(
    ParameterErrorReading,
    ReadingAdapter<usize>,
    ReadingAdapter<usize>,
)> {
    let kind = if covariance {
        CompanionReadingKind::Covariance
    } else {
        CompanionReadingKind::Radius(CompanionKind::DeterministicBound)
    };
    let physical = if covariance {
        vec![targets[0].reading().clone(), targets[1].reading().clone()]
    } else {
        vec![targets[0].reading().clone()]
    };
    let parameters = CompanionParameters::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("parameters")?,
        kind,
        &physical,
        &mut ledger()?,
    )?;
    let output = fixture.symbol("output")?;
    let companion = CompanionReading::define(
        &mut fixture.cas,
        &output,
        &parameters,
        &physical,
        &mut ledger()?,
    )?;
    let output_error = fixture.symbol("output_error")?;
    let errors = fixture.symbol("errors")?;
    let exact = ParameterErrorReading::define(
        &mut fixture.cas,
        &output_error,
        &errors,
        &companion,
        &mut ledger()?,
    )?;
    let parameter_adapter = source(fixture, "parameters", parameters.units(), 1, covariance)?;
    let numerical_units = error_units(parameters.units())?;
    let error_adapter = source(fixture, "errors", &numerical_units, 2, covariance)?;
    Ok((exact, parameter_adapter, error_adapter))
}

/// One original family expands to the complete block before any readers or numerical data exist.
fn targets(
    fixture: &mut Fixture,
) -> TestResult<(
    ReadingAdapter<usize>,
    ReadingAdapter<usize>,
    ReadingAdapter<usize>,
)> {
    let entry = meaning("impedance", [2, 1, -3, -2, 0, 0, 0])?;
    let root = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("root")?,
        vec![entry.clone(); 2],
        &mut ledger()?,
    )?;
    let block = fixture.symbol("block")?;
    let family = root.expand(
        &mut fixture.cas,
        &block,
        &map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0; 4])?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let selection = SelectionSpec::Block {
        row: 0,
        column: 0,
        rows: 2,
        columns: 2,
    };
    let whole = fixture.define(
        &family,
        "whole",
        selection.clone(),
        map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0; 2])?,
        vec![entry.clone(); 2],
    )?;
    let representation = Representation::prove(
        &mut fixture.cas,
        &whole,
        RepresentationDefinition::ComplexBlock(block),
        &mut ledger()?,
    )?;
    let whole = ReadingAdapter::new(representation, tickets(0, 4), &mut ledger()?)?;
    let left = fixture.define(
        &family,
        "left",
        selection.clone(),
        map(4, 2, &[2, 0, 0, 0, 0, 3, 0, 0], &[0; 2])?,
        vec![entry.clone(); 2],
    )?;
    let right = fixture.define(
        &family,
        "right",
        selection,
        map(4, 2, &[0, 0, 0, -4, 0, 0, -5, 0], &[0; 2])?,
        vec![entry; 2],
    )?;
    let first = inverse(&[1, 0, 0, 1, 0, -1, 1, 0], &[2, 1, 1, 3, 1, 3, 2, 1])?;
    let second = inverse(&[-1, 0, 0, 1, 0, -1, -1, 0], &[4, 1, 1, 5, 1, 5, 4, 1])?;
    Ok((
        whole,
        represented(fixture, &left, first, 0)?,
        represented(fixture, &right, second, 0)?,
    ))
}

/// Complete reconstruction maps retain the selected repeated and negative coordinates.
fn inverse(numerators: &[i64], denominators: &[u64]) -> TestResult<AffineMap> {
    let coefficients = numerators
        .iter()
        .zip(denominators)
        .map(|(&top, &bottom)| Rational::from_ratio(top, bottom, &mut ledger()?))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AffineMap::new(
        2,
        4,
        coefficients,
        vec![Rational::zero(); 4],
        &mut ledger()?,
    )?)
}

/// A source parameter has its own complete physical family and identity reading.
fn source(
    fixture: &mut Fixture,
    key: &str,
    units: &[PairingWitness],
    region: usize,
    covariance: bool,
) -> TestResult<ReadingAdapter<usize>> {
    let entries = units
        .iter()
        .map(|pairing| {
            let descriptor = pairing
                .uncertainty_descriptor()
                .ok_or("parameter descriptor")?
                .clone();
            let reference = ReferenceDefinition::unit(key, &descriptor)?;
            Ok(EntryMeaning::new(
                descriptor,
                reference,
                None,
                &mut ledger()?,
            )?)
        })
        .collect::<TestResult<Vec<_>>>()?;
    let symbol = fixture.symbol(key)?;
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &symbol,
        entries.clone(),
        &mut ledger()?,
    )?;
    let (rows, columns) = if covariance { (2, 2) } else { (4, 1) };
    let reading = fixture.define(
        &family,
        key,
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows,
            columns,
        },
        AffineMap::identity(4, &mut ledger()?)?,
        entries,
    )?;
    represented(
        fixture,
        &reading,
        AffineMap::identity(4, &mut ledger()?)?,
        region,
    )
}

/// Every represented reconstruction has the selected source rectangle, independent of result shape.
fn represented(
    fixture: &mut Fixture,
    reading: &PhysicalReading,
    expansion: AffineMap,
    region: usize,
) -> TestResult<ReadingAdapter<usize>> {
    let symbol = if region == 0 {
        fixture.symbol("block")?
    } else {
        reading.symbol().clone()
    };
    let representation = Representation::prove(
        &mut fixture.cas,
        reading,
        RepresentationDefinition::Defined { symbol, expansion },
        &mut ledger()?,
    )?;
    Ok(ReadingAdapter::new(
        representation,
        tickets(region, 4),
        &mut ledger()?,
    )?)
}

/// Parameter-error units target the original parameter rather than either physical covariance target.
fn error_units(units: &[PairingWitness]) -> TestResult<Vec<PairingWitness>> {
    units
        .iter()
        .map(|pairing| {
            Ok(PairingWitness::uncertainty(
                UncertaintyMode::Radius,
                pairing.uncertainty_descriptor().ok_or("source parameter")?,
                None,
                None,
            )?)
        })
        .collect()
}

/// All intersections refer to the original selected storage coordinates.
fn overlaps(
    fixture: &mut Fixture,
    adapters: &[ReadingAdapter<usize>],
) -> TestResult<Vec<ReadingRelation>> {
    let mut result = Vec::new();
    for (index, left) in adapters.iter().enumerate() {
        for right in &adapters[index + 1..] {
            let left =
                left.reading()
                    .project_selected(&mut fixture.cas, &[0, 1, 2, 3], &mut ledger()?)?;
            let right = right.reading().project_selected(
                &mut fixture.cas,
                &[0, 1, 2, 3],
                &mut ledger()?,
            )?;
            result.push(ReadingRelation::prove(
                &mut fixture.cas,
                left,
                right,
                &mut ledger()?,
            )?);
        }
    }
    Ok(result)
}

/// Every source coordinate has one literal storage position.
fn tickets(region: usize, count: u64) -> Vec<EntryTicket<usize>> {
    (0..count)
        .map(|index| EntryTicket { region, index })
        .collect()
}

/// Dyadic input values avoid finite-width rational-oracle overflow.
fn regions<P: Scalar>(covariance: bool) -> TestResult<Vec<ReadingRegion<P>>> {
    let quarter = P::ONE / P::from_u8(4);
    let values = if covariance {
        vec![
            quarter,
            quarter / P::from_u8(2),
            quarter / P::from_u8(2),
            P::ONE,
        ]
    } else {
        vec![
            quarter,
            quarter * P::from_u8(2),
            quarter * P::from_u8(3),
            P::ONE,
        ]
    };
    let extent = if covariance {
        Extent::new(2, 2)?
    } else {
        Extent::new(4, 1)?
    };
    Ok(vec![
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: vec![P::from_u8(2), P::from_u8(3), -P::from_u8(3), P::from_u8(2)],
        },
        ReadingRegion { extent, values },
        ReadingRegion {
            extent,
            values: vec![P::ONE / P::from_u8(16); 4],
        },
    ])
}
