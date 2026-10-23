// SPDX-License-Identifier: AGPL-3.0-only
//! Two declared phasors share one source graph and atomic scalar-copy/companion admission.

use crate::reading_testing::{Fixture, TestResult, ledger, map, matrix, meaning, real};
use crate::test_api::{
    CompanionAdapter, DependencyPolicy, EntryTicket, Extent, Heap, HeapLimits, PhasorOperand,
    PhasorProduct, ReadingAdapter, ReadingBinding, ReadingCoverage, ReadingRegion,
};
use nrz_bindings::readings::algebra::{
    readings::{AffineMap, EntryIndex, SelectionSpec},
    syntax::{AtomicSort, Field, Sort},
};
use nrz_bindings::{
    CompanionKind,
    readings::{
        CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily,
        PhasorBlockCorrespondence, PhasorConvention, PhysicalReading, ReadingRelation,
        Representation, RepresentationDefinition,
    },
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Complete admitted numerical family and its predeclared exact product.
pub(crate) struct Case<P: Scalar> {
    /// Own all independently rounded source and parameter regions.
    pub(crate) heap: Heap,
    /// Exact correspondence, prepared before numerical import.
    pub(crate) law: PhasorBlockCorrespondence,
    /// Every reading, in whole then four scalar-copy order for each operand.
    pub(crate) bindings: Vec<ReadingBinding<P>>,
    /// Optional complete output inventory, prepared before source import.
    pub(crate) output: Option<output::Declarations>,
}
#[path = "phasor/output.rs"]
mod output;

impl<P: Scalar> Case<P> {
    /// Bind all predeclared result views and companions to the actual ordered input product.
    pub(crate) fn publication(
        &self,
        budget: &mut nrz_bindings::Budget,
    ) -> TestResult<crate::test_api::PhasorPublication<P>> {
        let declarations = self.output.as_ref().ok_or("output declarations")?;
        Ok(crate::test_api::PhasorPublication::new(
            self.product(budget)?,
            declarations.definition.clone(),
            declarations.coverage.clone(),
            declarations.companions.clone(),
            budget,
        )?)
    }

    /// Refuse an otherwise covered output inventory that omits its required vector view.
    pub(crate) fn missing_vector(&self) -> TestResult<crate::test_api::PhasorPublication<P>> {
        output::missing_vector(
            self.product(&mut ledger()?)?,
            self.output.as_ref().ok_or("output declarations")?,
        )
    }
    /// The complete product plan consumes all original scalar-copy identities.
    pub(crate) fn product(
        &self,
        budget: &mut nrz_bindings::Budget,
    ) -> TestResult<PhasorProduct<P>> {
        let operands = [self.operand(0, budget)?, self.operand(5, budget)?];
        Ok(PhasorProduct::new(self.law.clone(), operands, budget)?)
    }
    /// One complete selected-copy inventory in logical column order.
    pub(crate) fn operand(
        &self,
        start: usize,
        budget: &mut nrz_bindings::Budget,
    ) -> TestResult<PhasorOperand<P>> {
        let copies = self.bindings[start + 1..start + 5]
            .to_vec()
            .try_into()
            .map_err(|_| "four copies")?;
        Ok(PhasorOperand::new(
            self.bindings[start].clone(),
            copies,
            budget,
        )?)
    }
}

/// All component and companion declarations exist before a float enters the heap.
pub(crate) fn case<P: Scalar + 'static>(rounded: bool) -> TestResult<Case<P>> {
    case_with_budget(rounded, &mut ledger()?)
}

/// Correspondence and complete heap admission share one unchanged caller policy and ledger.
pub(crate) fn case_with_budget<P: Scalar + 'static>(
    rounded: bool,
    budget: &mut nrz_bindings::Budget,
) -> TestResult<Case<P>> {
    build(
        rounded,
        false,
        crate::test_api::Coverage::DeclaredBounds,
        budget,
    )
}

/// Prepare a fresh complete result slot before importing either numerical operand.
pub(crate) fn publication_case<P: Scalar + 'static>(
    rounded: bool,
    budget: &mut nrz_bindings::Budget,
) -> TestResult<Case<P>> {
    build(
        rounded,
        true,
        crate::test_api::Coverage::DeclaredBounds,
        budget,
    )
}

/// Represented exact coordinates retain their original assurance after derived radius publication.
pub(crate) fn represented_case<P: Scalar + 'static>(
    budget: &mut nrz_bindings::Budget,
) -> TestResult<Case<P>> {
    build(
        false,
        true,
        crate::test_api::Coverage::RepresentedInputs,
        budget,
    )
}

/// Construct all immutable definitions before the one original input import.
fn build<P: Scalar + 'static>(
    rounded: bool,
    publish: bool,
    assurance: crate::test_api::Coverage,
    budget: &mut nrz_bindings::Budget,
) -> TestResult<Case<P>> {
    let mut fixture = declarations(publish)?;
    let (adapters, relations) = readings(&mut fixture)?;
    let law = PhasorBlockCorrespondence::prove(
        &mut fixture.cas,
        [
            adapters[0].representation().clone(),
            adapters[5].representation().clone(),
        ],
        PhasorConvention::PositiveTimePassive,
        budget,
    )?;
    let companion_adapters = if assurance == crate::test_api::Coverage::DeclaredBounds {
        companions(&mut fixture, &adapters)?
    } else {
        Vec::new()
    };
    let universe = (0..2)
        .flat_map(|region| tickets(region, &[0, 1, 2, 3]))
        .collect();
    let coverage = ReadingCoverage::new(universe, adapters, relations, &mut ledger()?)?;
    let output = if publish {
        Some(output::prepare(&mut fixture, &law, budget)?)
    } else {
        None
    };
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut payloads = regions(rounded)?;
    if assurance == crate::test_api::Coverage::RepresentedInputs {
        payloads.truncate(2);
    }
    let imported = heap.import_readings(
        Rc::new(fixture.cas),
        coverage,
        companion_adapters,
        &payloads,
        DependencyPolicy::WholeBase,
        budget,
    )?;
    Ok(Case {
        heap,
        law,
        bindings: imported.bindings().to_vec(),
        output,
    })
}

