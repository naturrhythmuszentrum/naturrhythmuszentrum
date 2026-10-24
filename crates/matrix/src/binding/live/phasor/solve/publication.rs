// SPDX-License-Identifier: AGPL-3.0-only
//! A complete current family and all eight radius parameters publish in one existing heap transaction.

use super::{
    super::{AT, PhasorError, publication},
    PhasorSolve, PhasorSolveEvaluation,
};
use crate::{CompanionAdapter, DependencyPolicy, Heap, HeapError, ReadingCoverage, ReadingImport};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetKind};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Complete current phasor/vector/component and radius declarations prepared before execution.
#[derive(Clone, Debug)]
pub struct PhasorSolvePublication<P: Scalar>(Rc<Plan<P>>);

/// Immutable exact output inventory and flattened original source closure.
#[derive(Debug)]
struct Plan<P: Scalar> {
    /// Checked original equation, numerical sources and named residual transfer.
    solve: PhasorSolve<P>,
    /// Complete output block, vector and all four scalar-copy readings.
    coverage: ReadingCoverage<usize>,
    /// Separate identity total and numerical-error parameters for every stored copy.
    companions: Vec<CompanionAdapter<usize>>,
    /// Flattened source versions, including all earlier result ancestors.
    sources: Vec<crate::binding::live::selected::SelectedReading>,
    /// Coarse admission stays coarse; fine sources authorize the operand closure.
    dependencies: DependencyPolicy,
    /// Retained exact metadata allowance beside evaluation and publication scratch.
    bytes: u64,
}

impl<P: Scalar> PhasorSolvePublication<P> {
    /// Require the complete fresh current slot, all component views and both radius channels.
    /// # Errors
    /// Refuses missing views, changed maps/targets/roots, incomplete companions and limits.
    pub fn new(
        solve: PhasorSolve<P>,
        coverage: ReadingCoverage<usize>,
        companions: Vec<CompanionAdapter<usize>>,
        budget: &mut Budget,
    ) -> Result<Self, PhasorError> {
        publication::check::coverage(solve.definition().output(), &coverage, &companions, budget)?;
        let sources = publication::sources(solve.operands(), budget)?;
        // The solve is an Rc-backed reference already charged by its owner.
        // This closed inventory adds metadata, actual encoded witnesses and source records.
        let mut bytes = 8192_u64;
        let targets = companions.iter().flat_map(CompanionAdapter::targets);
        for adapter in coverage.readings().iter().chain(targets) {
            budget.work(1, AT)?;
            let proof = u64::try_from(adapter.representation().witness().exported_byte_len())
                .map_err(|_| HeapError::ExtentOverflow)?;
            bytes = bytes.checked_add(proof).ok_or(HeapError::ExtentOverflow)?;
        }
        let ancestors = sources
            .readings
            .len()
            .checked_mul(std::mem::size_of::<
                crate::binding::live::selected::SelectedReading,
            >())
            .ok_or(HeapError::ExtentOverflow)?;
        bytes = bytes
            .checked_add(u64::try_from(ancestors).map_err(|_| HeapError::ExtentOverflow)?)
            .ok_or(HeapError::ExtentOverflow)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Plan {
            solve,
            coverage,
            companions,
            sources: sources.readings,
            dependencies: sources.dependencies,
            bytes,
        })))
    }
    /// Complete original solve and all input versions.
    #[must_use]
    pub fn solve(&self) -> &PhasorSolve<P> {
        &self.0.solve
    }
    /// Whole phasor, component vector and all selected scalar-copy declarations.
    #[must_use]
    pub fn coverage(&self) -> &ReadingCoverage<usize> {
        &self.0.coverage
    }
    /// Every original output parameter and its exact target.
    #[must_use]
    pub fn companions(&self) -> &[CompanionAdapter<usize>] {
        &self.0.companions
    }
    /// Newly owned metadata, encoded witnesses and source records; the shared solve is charged once.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Atomically restored current readings and the complete Real solve certificate that produced them.
#[derive(Clone, Debug)]
pub struct PublishedPhasorSolve<P: Scalar> {
    /// Every predeclared output reading and companion, available together.
    readings: ReadingImport<P>,
    /// Complete numerical proof with the original equation and live source closure.
    evaluation: PhasorSolveEvaluation<P>,
}
impl<P: Scalar> PublishedPhasorSolve<P> {
    /// Complete current phasor, vector and scalar-copy family.
    #[must_use]
    pub const fn readings(&self) -> &ReadingImport<P> {
        &self.readings
    }
    /// Real solve proof and matching reference/norm transport.
    #[must_use]
    pub const fn evaluation(&self) -> &PhasorSolveEvaluation<P> {
        &self.evaluation
    }
}

impl Heap {
    /// Certify and atomically publish all restored current views and deterministic companions.
    ///
    /// Every output inherits the validated operands' reading, proof and companion closure.
    /// Fine admission preserves disjoint entries; a whole-base operand remains conservative.
    /// # Errors
    /// Refuses bound output roots, poles, unresolved reactance, stale sources, failed proof and capacity.
    pub fn publish_phasor_solve<P: Scalar + 'static>(
        &mut self,
        publication: &PhasorSolvePublication<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<PublishedPhasorSolve<P>, PhasorError> {
        let plan = &publication.0;
        for group in self.symbols.groups() {
            for reading in group.coverage.readings() {
                budget.work(1, AT)?;
                if reading.reading().family().root()
                    == plan.solve.definition().output().reading().family().root()
                {
                    return Err(PhasorError::Identity);
                }
            }
        }
        budget.with_retained_payload(plan.bytes, AT, |budget| {
            plan.solve.validate(self)?;
            let run = super::inventory::Run::new(&plan.solve, true, budget)?;
            let mut evaluation =
                super::execution::evaluate(self, &plan.solve, &run, context, budget)?;
            let regions = run.regions(&evaluation.contents)?;
            evaluation.inventory = run.published(&evaluation.contents, &regions)?;
            let provenance = crate::binding::live::import::ImportProvenance {
                definition: crate::binding::live::ResultDefinition::Solve(
                    plan.solve.definition().clone(),
                ),
                sources: plan.sources.clone(),
                dependencies: plan.dependencies,
                coverage: evaluation.coverage(),
            };
            // Fresh output declarations have no operand edges. Sealed solve provenance
            // installs the actual source closure before the atomic import publishes.
            let readings = self.import_readings_with_provenance(
                Rc::clone(&plan.solve.operands()[0].whole().group.graph),
                plan.coverage.clone(),
                plan.companions.clone(),
                regions.values(&run)?,
                DependencyPolicy::WholeBase,
                Some(provenance),
                budget,
            )?;
            Ok(PublishedPhasorSolve {
                readings,
                evaluation,
            })
        })
    }
}
