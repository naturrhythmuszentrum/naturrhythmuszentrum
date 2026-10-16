// SPDX-License-Identifier: AGPL-3.0-only
//! Owned public observations separate deterministic evidence from process measurements.

use super::ResultOf;
use nrz_matrix::{
    Heap, KernelEvidence, MomentAssurance, OperationError, Outcome, ParameterView, PublishedBundle,
    RecipeEvidence, SolveEvidence, SolveFailure, SolveStage,
};
use nrz_units::{Descriptor, Dimension};

/// Center/error pairs of one independently shaped numerical parameter region.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Parameter {
    /// Logical column-major extent.
    pub(crate) shape: [usize; 2],
    /// Displayed parameter centers.
    pub(crate) centers: Vec<f64>,
    /// Numerical evaluation radii, never physical source radii.
    pub(crate) errors: Vec<f64>,
    /// Quantity dimension of every parameter entry, independently of its storage schema.
    pub(crate) dimensions: Vec<Dimension>,
}

/// Complete inspectable numerical output, with optional solve and statistical evidence.
#[derive(Debug)]
pub(crate) struct Sample {
    /// Stable client slot name.
    pub(crate) name: String,
    /// Public record's stated arithmetic assumptions.
    pub(crate) environment: String,
    /// Value region in logical order.
    pub(crate) centers: Vec<f64>,
    /// Concrete physical or normalized output descriptor.
    pub(crate) quantity: Descriptor,
    /// Deterministic total region, including the overlapping numerical contribution once.
    pub(crate) totals: Vec<f64>,
    /// Overlapping represented-center evaluation region.
    pub(crate) errors: Vec<f64>,
    /// All available statistical parameters, ordered covariance, sensitivity, Jacobian, means.
    pub(crate) parameters: Vec<Parameter>,
    /// Statistical claim, independent from the deterministic scope.
    pub(crate) assurance: Option<MomentAssurance>,
    /// Solve proof retained verbatim through the public facade.
    pub(crate) solve: Option<SolveEvidence>,
    /// Deterministic map evidence, including its separate complete-phase work allowance.
    pub(crate) kernel: Option<KernelEvidence>,
    /// Complete statistical phase admission count captured before evaluation.
    pub(crate) statistical_work: usize,
    /// Solve lifecycle state of the published record.
    pub(crate) stage: Option<SolveStage>,
}

/// One integrated case, with wall times kept outside replayable numerical observations.
#[derive(Debug)]
pub(crate) struct Case {
    /// Stable case name matching the command-line selector.
    pub(crate) name: &'static str,
    /// Observations in actual execution order.
    pub(crate) samples: Vec<Sample>,
    /// Materialized complete cross blocks in point-pair order.
    pub(crate) cross: Vec<Parameter>,
    /// Refusal and recovery details from public failure records.
    pub(crate) refusals: Vec<Refusal>,
    /// Explicit successful factor constructions performed by this fixture.
    pub(crate) factors: usize,
    /// Elapsed stage names and seconds; never acceptance criteria.
    pub(crate) timings: Vec<(&'static str, f64)>,
    /// Retained public provenance accounting: records, edges, depth, bytes, traversal work.
    pub(crate) provenance: [usize; 5],
    /// Addressable input and initial source-parameter region sizes, excluding backend padding.
    pub(crate) input_regions: Vec<usize>,
}

impl Case {
    /// Start an unexecuted fixture without inventing numerical success.
    pub(crate) const fn new(name: &'static str) -> Self {
        Self {
            name,
            samples: Vec::new(),
            cross: Vec::new(),
            refusals: Vec::new(),
            factors: 0,
            timings: Vec::new(),
            provenance: [0; 5],
            input_regions: Vec::new(),
        }
    }

    /// Inspect imported values, deterministic companions and the one complete joint model.
    pub(crate) fn inputs(&mut self, heap: &Heap, inputs: &[PublishedBundle<f64>]) -> ResultOf {
        for input in inputs {
            self.input_regions
                .push(heap.value(input.value())?.region().extent().entries());
            if let Some(bound) = input.bound() {
                self.input_regions
                    .push(heap.companion(bound)?.region().extent().entries());
            }
        }
        let first = inputs.first().ok_or("input inventory")?;
        if let Some(&joint) = heap.joint_companions(first.value())?.first() {
            let view = heap.statistical(joint)?;
            for region in [view.factor_region(), view.weight_region()]
                .into_iter()
                .flatten()
            {
                self.input_regions.push(region.extent().entries());
            }
            if let Some(covariance) = view.covariance_region() {
                self.input_regions.push(covariance.extent().entries());
            }
            if let Some(errors) = view.parameter_errors() {
                self.input_regions.push(errors.extent().entries());
            }
            self.input_regions.extend(
                view.mean_error_regions()
                    .iter()
                    .map(|r| r.extent().entries()),
            );
        }
        Ok(())
    }