/// Separate declarations for every whole operand, scalar copy and stored output parameter.
fn declarations(publish: bool) -> TestResult<Fixture> {
    let mut declarations = vec![("root".to_owned(), matrix(4, 1))];
    for prefix in ["left", "right"] {
        declarations.push((
            prefix.to_owned(),
            Sort::Value(AtomicSort::Scalar(Field::Complex)),
        ));
        declarations.push((format!("{prefix}_block"), matrix(2, 2)));
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
    if publish {
        output::declare(&mut declarations);
    }
    let borrowed = declarations
        .iter()
        .map(|(key, sort)| (key.as_str(), sort.clone()))
        .collect::<Vec<_>>();
    Fixture::new(&borrowed)
}

/// Build both expansions from the same root without declaring independent repeated coordinates.
fn readings(
    fixture: &mut Fixture,
) -> TestResult<(Vec<ReadingAdapter<usize>>, Vec<ReadingRelation>)> {
    let entry = meaning("coordinate", [0; 7])?;
    let root = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("root")?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let mut adapters = Vec::new();
    let mut relations = Vec::new();
    for (region, prefix) in ["left", "right"].into_iter().enumerate() {
        let coefficients = if region == 0 {
            [1, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0, 0, 1, 0, 0, 0]
        } else {
            [0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0, 0, 1, 0]
        };
        let block = fixture.symbol(&format!("{prefix}_block"))?;
        let family = root.expand(
            &mut fixture.cas,
            &block,
            &map(4, 4, &coefficients, &[0; 4])?,
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
            vec![entry.clone(); 2],
        )?;
        let representation = Representation::prove(
            &mut fixture.cas,
            &whole,
            RepresentationDefinition::ComplexBlock(block),
            &mut ledger()?,
        )?;
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
    Ok((adapters, relations))
}

/// Each identity scalar view has its own proof against the specified selected storage entry.
fn components(
    fixture: &mut Fixture,
    whole: &PhysicalReading,
    prefix: &str,
    region: usize,
    adapters: &mut Vec<ReadingAdapter<usize>>,
    relations: &mut Vec<ReadingRelation>,
) -> TestResult {
    for index in 0..4 {
        let key = format!("{prefix}_{index}");
        let reading = fixture.define(
            whole.family(),
            &key,
            SelectionSpec::Entry(EntryIndex {
                row: index % 2,
                column: index / 2,
            }),
            AffineMap::identity(1, &mut ledger()?)?,
            vec![whole.entries()[0].clone()],
        )?;
        let symbol = fixture.symbol(&key)?;
        let representation = Representation::prove(
            &mut fixture.cas,
            &reading,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(1, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        let left = whole.project_selected(&mut fixture.cas, &[index], &mut ledger()?)?;
        let right = reading.project_selected(&mut fixture.cas, &[0], &mut ledger()?)?;
        relations.push(ReadingRelation::prove(
            &mut fixture.cas,
            left,
            right,
            &mut ledger()?,
        )?);
        adapters.push(ReadingAdapter::new(
            representation,
            tickets(region, &[u64::from(index)]),
            &mut ledger()?,
        )?);
    }
    Ok(())
}

/// Identity output parameters are distinct per copy and coverage channel.
fn companions(
    fixture: &mut Fixture,
    adapters: &[ReadingAdapter<usize>],
) -> TestResult<Vec<CompanionAdapter<usize>>> {
    let mut result = Vec::new();
    for (start, prefix) in [(0, "left"), (5, "right")] {
        for (index, target) in adapters[start + 1..start + 5].iter().enumerate() {
            for (channel, kind) in [
                ("total", CompanionKind::DeterministicBound),
                ("error", CompanionKind::NumericalError),
            ] {
                let name = format!("{prefix}_{index}_{channel}");
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
                let parameter = u64::try_from(result.len())?;
                result.push(CompanionAdapter::new(
                    companion,
                    tickets(2, &[parameter]),
                    vec![target.clone()],
                    &mut ledger()?,
                )?);
            }
        }
    }
    Ok(result)
}

/// Literal storage positions remain independent from logical result component indices.
fn tickets(region: usize, indices: &[u64]) -> Vec<EntryTicket<usize>> {
    indices
        .iter()
        .map(|&index| EntryTicket { region, index })
        .collect()
}

/// Different repeated and negative copies are covered by their separate quarter-unit errors.
fn regions<P: Scalar>(rounded: bool) -> TestResult<Vec<ReadingRegion<P>>> {
    let mut left = vec![P::from_u8(2), P::from_u8(3), -P::from_u8(3), P::from_u8(2)];
    let mut right = vec![P::from_u8(5), P::from_u8(7), -P::from_u8(7), P::from_u8(5)];
    if rounded {
        left[3] = left[3] + P::ONE / P::from_u8(8);
        right[2] = right[2] - P::ONE / P::from_u8(8);
    }
    let parameters = (0..8)
        .flat_map(|_| [P::ONE, P::ONE / P::from_u8(4)])
        .collect();
    Ok(vec![
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: left,
        },
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: right,
        },
        ReadingRegion {
            extent: Extent::new(16, 1)?,
            values: parameters,
        },
    ])
}
