// SPDX-License-Identifier: AGPL-3.0-only
//! Two certified current means share one actual original voltage reading and one covariance parameter.

use super::reading_testing::{Fixture, TestResult, ledger, meaning};
use super::{blocks, parameters};
use super::{budget, policy};
use nrz_bindings::{
    BinaryOp, Budget, ExponentPolicy, Registry,
    readings::{
        CoordinateNormalization, ParameterErrorReading, PhasorBlockCorrespondence,
        PhasorConvention, PhasorResidualNorm, PhasorResidualTransfer, PhasorSolveDefinition,
    },
};
use nrz_matrix::{
    CompanionExecution, DependencyPolicy, EntryTicket, Extent, Heap, HeapLimits, PhasorOperand,
    PhasorSolve, PhasorSolvePublication, PublishedPhasorSolve, ReadingBinding, ReadingCoverage,
    ReadingRegion,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};
use std::rc::Rc;

/// A complete statistical request with every exact result and companion declared before execution.
pub(super) struct Case<P: Scalar> {
    /// Cumulative operation Work after equations, covariance laws, metrics and input/publication plans.
    pub(super) construction: [u64; 4],
    /// Numerical owner of the shared source, both means and all parameter versions.
    pub(super) heap: Heap,
    /// Both current publication plans retain the same original voltage operand.
    pub(super) publications: [PhasorSolvePublication<P>; 2],
    /// Original source whole/scalar views, then source variance and parameter error.
    pub(super) bindings: Vec<ReadingBinding<P>>,
    /// Predeclared exact variance/cross/variance error transports.
    pub(super) exact: [Rc<ParameterErrorReading>; 3],
    /// Actual scalar or complete vector target selected before numerical execution.
    target: usize,
}

impl<P: Scalar + 'static> Case<P> {
    /// Publish deterministic mean families before constructing their versioned statistical executor.
    pub(super) fn means(
        &mut self,
        budget: &mut Budget,
    ) -> TestResult<[PublishedPhasorSolve<P>; 2]> {
        let context = ArithmeticContext::new()?;
        Ok([
            self.heap
                .publish_phasor_solve(&self.publications[0], &context, budget)?,
            self.heap
                .publish_phasor_solve(&self.publications[1], &context, budget)?,
        ])
    }
    /// All three exact companion maps read the same original variance/error pair.
    pub(super) fn executions(
        &self,
        means: &[PublishedPhasorSolve<P>; 2],
        budget: &mut Budget,
    ) -> TestResult<[CompanionExecution<P>; 3]> {
        let mut result = Vec::new();
        for (index, left, right) in [(0, 0, 0), (1, 0, 1), (2, 1, 1)] {
            result.push(CompanionExecution::new(
                Rc::clone(&self.exact[index]),
                [self.bindings[15].clone(), self.bindings[16].clone()],
                vec![
                    means[left].readings().bindings()[self.target].clone(),
                    means[right].readings().bindings()[self.target].clone(),
                ],
                LiteralLimits::default(),
                budget,
            )?);
        }
        result
            .try_into()
            .map_err(|_| "three companion executions".into())
    }
}

/// Fixed `-75i,+75i` impedances and one shared real voltage amplitude define both exact affine solves.
pub(super) fn case<P: Scalar + 'static>() -> TestResult<Case<P>> {
    case_with_budget(&mut budget()?)
}

/// One caller ledger covers both equations, all covariance laws and complete source/publication admission.
pub(super) fn case_with_budget<P: Scalar + 'static>(resources: &mut Budget) -> TestResult<Case<P>> {
    build(resources, false, DependencyPolicy::WholeBase)
}

/// Full two-coordinate targets exercise both covariance axes without a scalar statistics facade.
pub(super) fn vector_case<P: Scalar + 'static>() -> TestResult<Case<P>> {
    vector_case_with_budget(&mut budget()?)
}

/// Measure the complete vector-target construction on the caller's cumulative ledger.
pub(super) fn vector_case_with_budget<P: Scalar + 'static>(
    resources: &mut Budget,
) -> TestResult<Case<P>> {
    build(resources, true, DependencyPolicy::WholeBase)
}

