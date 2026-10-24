// SPDX-License-Identifier: AGPL-3.0-only
//! Result symbols, signed reconstructions, component equations and own-target companions.

use super::{TestResult, exact, source};
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{
        CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily,
        LcSpectralLaw, LcSpectralResultDefinition, PhysicalReading, ReadingDefinition,
        ReadingRelation, Representation, RepresentationDefinition,
        algebra::{
            Handle, Rational,
            domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
            readings::{AffineMap, EntryIndex, SelectionSpec},
            syntax::{
                AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Extent, Field,
                Operator, Sort,
            },
        },
    },
};
use nrz_matrix::{CompanionAdapter, EntryTicket, ReadingAdapter, ReadingCoverage};

pub(crate) struct Inventory {
    pub(crate) definition: LcSpectralResultDefinition,
    pub(crate) coverage: ReadingCoverage<usize>,
    pub(crate) companions: Vec<CompanionAdapter<usize>>,
    pub(crate) work: u64,
    pub(crate) definition_work: u64,
    pub(crate) costs: Costs,
}
/// Each row separates complete declaration work from its final native proof or companion definition.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Costs {
    pub(crate) setup: u64,
    pub(crate) views: [[u64; 2]; 6],
    pub(crate) components: [[u64; 2]; 4],
    pub(crate) overlaps: [[u64; 2]; 6],
    pub(crate) companions: [[u64; 2]; 8],
    pub(crate) adapters: u64,
}
const KEYS: [&str; 6] = [
    "spectral_positive",
    "spectral_negative",
    "spectral_positive_real",
    "spectral_negative_real",
    "spectral_positive_imaginary",
    "spectral_negative_imaginary",
];

/// All output symbols occupy a fresh header before numerical input access.
pub(super) fn inventory() -> Vec<Declaration> {
    let mut result = vec![declaration("spectral_stored", matrix(1))];
    for (index, key) in KEYS.into_iter().enumerate() {
        let field = if index < 2 {
            Field::Complex
        } else {
            Field::Real
        };
        result.push(declaration(key, Sort::Value(AtomicSort::Scalar(field))));
        if index == 2 || index == 3 {
            continue;
        }
        for channel in ["total", "error"] {
            result.push(declaration(&format!("{key}_{channel}_source"), matrix(1)));
            result.push(declaration(
                &format!("{key}_{channel}_output"),
                matrix(if index < 2 { 2 } else { 1 }),
            ));
        }
    }
    result
}
fn declaration(key: &str, sort: Sort) -> Declaration {
    Declaration {
        key: key.into(),
        display_name: key.into(),
        sort,
    }
}
const fn matrix(rows: u32) -> Sort {
    Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(rows),
        columns: Extent::Literal(1),
    })
}
fn symbol(
    source: &mut source::Fixture,
    key: &str,
    sort: Sort,
    budget: &mut Budget,
) -> TestResult<Handle> {
    source::admit(
        &mut source.cas,
        Draft::Expression(ExpressionDraft {
            header: source::reference(&source.header),
            scope: Vec::new(),
            operator: Operator::Free(key.into()),
            sort,
            operands: Vec::new(),
            space: None,
            binding: None,
        }),
        budget,
    )
}