    /// Total retained addressable regions/descriptors and payload elements, not allocator usage.
    pub(crate) fn storage(&self) -> [usize; 2] {
        let mut regions = self.input_regions.len() + 2 * self.cross.len();
        let mut entries = self.input_regions.iter().sum::<usize>()
            + self
                .cross
                .iter()
                .map(|p| p.centers.len() + p.errors.len())
                .sum::<usize>();
        for sample in &self.samples {
            regions += 3 + 2 * sample.parameters.len();
            entries += sample.centers.len()
                + sample.totals.len()
                + sample.errors.len()
                + sample
                    .parameters
                    .iter()
                    .map(|p| p.centers.len() + p.errors.len())
                    .sum::<usize>();
        }
        [regions, entries]
    }

    /// Snapshot the crate's accounted metadata, distinct from peak process memory.
    pub(crate) const fn finish(&mut self, heap: &Heap) {
        let usage = heap.provenance_usage();
        self.provenance = [
            usage.records(),
            usage.edges(),
            usage.depth(),
            usage.bytes(),
            usage.work(),
        ];
    }
}

/// Capture every public parameter center and numerical radius without interval oracle arithmetic.
pub(crate) fn parameter(view: &ParameterView<'_, f64>) -> ResultOf<Parameter> {
    let shape = [view.extent().rows(), view.extent().columns()];
    let mut centers = Vec::new();
    let mut errors = Vec::new();
    let mut dimensions = Vec::new();
    for column in 0..shape[1] {
        for row in 0..shape[0] {
            centers.push(view.get(row, column)?);
            errors.push(view.error(row, column)?);
            dimensions.push(if let Some(mean) = view.mean_descriptor() {
                mean.dimension().clone()
            } else {
                view.descriptor(row, column)?.dimension().clone()
            });
        }
    }
    Ok(Parameter {
        shape,
        centers,
        errors,
        dimensions,
    })
}

/// Inspect the actual published record and all output regions through checked public access.
pub(crate) fn sample(
    heap: &Heap,
    outcome: &Outcome<Vec<PublishedBundle<f64>>>,
    name: &str,
    statistical_work: usize,
) -> ResultOf<Sample> {
    let bundle = outcome.output().first().ok_or("missing output")?;
    let record = heap.operation_record(outcome.record())?;
    let mut result = Sample {
        name: name.into(),
        environment: record.environment().into(),
        centers: heap.value(bundle.value())?.region().export()?,
        quantity: heap.value(bundle.value())?.region().descriptor().clone(),
        totals: heap
            .companion(bundle.bound().ok_or("total")?)?
            .region()
            .export()?,
        errors: heap
            .companion(bundle.evaluation().ok_or("evaluation")?)?
            .region()
            .export()?,
        parameters: Vec::new(),
        assurance: None,
        solve: record.solve_evidence().cloned(),
        kernel: record
            .evidence()
            .iter()
            .find_map(|evidence| match evidence {
                RecipeEvidence::Kernel(proof) => Some(proof.clone()),
                _ => None,
            }),
        statistical_work,
        stage: record.solve_stage(),
    };
    if statistical_work != 0 {
        let moments = heap.moments(heap.moment_companion(bundle.value())?)?;
        result.assurance = Some(moments.assurance());
        result.parameters.push(parameter(moments.covariance())?);
        for view in [moments.sensitivity(), moments.jacobian(), moments.means()]
            .into_iter()
            .flatten()
        {
            result.parameters.push(parameter(view)?);
        }
    }
    Ok(result)
}

/// Typed failure facts, including the valid partial numerical proof if one exists.
#[derive(Debug)]
pub(crate) struct Refusal {
    /// Exact public refusal, preserved without string matching.
    pub(crate) cause: OperationError,
    /// Last reached lifecycle stage.
    pub(crate) stage: SolveStage,
    /// Conservative admitted cost of one numerical phase.
    pub(crate) work: u64,
    /// Independently valid historical theorem stages, without a success handle.
    pub(crate) proof: Option<SolveEvidence>,
}

/// Capture a complete failed request through the public recorded lifecycle.
pub(crate) fn refusal(failure: &SolveFailure<f64>) -> Refusal {
    let record = failure.record();
    let proof = record.numerical().and_then(|failure| {
        failure
            .record()
            .historical_evidence()
            .iter()
            .find_map(|item| match item {
                RecipeEvidence::Solve(proof) => Some(proof.as_ref().clone()),
                _ => None,
            })
    });
    Refusal {
        cause: failure.cause().clone(),
        stage: record.stage(),
        work: record.attempt_work(),
        proof,
    }
}