/// Scalar and vector requests retain their complete inventories before importing any values.
fn build<P: Scalar + 'static>(
    resources: &mut Budget,
    full: bool,
    dependencies: DependencyPolicy,
) -> TestResult<Case<P>> {
    let mut fixture = declarations(full)?;
    let voltage = meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let impedance = meaning("impedance", [2, 1, -3, -2, 0, 0, 0])?;
    let current = meaning("current", [0, 0, 0, 1, 0, 0, 0])?;
    let mut inputs = [
        blocks::input(&mut fixture, "below", impedance.clone(), Some(-75), 0)?,
        blocks::input(&mut fixture, "above", impedance, Some(75), 1)?,
        blocks::input(&mut fixture, "voltage", voltage, None, 2)?,
    ];
    blocks::cover_reactance(&mut fixture, "below", &mut inputs[0], 0)?;
    blocks::cover_reactance(&mut fixture, "above", &mut inputs[1], 2)?;
    let outputs = [
        blocks::output(&mut fixture, "first", current.clone())?,
        blocks::output(&mut fixture, "second", current)?,
    ];
    let definitions = equations(&mut fixture, &inputs, &outputs, resources)?;
    let equation_work = resources.usage().work;
    let target = if full { 5 } else { 2 };
    let targets = [
        outputs[0].adapters[target].reading().clone(),
        outputs[1].adapters[target].reading().clone(),
    ];
    let (parameters, exact) = parameters::prepare(&mut fixture, &definitions, &targets, resources)?;
    let covariance_work = resources.usage().work;
    let frames = [
        CoordinateNormalization::new(
            &mut fixture.cas,
            &fixture.context,
            definitions[0].voltage().reading().entries(),
            resources,
        )?,
        CoordinateNormalization::new(
            &mut fixture.cas,
            &fixture.context,
            definitions[0].output().reading().entries(),
            resources,
        )?,
    ];
    let metric_work = resources.usage().work;
    let (mut heap, bindings) = import::<P>(fixture, &inputs, &parameters, dependencies, resources)
        .map_err(|error| format!("source admission ({dependencies:?}): {error}"))?;
    let publications = publications(&definitions, &outputs, &bindings, &frames, resources)?;
    heap.compact()?;
    let construction = [
        equation_work,
        covariance_work,
        metric_work,
        resources.usage().work,
    ];
    Ok(Case {
        construction,
        heap,
        publications,
        bindings,
        exact,
        target,
    })
}

/// Every exact symbol is available in the shared graph before any numerical computation.
fn declarations(full: bool) -> TestResult<Fixture> {
    let mut declarations = Vec::new();
    for prefix in ["below", "above", "voltage"] {
        blocks::declare(&mut declarations, prefix, 1, false);
    }
    for prefix in ["first", "second"] {
        blocks::declare(&mut declarations, prefix, 2, true);
    }
    for prefix in ["below", "above"] {
        blocks::error_declarations(&mut declarations, prefix);
    }
    parameters::declare(&mut declarations, full);
    let borrowed = declarations
        .iter()
        .map(|(key, sort)| (key.as_str(), sort.clone()))
        .collect::<Vec<_>>();
    Fixture::with_limits(&borrowed, policy())
}

/// Physical impedance/current multiplication is registered explicitly, independently of equal dimensions.
fn equations(
    fixture: &mut Fixture,
    inputs: &[blocks::Block; 3],
    outputs: &[blocks::Block; 2],
    budget: &mut Budget,
) -> TestResult<[PhasorSolveDefinition; 2]> {
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        inputs[0].representation.reading().entries()[0]
            .descriptor()
            .kind()
            .clone(),
        outputs[0].representation.reading().entries()[0]
            .descriptor()
            .kind()
            .clone(),
        inputs[2].representation.reading().entries()[0]
            .descriptor()
            .kind()
            .clone(),
    )?;
    let mut definitions = Vec::new();
    for (input, output) in inputs[..2].iter().zip(outputs) {
        let law = PhasorBlockCorrespondence::prove(
            &mut fixture.cas,
            [input.representation.clone(), output.representation.clone()],
            PhasorConvention::PositiveTimePassive,
            budget,
        )?;
        definitions.push(PhasorSolveDefinition::new(
            &fixture.cas,
            law,
            inputs[2].representation.clone(),
            &registry,
            budget,
        )?);
    }
    definitions
        .try_into()
        .map_err(|_| "two exact solves".into())
}