/// A single independent free positive-rate storage coordinate supports every signed view.
pub(super) fn prepare(
    source: &mut source::Fixture,
    law: LcSpectralLaw,
    budget: &mut Budget,
) -> TestResult<Inventory> {
    let before = budget.usage().work;
    let mut costs = Costs::default();
    let context = context(source, budget)?;
    let meaning = exact::meaning("rate", [0, 0, -1, 0, 0, 0, 0], "1", budget)?;
    let stored = symbol(source, "spectral_stored", matrix(1), budget)?;
    let family = CoordinateFamily::new(&source.cas, &context, &stored, vec![meaning], budget)?;
    costs.setup = budget.usage().work - before;
    let mut outputs = Vec::new();
    for (index, cost) in costs.views.iter_mut().enumerate() {
        let start = budget.usage().work;
        let (output, proof) = view(source, &family, index, budget)?;
        outputs.push(output);
        *cost = [budget.usage().work - start, proof];
    }
    let outputs: [Representation; 6] = outputs
        .try_into()
        .map_err(|_| "six spectral result views")?;
    let mut components = Vec::new();
    for (index, (root, component, coordinate)) in [(0, 2, 0), (0, 4, 1), (1, 3, 0), (1, 5, 1)]
        .into_iter()
        .enumerate()
    {
        let start = budget.usage().work;
        let left = outputs[root]
            .reading()
            .project(&mut source.cas, &[coordinate], budget)?;
        let right = outputs[component]
            .reading()
            .project(&mut source.cas, &[0], budget)?;
        let proof_start = budget.usage().work;
        components.push(ReadingRelation::prove(
            &mut source.cas,
            left,
            right,
            budget,
        )?);
        costs.components[index] = [
            budget.usage().work - start,
            budget.usage().work - proof_start,
        ];
    }
    let components = components
        .try_into()
        .map_err(|_| "four component equations")?;
    let definition_start = budget.usage().work;
    let definition =
        LcSpectralResultDefinition::new(&source.cas, law, outputs, components, budget)?;
    let definition_work = budget.usage().work - definition_start;
    let (coverage, companions) = adapters(source, &context, &definition, &mut costs, budget)?;
    Ok(Inventory {
        definition,
        coverage,
        companions,
        work: budget.usage().work - before,
        definition_work,
        costs,
    })
}

/// A checked explicit inverse reconstructs the shared selected coordinate for every nonzero view.
fn view(
    source: &mut source::Fixture,
    family: &CoordinateFamily,
    index: usize,
    budget: &mut Budget,
) -> TestResult<(Representation, u64)> {
    let values: &[i64] = match index {
        0 => &[0, 1],
        1 => &[0, -1],
        2 | 3 => &[0],
        4 => &[1],
        5 => &[-1],
        _ => return Err("view index".into()),
    };
    let field = if index < 2 {
        Field::Complex
    } else {
        Field::Real
    };
    let target = symbol(
        source,
        KEYS[index],
        Sort::Value(AtomicSort::Scalar(field)),
        budget,
    )?;
    let map = affine(1, u32::try_from(values.len())?, values, budget)?;
    let reading = PhysicalReading::define(
        &mut source.cas,
        family,
        ReadingDefinition {
            symbol: target,
            selection: SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
            map,
            entries: vec![family.entries()[0].clone(); values.len()],
        },
        budget,
    )?;
    let definition = if index == 2 || index == 3 {
        RepresentationDefinition::Exact
    } else {
        let inverse = if index == 0 {
            &[0, 1][..]
        } else if index == 1 {
            &[0, -1][..]
        } else if index == 4 {
            &[1][..]
        } else {
            &[-1][..]
        };
        let reconstruction = symbol(
            source,
            KEYS[if index < 2 { index + 4 } else { index }],
            Sort::Value(AtomicSort::Scalar(Field::Real)),
            budget,
        )?;
        RepresentationDefinition::Defined {
            symbol: reconstruction,
            expansion: affine(u32::try_from(values.len())?, 1, inverse, budget)?,
        }
    };
    let before = budget.usage().work;
    let result = Representation::prove(&mut source.cas, &reading, definition, budget)?;
    Ok((result, budget.usage().work - before))
}
fn affine(inputs: u32, outputs: u32, values: &[i64], budget: &mut Budget) -> TestResult<AffineMap> {
    let coefficients = values
        .iter()
        .map(|value| Rational::from_ratio(*value, 1, budget))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AffineMap::new(
        inputs,
        outputs,
        coefficients,
        vec![Rational::zero(); usize::try_from(outputs)?],
        budget,
    )?)
}

