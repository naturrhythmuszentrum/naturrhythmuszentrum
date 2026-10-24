// SPDX-License-Identifier: AGPL-3.0-only
//! Public physical LC equation fixtures with predeclared current families and named references.

#[path = "phasor_solve/declarations.rs"]
mod declarations;
#[path = "phasor_solve/inputs.rs"]
mod inputs;
#[path = "phasor_solve/voltage.rs"]
mod voltage;
use crate::reading_testing::{TestResult, ledger};
use crate::test_api::{
    CompanionAdapter, Heap, HeapLimits, PhasorOperand, PhasorSolve, PhasorSolvePublication,
    ReadingBinding, ReadingCoverage,
};
use nrz_bindings::{
    Budget, Limits,
    readings::{
        CoordinateNormalization, PhasorResidualNorm, PhasorResidualTransfer, PhasorSolveDefinition,
    },
};
use nrz_scalar::{LiteralLimits, Scalar};
use std::rc::Rc;

/// Explicit cumulative caller policy for correspondence, equation, references and publication.
pub(crate) fn policy() -> Limits {
    Limits {
        work: 16_000_000,
        ..Limits::default()
    }
}
/// One cumulative operation ledger; fixture declaration scaffolding retains its separate setup costs.
pub(crate) fn budget() -> TestResult<Budget> {
    Ok(Budget::new(policy())?)
}

/// Complete original sources and a fresh predeclared output family.
pub(crate) struct Case<P: Scalar> {
    /// Cumulative Work after correspondence, equation, metrics and numerical input admission.
    pub(crate) construction: [u64; 4],
    /// Numerical owner for both original operands and published current.
    pub(crate) heap: Heap,
    /// Original impedance then voltage, each in whole/scalar-copy order.
    pub(crate) bindings: Vec<ReadingBinding<P>>,
    /// Checked original physical equation, including the whole-family imaginary-axis proof.
    pub(crate) definition: PhasorSolveDefinition,
    /// Unit voltage/current metrics and alternative voltage-reference metric.
    pub(crate) frames: [CoordinateNormalization; 3],
    /// Complete fresh current output views.
    pub(crate) coverage: ReadingCoverage<usize>,
    /// Fresh voltage output prepared before the current solve.
    pub(crate) restoration: voltage::Declarations,
    /// Both output radius channels for every actual copy.
    pub(crate) companions: Vec<CompanionAdapter<usize>>,
}

impl<P: Scalar> Case<P> {
    /// Bind the actual ordered input families, never copied or reinterpreted by equal schema.
    pub(crate) fn operands(&self, budget: &mut Budget) -> TestResult<[PhasorOperand<P>; 2]> {
        let build = |start: usize, budget: &mut Budget| -> TestResult<PhasorOperand<P>> {
            Ok(PhasorOperand::new(
                self.bindings[start].clone(),
                self.bindings[start + 1..start + 5]
                    .to_vec()
                    .try_into()
                    .map_err(|_| "four copies")?,
                budget,
            )?)
        };
        Ok([build(0, budget)?, build(5, budget)?])
    }
    /// Change the explicitly retained voltage reference or residual norm before numerical access.
    pub(crate) fn solve(
        &self,
        changed: bool,
        norm: PhasorResidualNorm,
        budget: &mut Budget,
    ) -> TestResult<PhasorSolve<P>> {
        let voltage = if changed {
            &self.frames[2]
        } else {
            &self.frames[0]
        };
        let transfer = PhasorResidualTransfer::new(
            self.definition.clone(),
            [voltage.clone(), self.frames[1].clone()],
            norm,
            budget,
        )?;
        Ok(PhasorSolve::new(
            transfer,
            self.operands(budget)?,
            LiteralLimits::default(),
            budget,
        )?)
    }
    /// Restore the voltage through the checked product using the actually published current copies.
    pub(crate) fn voltage(
        &self,
        current: &crate::test_api::PublishedPhasorSolve<P>,
        budget: &mut Budget,
    ) -> TestResult<crate::test_api::PhasorPublication<P>> {
        let readings = current.readings().bindings();
        let current = PhasorOperand::new(
            readings[0].clone(),
            readings[1..5]
                .to_vec()
                .try_into()
                .map_err(|_| "four current copies")?,
            budget,
        )?;
        let impedance = self.operands(budget)?[0].clone();
        let product = crate::test_api::PhasorProduct::new(
            self.definition.correspondence().clone(),
            [impedance, current],
            budget,
        )?;
        Ok(crate::test_api::PhasorPublication::new(
            product,
            self.restoration.definition.clone(),
            self.restoration.coverage.clone(),
            self.restoration.companions.clone(),
            budget,
        )?)
    }
    /// Prepare every result and companion before running the numerical solve.
    pub(crate) fn publication(&self, budget: &mut Budget) -> TestResult<PhasorSolvePublication<P>> {
        Ok(PhasorSolvePublication::new(
            self.solve(false, PhasorResidualNorm::ComponentEuclidean, budget)?,
            self.coverage.clone(),
            self.companions.clone(),
            budget,
        )?)
    }
}

/// Nominal frequencies use an independent exact LC reactance oracle in the test layer.
pub(crate) fn case<P: Scalar + 'static>(
    reactance: P,
    radius: P,
    rounded: bool,
    budget: &mut Budget,
) -> TestResult<Case<P>> {
    case_in_heap(
        reactance,
        radius,
        rounded,
        Heap::new(HeapLimits::default())?,
        budget,
    )
}

/// Keep unrelated live experiments in the same numerical owner while preparing a driven equation.
pub(crate) fn case_in_heap<P: Scalar + 'static>(
    reactance: P,
    radius: P,
    rounded: bool,
    mut heap: Heap,
    budget: &mut Budget,
) -> TestResult<Case<P>> {
    let mut fixture = declarations::fixture()?;
    let (adapters, relations, current, coverage, companions) =
        declarations::readings(&mut fixture)?;
    let registry = declarations::registry(&adapters, &current)?;
    let law = nrz_bindings::readings::PhasorBlockCorrespondence::prove(
        &mut fixture.cas,
        [adapters[0].representation().clone(), current],
        nrz_bindings::readings::PhasorConvention::PositiveTimePassive,
        budget,
    )?;
    let correspondence_work = budget.usage().work;
    let definition = PhasorSolveDefinition::new(
        &fixture.cas,
        law,
        adapters[5].representation().clone(),
        &registry,
        budget,
    )?;
    let equation_work = budget.usage().work;
    let restoration = voltage::prepare(&mut fixture, &definition, &registry, budget)?;
    let frames = declarations::frames(&mut fixture, &definition, budget)?;
    let metric_work = budget.usage().work;
    let source_companions = inputs::companions(&mut fixture, &adapters)?;
    let universe = (0..2)
        .flat_map(|region| crate::phasor_testing::tickets(region, &[0, 1, 2, 3]))
        .collect();
    let source_coverage = ReadingCoverage::new(universe, adapters, relations, &mut ledger()?)?;
    let imported = heap.import_readings(
        Rc::new(fixture.cas),
        source_coverage,
        source_companions,
        &inputs::regions(reactance, radius, rounded)?,
        crate::test_api::DependencyPolicy::WholeBase,
        budget,
    )?;
    let construction = [
        correspondence_work,
        equation_work,
        metric_work,
        budget.usage().work,
    ];
    Ok(Case {
        construction,
        heap,
        bindings: imported.bindings().to_vec(),
        definition,
        frames,
        coverage,
        companions,
        restoration,
    })
}