/// Source means and both original parameter channels enter one complete atomic input group.
fn import<P: Scalar + 'static>(
    fixture: Fixture,
    inputs: &[blocks::Block; 3],
    parameters: &[nrz_matrix::ReadingAdapter<usize>; 2],
    dependencies: DependencyPolicy,
    budget: &mut Budget,
) -> TestResult<(Heap, Vec<ReadingBinding<P>>)> {
    let mut adapters = inputs
        .iter()
        .flat_map(|block| block.adapters.iter().cloned())
        .collect::<Vec<_>>();
    adapters.extend_from_slice(parameters);
    let overlaps = inputs
        .iter()
        .flat_map(|block| block.overlaps.iter().cloned())
        .collect();
    let universe = (0..3)
        .flat_map(|region| (0..4).map(move |index| EntryTicket { region, index }))
        .chain([
            EntryTicket {
                region: 3,
                index: 0,
            },
            EntryTicket {
                region: 4,
                index: 0,
            },
        ])
        .collect();
    let coverage = ReadingCoverage::new(universe, adapters, overlaps, &mut ledger()?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    budget.work(
        32,
        nrz_bindings::readings::algebra::Location::new("fixed reactance literal import"),
    )?;
    let imported = nrz_scalar::NumericLiteral::new("75", LiteralLimits::new(1024, 4096, 32))?
        .decode::<P>(&ArithmeticContext::new()?)?;
    let nominal = imported.center().get();
    let import_error = imported.displacement().get();
    let regions = vec![
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: vec![P::ZERO, -nominal, nominal, P::ZERO],
        },
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: vec![P::ZERO, nominal, -nominal, P::ZERO],
        },
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: vec![P::from_u8(3), P::ZERO, P::ZERO, P::from_u8(3)],
        },
        ReadingRegion {
            extent: Extent::new(1, 1)?,
            values: vec![P::ONE / P::from_u8(4)],
        },
        ReadingRegion {
            extent: Extent::new(1, 1)?,
            values: vec![P::ZERO],
        },
        ReadingRegion {
            extent: Extent::new(4, 1)?,
            values: vec![import_error; 4],
        },
        ReadingRegion {
            extent: Extent::new(4, 1)?,
            values: vec![import_error; 4],
        },
    ];
    let imported = heap.import_readings(
        Rc::new(fixture.cas),
        coverage,
        inputs
            .iter()
            .flat_map(|block| block.companions.iter().cloned())
            .collect(),
        &regions,
        dependencies,
        budget,
    )?;
    Ok((heap, imported.bindings().to_vec()))
}

/// Both solves read the same complete voltage operand without declaring another uncertain source.
fn publications<P: Scalar>(
    definitions: &[PhasorSolveDefinition; 2],
    outputs: &[blocks::Block; 2],
    bindings: &[ReadingBinding<P>],
    frames: &[CoordinateNormalization; 2],
    budget: &mut Budget,
) -> TestResult<[PhasorSolvePublication<P>; 2]> {
    let mut result = Vec::new();
    for (index, definition) in definitions.iter().enumerate() {
        let transfer = PhasorResidualTransfer::new(
            definition.clone(),
            [frames[0].clone(), frames[1].clone()],
            PhasorResidualNorm::ComponentEuclidean,
            budget,
        )?;
        let solve = PhasorSolve::new(
            transfer,
            [
                operand(bindings, index * 5, budget)?,
                operand(bindings, 10, budget)?,
            ],
            LiteralLimits::default(),
            budget,
        )?;
        result.push(PhasorSolvePublication::new(
            solve,
            outputs[index].coverage()?,
            outputs[index].companions.clone(),
            budget,
        )?);
    }
    result
        .try_into()
        .map_err(|_| "two publication plans".into())
}

/// Original scalar-copy selections retain their actual imported group and versions.
fn operand<P: Scalar>(
    bindings: &[ReadingBinding<P>],
    start: usize,
    budget: &mut Budget,
) -> TestResult<PhasorOperand<P>> {
    Ok(PhasorOperand::new(
        bindings[start].clone(),
        bindings[start + 1..start + 5]
            .to_vec()
            .try_into()
            .map_err(|_| "four copies")?,
        budget,
    )?)
}
