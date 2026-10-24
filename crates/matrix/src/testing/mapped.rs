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
pub(crate) struct Case<P: Scalar> {
    /// Own all live source parameters and target entries.
    pub(crate) heap: Heap,
    /// Exact error transport retains the output companion and both original parameter symbols.
    pub(crate) exact: Rc<ParameterErrorReading>,
    /// All checked reading bindings in whole, left, right, parameters and errors order.
    pub(crate) bindings: Vec<ReadingBinding<P>>,
}

impl<P: Scalar> Case<P> {
    /// Build a fresh metadata plan without reading any numerical source.
    pub(crate) fn execution(
        &self,
        budget: &mut nrz_bindings::Budget,
    ) -> TestResult<CompanionExecution<P>> {
        let targets = self
            .exact
            .target()
            .targets()
            .iter()
            .map(|target| {
                self.bindings
                    .iter()
                    .find(|binding| binding.reading() == target)
                    .cloned()
                    .ok_or_else(|| "companion target binding".into())
            })
            .collect::<TestResult<Vec<_>>>()?;
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
pub(crate) fn case<P: Scalar + 'static>(covariance: bool) -> TestResult<Case<P>> {
    from_regions(covariance, &regions(covariance)?, false, false)
}

/// Invalid entries include zero-weight source parameters that numerical execution must still inspect.
pub(crate) fn invalid<P: Scalar + 'static>(covariance: bool, error: bool) -> TestResult<Case<P>> {
    let mut payloads = regions(covariance)?;
    let region = if error { 2 } else { 1 };
    payloads[region].values[3] = -P::ONE;
    from_regions(covariance, &payloads, false, false)
}