/// Every intersecting value view has its own checked selected-coordinate relation.
fn adapters(
    source: &mut source::Fixture,
    context: &Handle,
    definition: &LcSpectralResultDefinition,
    costs: &mut Costs,
    budget: &mut Budget,
) -> TestResult<(ReadingCoverage<usize>, Vec<CompanionAdapter<usize>>)> {
    let start = budget.usage().work;
    let entry = EntryTicket {
        region: 0,
        index: 0,
    };
    let mut adapters = Vec::new();
    for (index, representation) in definition.outputs().iter().enumerate() {
        let entries = if index == 2 || index == 3 {
            Vec::new()
        } else {
            vec![entry.clone()]
        };
        adapters.push(ReadingAdapter::new(
            representation.clone(),
            entries,
            budget,
        )?);
    }
    let mut overlaps = Vec::new();
    for (position, index) in [0, 1, 4, 5].into_iter().enumerate() {
        for other in &[0, 1, 4, 5][position + 1..] {
            let before = budget.usage().work;
            let left = adapters[index]
                .reading()
                .project_selected(&mut source.cas, &[0], budget)?;
            let right =
                adapters[*other]
                    .reading()
                    .project_selected(&mut source.cas, &[0], budget)?;
            let proof_start = budget.usage().work;
            overlaps.push(ReadingRelation::prove(
                &mut source.cas,
                left,
                right,
                budget,
            )?);
            costs.overlaps[overlaps.len() - 1] = [
                budget.usage().work - before,
                budget.usage().work - proof_start,
            ];
        }
    }
    let mut companions = Vec::new();
    for index in [0, 1, 4, 5] {
        parameters(
            source,
            context,
            index,
            &adapters[index],
            &mut companions,
            costs,
            budget,
        )?;
    }
    let coverage = ReadingCoverage::new(vec![entry], adapters, overlaps, budget)?;
    let relations = costs
        .overlaps
        .iter()
        .chain(&costs.companions)
        .map(|row| row[0])
        .sum::<u64>();
    costs.adapters = budget.usage().work - start - relations;
    Ok((coverage, companions))
}

/// Complex roots have two logical radius rows; the exact zero row receives literal zero.
fn parameters(
    source: &mut source::Fixture,
    context: &Handle,
    index: usize,
    target: &ReadingAdapter<usize>,
    result: &mut Vec<CompanionAdapter<usize>>,
    costs: &mut Costs,
    budget: &mut Budget,
) -> TestResult {
    let prefix = KEYS[index];
    let (offset, count) = match index {
        0 => (0, 2),
        1 => (2, 2),
        4 => (4, 1),
        5 => (5, 1),
        _ => return Err("parameter target".into()),
    };
    for (channel, (name, kind)) in [
        ("total", CompanionKind::DeterministicBound),
        ("error", CompanionKind::NumericalError),
    ]
    .into_iter()
    .enumerate()
    {
        let before = budget.usage().work;
        let input = symbol(
            source,
            &format!("{prefix}_{name}_source"),
            matrix(1),
            budget,
        )?;
        let parameters = CompanionParameters::new(
            &source.cas,
            context,
            &input,
            CompanionReadingKind::Radius(kind),
            std::slice::from_ref(target.reading()),
            budget,
        )?;
        let output = symbol(
            source,
            &format!("{prefix}_{name}_output"),
            matrix(count),
            budget,
        )?;
        let proof_start = budget.usage().work;
        let reading = CompanionReading::define(
            &mut source.cas,
            &output,
            &parameters,
            std::slice::from_ref(target.reading()),
            budget,
        )?;
        let proof = budget.usage().work - proof_start;
        let entries = (offset..offset + count)
            .map(|index| EntryTicket {
                region: channel + 1,
                index: u64::from(index),
            })
            .collect();
        result.push(CompanionAdapter::new(
            reading,
            entries,
            vec![target.clone()],
            budget,
        )?);
        costs.companions[result.len() - 1] = [budget.usage().work - before, proof];
    }
    Ok(())
}

/// Immutable physical proof headers remain small; output declarations have their own unrestricted root.
fn context(source: &mut source::Fixture, budget: &mut Budget) -> TestResult<Handle> {
    source.header = source::admit(
        &mut source.cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations: inventory(),
            ..Environment::default()
        })),
        budget,
    )?;
    let region = source::admit(
        &mut source.cas,
        Draft::Region(RegionDraft {
            header: source::reference(&source.header),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        budget,
    )?;
    source::admit(
        &mut source.cas,
        Draft::Context(ContextDraft::Root {
            header: source::reference(&source.header),
            admissible: source::reference(&region),
            assumptions: Vec::new(),
        }),
        budget,
    )
}