/// Admit one complete source/target inventory with independently supplied literal source arrays.
fn from_regions<P: Scalar + 'static>(
    covariance: bool,
    payloads: &[ReadingRegion<P>],
    attached: bool,
    right_radius: bool,
) -> TestResult<Case<P>> {
    let mut fixture = declarations(covariance)?;
    let (whole, left, right) = targets(&mut fixture)?;
    let ordered = if right_radius {
        [&right, &left]
    } else {
        [&left, &right]
    };
    let (exact, parameter_adapter, error_adapter) = parameters(&mut fixture, ordered, covariance)?;
    let companions = if attached {
        let targets = if covariance {
            vec![left.clone(), right.clone()]
        } else if right_radius {
            vec![right.clone()]
        } else {
            vec![left.clone()]
        };
        vec![nrz_matrix::CompanionAdapter::new(
            exact.target().clone(),
            tickets(3, u64::from(exact.target().map().outputs())),
            targets,
            &mut ledger()?,
        )?]
    } else {
        Vec::new()
    };
    let adapters = vec![whole, left, right, parameter_adapter, error_adapter];
    let relations = overlaps(&mut fixture, &adapters[..3])?;
    let universe = (0..3).flat_map(|region| tickets(region, 4)).collect();
    let coverage = ReadingCoverage::new(universe, adapters, relations, &mut ledger()?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = heap.import_readings(
        Rc::new(fixture.cas),
        coverage,
        companions,
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
    parameters_named(
        fixture,
        targets,
        kind,
        ["parameters", "errors", "output", "output_error"],
        [1, 2],
        covariance,
    )
}

/// Channel names remain one declared inventory while source units and maps use the same checker.
fn parameters_named(
    fixture: &mut Fixture,
    targets: [&ReadingAdapter<usize>; 2],
    kind: CompanionReadingKind,
    names: [&str; 4],
    regions: [usize; 2],
    covariance: bool,
) -> TestResult<(
    ParameterErrorReading,
    ReadingAdapter<usize>,
    ReadingAdapter<usize>,
)> {
    let physical = if covariance {
        vec![targets[0].reading().clone(), targets[1].reading().clone()]
    } else {
        vec![targets[0].reading().clone()]
    };
    let parameters = CompanionParameters::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol(names[0])?,
        kind,
        &physical,
        &mut ledger()?,
    )?;
    let output = fixture.symbol(names[2])?;
    let companion = CompanionReading::define(
        &mut fixture.cas,
        &output,
        &parameters,
        &physical,
        &mut ledger()?,
    )?;
    let output_error = fixture.symbol(names[3])?;
    let errors = fixture.symbol(names[1])?;
    let exact = ParameterErrorReading::define(
        &mut fixture.cas,
        &output_error,
        &errors,
        &companion,
        &mut ledger()?,
    )?;
    let parameter_adapter = source(
        fixture,
        names[0],
        parameters.units(),
        regions[0],
        covariance,
    )?;
    let numerical_units = error_units(parameters.units())?;
    let error_adapter = source(fixture, names[1], &numerical_units, regions[1], covariance)?;
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

/// The same source-law fixture also declares independently stored output companion coordinates.
pub(crate) fn attached<P: Scalar + 'static>(covariance: bool) -> TestResult<Case<P>> {
    from_regions(covariance, &attached_regions(covariance)?, true, false)
}

/// Stored output parameters are distinct coordinates, not another source map application.
fn attached_regions<P: Scalar>(covariance: bool) -> TestResult<Vec<ReadingRegion<P>>> {
    let mut payloads = regions(covariance)?;
    let values = if covariance {
        vec![
            -P::from_u8(2),
            -P::from_u8(3) / P::from_u8(2),
            P::from_u8(5) / P::from_u8(4),
            P::from_u8(15),
        ]
    } else {
        vec![P::ONE / P::from_u8(2), P::from_u8(3) / P::from_u8(2)]
    };
    payloads.push(ReadingRegion {
        extent: Extent::new(2, if covariance { 2 } else { 1 })?,
        values,
    });
    Ok(payloads)
}

/// Both physical radius channels retain their own original source-parameter errors.
pub(crate) struct ChannelCase {
    /// Complete shared value and parameter storage owner.
    pub(crate) heap: Heap,
    /// Original nonidentity value selection with both declared radius channels.
    pub(crate) source: ReadingBinding<f64>,
    /// Total and numerical-only laws over distinct original parameter sources.
    pub(crate) executions: Vec<CompanionExecution<f64>>,
}

/// Extend the shared complete-block fixture with a distinct numerical-only source channel.
pub(crate) fn channels() -> TestResult<ChannelCase> {
    let mut fixture = Fixture::new(&[
        ("root", matrix(2, 1)),
        ("block", matrix(2, 2)),
        ("whole", Sort::Value(AtomicSort::Scalar(Field::Complex))),
        ("left", matrix(2, 1)),
        ("right", matrix(2, 1)),
        ("parameters", matrix(4, 1)),
        ("errors", matrix(4, 1)),
        ("output", matrix(2, 1)),
        ("output_error", matrix(2, 1)),
        ("numerical", matrix(4, 1)),
        ("numerical_error", matrix(4, 1)),
        ("numerical_output", matrix(2, 1)),
        ("numerical_output_error", matrix(2, 1)),
    ])?;
    let (whole, left, right) = targets(&mut fixture)?;
    let (total, total_source, total_error) = parameters(&mut fixture, [&left, &right], false)?;
    let (error, error_source, error_error) = parameters_named(
        &mut fixture,
        [&left, &right],
        CompanionReadingKind::Radius(CompanionKind::NumericalError),
        [
            "numerical",
            "numerical_error",
            "numerical_output",
            "numerical_output_error",
        ],
        [3, 4],
        false,
    )?;
    let mut companions = Vec::new();
    for (exact, region) in [(&total, 5), (&error, 6)] {
        companions.push(nrz_matrix::CompanionAdapter::new(
            exact.target().clone(),
            tickets(region, 2),
            vec![left.clone()],
            &mut ledger()?,
        )?);
    }
    let adapters = vec![
        whole,
        left,
        right,
        total_source,
        total_error,
        error_source,
        error_error,
    ];
    let relations = overlaps(&mut fixture, &adapters[..3])?;
    let universe = (0..5).flat_map(|region| tickets(region, 4)).collect();
    let coverage = ReadingCoverage::new(universe, adapters, relations, &mut ledger()?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = heap.import_readings(
        Rc::new(fixture.cas),
        coverage,
        companions,
        &channel_payloads()?,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    )?;
    let bindings = imported.bindings();
    let mut executions = Vec::new();
    for (exact, index) in [(total, 3), (error, 5)] {
        executions.push(CompanionExecution::new(
            Rc::new(exact),
            [bindings[index].clone(), bindings[index + 1].clone()],
            vec![bindings[1].clone()],
            LiteralLimits::default(),
            &mut ledger()?,
        )?);
    }
    Ok(ChannelCase {
        heap,
        source: bindings[1].clone(),
        executions,
    })
}

/// Distinct dyadic source radii make accidental total/error addition or substitution visible.
fn channel_payloads() -> TestResult<Vec<ReadingRegion<f64>>> {
    let mut payloads = regions(false)?;
    for values in [vec![0.0625, 0.125, 0.1875, 0.25], vec![0.015_625; 4]] {
        payloads.push(ReadingRegion {
            extent: Extent::new(4, 1)?,
            values,
        });
    }
    for values in [vec![0.5, 1.5], vec![0.125, 0.375]] {
        payloads.push(ReadingRegion {
            extent: Extent::new(2, 1)?,
            values,
        });
    }
    Ok(payloads)
}

/// The negative selected map uses absolute radius factors for the original block coordinates.
pub(crate) fn negative_radius() -> TestResult<Case<f64>> {
    let mut payloads = regions(false)?;
    payloads.push(ReadingRegion {
        extent: Extent::new(2, 1)?,
        values: vec![4.0, 3.75],
    });
    from_regions(false, &payloads, true, true)
}

/// A different declared output parameter cannot override its original source enclosure.
pub(crate) fn inconsistent(covariance: bool) -> TestResult<Case<f64>> {
    let mut payloads = attached_regions(covariance)?;
    payloads[3].values[0] += 8.0;
    from_regions(covariance, &payloads, true, false)
}
