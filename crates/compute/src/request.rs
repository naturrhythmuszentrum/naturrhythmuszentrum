// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit request policies and admission before workload preparation.

use crate::decimal::MAX_INTEGER;
use crate::{Binary64, Dependency, Operation, Placement, Scope, Seed, Width};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// One owned case selection; all dimensions and policies are explicit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseRequest {
    /// Named catalog operation.
    pub operation: Operation,
    /// Logical source width.
    pub width: Width,
    /// Recurrence or traversal dependence.
    pub dependency: Dependency,
    /// Preparation/restoration boundary.
    pub scope: Scope,
    /// Ordered shape dimensions; scalar arithmetic uses an empty shape.
    pub shape: Vec<u64>,
    /// Equal per-worker initial update or pass count.
    pub item_length: u64,
    /// Active worker count; coordinator is separate.
    pub workers: u64,
    /// Requested placement.
    pub placement: Placement,
    /// Explicit externally allowed CPU set, empty for unbound requests.
    pub allowed_cpus: Vec<u64>,
    /// Per-case admission duration in integer nanoseconds.
    pub budget_ns: u64,
    /// Positive diagonal-dominance margin for factor/solve, absent otherwise.
    pub diagonal_margin: Option<Binary64>,
    /// Explicit numerical arrays for LC recipes, never inferred defaults.
    pub inputs: Vec<Vec<Binary64>>,
    /// Explicit inverse-time cycle indices.
    pub cycles: Vec<i64>,
}

/// Statistical policy passed unchanged to matrix's host sampler.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingPolicy {
    /// Required complete row count, at least ten.
    pub samples: u32,
    /// Bootstrap resamples, at least one thousand.
    pub resamples: u32,
    /// Authoritative finite binary64 confidence strictly inside zero and one.
    pub confidence: Binary64,
    /// Positive timed warm-up duration in nanoseconds.
    pub warm_up_ns: u64,
    /// Positive timed measurement target in nanoseconds.
    pub measurement_ns: u64,
    /// Independent sampler analysis report payload cap.
    pub analysis_report_bytes: u64,
    /// Independent sampler analysis scratch payload cap.
    pub analysis_scratch_bytes: u64,
}

/// Explicit aggregate capacity policy; no machine-derived defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePolicy {
    /// Total admission duration from run entry.
    pub budget_ns: u64,
    /// Maximum requested case count.
    pub cases: u64,
    /// Maximum independent repetitions.
    pub repetitions: u64,
    /// Total attempts, including controls, pilot and warm-up.
    pub attempts: u64,
    /// Maximum iterations per row or warm-up group.
    pub iterations_per_row: u64,
    /// Maximum named operations per item.
    pub operations_per_item: u64,
    /// Aggregate input/output buffer payload cap.
    pub buffer_bytes: u64,
    /// Aggregate scratch payload cap.
    pub scratch_bytes: u64,
    /// Aggregate report payload cap.
    pub report_bytes: u64,
    /// Active worker cap.
    pub workers: u64,
    /// Stack reservation for each spawned worker.
    pub worker_stack_bytes: u64,
}

/// Current-schema owned request for one public run invocation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    /// Exact schema triple.
    pub version: [u64; 3],
    /// Caller-assigned public lowercase slug.
    pub run_id: String,
    /// Explicit ordered case selections.
    pub cases: Vec<CaseRequest>,
    /// One full seed per independent repetition; effective offsets must differ.
    pub seeds: Vec<Seed>,
    /// Sampling policy.
    pub sampling: SamplingPolicy,
    /// Aggregate resource policy.
    pub resources: ResourcePolicy,
}

/// Host-declared bounds; never inferred from pilot durations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionBounds {
    /// Maximum uninterrupted outer execution interval U.
    pub uninterrupted_ns: u64,
    /// Complete mandatory outer tail C, including every worker and report conversion.
    pub finalization_ns: u64,
    /// Maximum uninterrupted item `U_item`, including diagnostic items.
    pub item_ns: u64,
    /// Sampler-owned uninterrupted callback/checkpoint bound.
    pub sampler_uninterrupted_ns: u64,
    /// Complete sampler finalization bound.
    pub sampler_finalization_ns: u64,
    /// Complete receipt preservation cost.
    pub receipt_ns: u64,
    /// Complete output disposal cost.
    pub disposal_ns: u64,
    /// Complete adapter cleanup cost, including worker joins.
    pub cleanup_ns: u64,
    /// Complete outer owned-report conversion cost.
    pub report_ns: u64,
    /// Positive blocking receive interval between worker cancellation checkpoints.
    pub wait_poll_ns: u64,
    /// Declared maximum cancellation read cost.
    pub cancellation_read_ns: u64,
}

/// Shared explicit cancellation token; no global state or implicit worker.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Request cooperative cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Observe cancellation at an admission checkpoint.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Typed admission refusal; ordinary environment limitations are case outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid request field {field}: {cause:?}")]
pub struct RequestError {
    /// Stable field path naming the refused policy.
    pub field: &'static str,
    /// Typed reason, without allocating user input into an error.
    pub cause: RequestCause,
}

/// Request errors precede workload preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestCause {
    /// Unsupported schema revision.
    Schema,
    /// Invalid nonzero count, policy or finite domain.
    Policy,
    /// Checked product or wire count/duration overflow.
    Overflow,
    /// Identity collision or incompatible enum combination.
    Identity,
    /// Explicit aggregate capacity cannot retain the request's work.
    Capacity,
}

/// Construct a fixed-size field error.
const fn error(field: &'static str, cause: RequestCause) -> RequestError {
    RequestError { field, cause }
}

/// Validate count and duration representability before any work.
fn bounded(field: &'static str, values: &[u64]) -> Result<(), RequestError> {
    if values.iter().any(|n| *n > MAX_INTEGER) {
        return Err(error(field, RequestCause::Overflow));
    }
    Ok(())
}

impl RunRequest {
    /// Validate schema, policies, identities and checked work reservations.
    /// # Errors
    /// Returns a typed field refusal before any workload allocation or dispatch.
    pub fn validate(&self, bounds: Option<ExecutionBounds>) -> Result<(), RequestError> {
        if self.version != [1, 0, 0] {
            return Err(error("version", RequestCause::Schema));
        }
        let slug = !self.run_id.is_empty();
        let spelling = self
            .run_id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !(slug && spelling) {
            return Err(error("run_id", RequestCause::Identity));
        }
        if self.cases.is_empty() || self.seeds.is_empty() {
            return Err(error("cases/seeds", RequestCause::Policy));
        }
        self.validate_sampling()?;
        self.validate_resources()?;
        for (index, seed) in self.seeds.iter().enumerate() {
            if self.seeds[..index]
                .iter()
                .any(|previous| previous.offset() == seed.offset())
            {
                return Err(error("seeds", RequestCause::Identity));
            }
        }
        if let Some(bounds) = bounds {
            bounded(
                "bounds",
                &[
                    bounds.uninterrupted_ns,
                    bounds.finalization_ns,
                    bounds.item_ns,
                    bounds.cancellation_read_ns,
                    bounds.sampler_uninterrupted_ns,
                    bounds.sampler_finalization_ns,
                    bounds.receipt_ns,
                    bounds.disposal_ns,
                    bounds.cleanup_ns,
                    bounds.report_ns,
                    bounds.wait_poll_ns,
                ],
            )?;
            if bounds.wait_poll_ns == 0 || bounds.wait_poll_ns > bounds.uninterrupted_ns {
                return Err(error("bounds.wait_poll_ns", RequestCause::Policy));
            }
            let longest = bounds
                .item_ns
                .max(bounds.receipt_ns)
                .max(bounds.disposal_ns)
                .max(bounds.cleanup_ns);
            let uninterrupted = longest
                .checked_add(bounds.sampler_uninterrupted_ns)
                .ok_or_else(|| error("bounds", RequestCause::Overflow))?;
            let tail = [
                bounds.receipt_ns,
                bounds.disposal_ns,
                bounds.cleanup_ns,
                bounds.sampler_finalization_ns,
                bounds.report_ns,
            ]
            .into_iter()
            .try_fold(0_u64, u64::checked_add)
            .ok_or_else(|| error("bounds", RequestCause::Overflow))?;
            if uninterrupted > bounds.uninterrupted_ns || tail > bounds.finalization_ns {
                return Err(error("bounds", RequestCause::Policy));
            }
            if bounds.item_ns > bounds.uninterrupted_ns
                || bounds.cancellation_read_ns > bounds.uninterrupted_ns
            {
                return Err(error("bounds", RequestCause::Policy));
            }
        }
        for case in &self.cases {
            case.validate(&self.resources)?;
        }
        Ok(())
    }

    /// Check strict confidence and unchanged host sampler minima.
    fn validate_sampling(&self) -> Result<(), RequestError> {
        let s = self.sampling;
        let confidence = f64::from_bits(s.confidence.0);
        let finite = confidence.is_finite();
        let interior = confidence > 0.0 && confidence < 1.0;
        let minima = s.samples >= 10 && s.resamples >= 1000;
        if !(finite && interior && minima) {
            return Err(error("sampling", RequestCause::Policy));
        }
        bounded(
            "sampling",
            &[
                s.warm_up_ns,
                s.measurement_ns,
                s.analysis_report_bytes,
                s.analysis_scratch_bytes,
            ],
        )?;
        if s.warm_up_ns == 0 || s.measurement_ns == 0 {
            return Err(error("sampling.duration", RequestCause::Policy));
        }
        Ok(())
    }

    /// Check aggregate dimensions and reservations, without preparing work.
    fn validate_resources(&self) -> Result<(), RequestError> {
        let r = self.resources;
        let values = [
            r.budget_ns,
            r.cases,
            r.repetitions,
            r.attempts,
            r.iterations_per_row,
            r.operations_per_item,
            r.buffer_bytes,
            r.scratch_bytes,
            r.report_bytes,
            r.workers,
            r.worker_stack_bytes,
        ];
        bounded("resources", &values)?;
        if values.contains(&0) {
            return Err(error("resources", RequestCause::Policy));
        }
        let cases =
            u64::try_from(self.cases.len()).map_err(|_| error("cases", RequestCause::Overflow))?;
        let repeats =
            u64::try_from(self.seeds.len()).map_err(|_| error("seeds", RequestCause::Overflow))?;
        let outcomes = cases
            .checked_mul(repeats)
            .filter(|n| *n <= MAX_INTEGER)
            .ok_or_else(|| error("outcomes", RequestCause::Overflow))?;
        let stacks = r
            .workers
            .checked_mul(r.worker_stack_bytes)
            .ok_or_else(|| error("worker_stack_bytes", RequestCause::Overflow))?;
        if cases > r.cases
            || repeats > r.repetitions
            || stacks > r.scratch_bytes
            || outcomes > r.report_bytes
        {
            return Err(error("resources", RequestCause::Capacity));
        }
        Ok(())
    }
}

impl CaseRequest {
    /// Check count products, shape and operation-domain consistency.
    fn validate(&self, caps: &ResourcePolicy) -> Result<(), RequestError> {
        self.validate_recipe()?;
        self.validate_inputs()?;
        bounded("case", &[self.item_length, self.workers, self.budget_ns])?;
        bounded("case.shape", &self.shape)?;
        bounded("case.allowed_cpus", &self.allowed_cpus)?;
        if self.item_length == 0
            || self.workers == 0
            || self.budget_ns == 0
            || self.shape.contains(&0)
        {
            return Err(error("case", RequestCause::Policy));
        }
        let elements = self
            .shape
            .iter()
            .try_fold(1_u64, |n, dimension| n.checked_mul(*dimension))
            .ok_or_else(|| error("case.shape", RequestCause::Overflow))?;
        let numerators = crate::kernels::counts(self, self.item_length)
            .map_err(|_| error("case.work", RequestCause::Overflow))?;
        let work = numerators
            .floating_operations
            .unwrap_or(numerators.elements);
        let total_elements = elements
            .checked_mul(self.item_length)
            .and_then(|n| n.checked_mul(self.workers))
            .filter(|n| *n <= MAX_INTEGER)
            .ok_or_else(|| error("case.work", RequestCause::Overflow))?;
        if self.workers > caps.workers
            || work > caps.operations_per_item
            || total_elements > caps.operations_per_item
        {
            return Err(error("case.work", RequestCause::Capacity));
        }
        if self
            .inputs
            .iter()
            .flatten()
            .any(|bits| !f64::from_bits(bits.0).is_finite())
        {
            return Err(error("case.inputs", RequestCause::Policy));
        }
        if self.cycles.iter().any(|n| n.unsigned_abs() > MAX_INTEGER) {
            return Err(error("case.cycles", RequestCause::Overflow));
        }
        Ok(())
    }
}

impl CaseRequest {
    /// Validate enum combinations before capability checks or preparation.
    fn validate_recipe(&self) -> Result<(), RequestError> {
        let (dimensions, dependence) = match self.operation {
            Operation::Add
            | Operation::Multiply
            | Operation::FmaAddend
            | Operation::FmaMultiplicand
            | Operation::MultiplyAdd
            | Operation::Divide
            | Operation::SquareRoot => (
                0,
                matches!(
                    self.dependency,
                    Dependency::Dependent | Dependency::Independent
                ),
            ),
            Operation::Read => (1, self.dependency == Dependency::MultiAccumulatorReduction),
            Operation::Write | Operation::Copy | Operation::FusedStream => {
                (1, self.dependency == Dependency::IndependentElements)
            }
            Operation::Gemm => (3, self.dependency == Dependency::BackendKernel),
            Operation::Gemv | Operation::Solve => (2, self.dependency == Dependency::BackendKernel),
            Operation::Factor => (1, self.dependency == Dependency::BackendKernel),
            Operation::ForwardFused
            | Operation::ForwardSeparate
            | Operation::Inductance
            | Operation::Capacitance => (
                self.shape.len(),
                self.dependency == Dependency::PreparedTraversal,
            ),
            Operation::InverseTime => (2, self.dependency == Dependency::PreparedTraversal),
        };
        if self.shape.len() != dimensions || !dependence {
            return Err(error("case.recipe", RequestCause::Identity));
        }
        let stream = matches!(
            self.dependency,
            Dependency::MultiAccumulatorReduction | Dependency::IndependentElements
        );
        if stream && self.scope != Scope::Prepared {
            return Err(error("case.scope", RequestCause::Identity));
        }
        let repeated = self.scope == Scope::RestorationInclusiveRepeated;
        if repeated && self.operation != Operation::Factor {
            return Err(error("case.scope", RequestCause::Identity));
        }
        if self.operation == Operation::Factor
            && self.scope != Scope::RestorationInclusiveRepeated
            && self.item_length != 1
        {
            return Err(error("case.item_length", RequestCause::Policy));
        }
        let arithmetic = dimensions == 0;
        if arithmetic && self.scope != Scope::Prepared {
            return Err(error("case.scope", RequestCause::Identity));
        }
        let factor = matches!(self.operation, Operation::Factor | Operation::Solve);
        match self.diagonal_margin {
            Some(bits) if factor => {
                let value = f64::from_bits(bits.0);
                if !value.is_finite() || value <= 0.0 {
                    return Err(error("case.diagonal_margin", RequestCause::Policy));
                }
            }
            None if !factor => {}
            Some(_) | None => return Err(error("case.diagonal_margin", RequestCause::Identity)),
        }
        Ok(())
    }
}

impl CaseRequest {
    /// Validate finite input domains and exact recipe population identities.
    fn validate_inputs(&self) -> Result<(), RequestError> {
        let constrained = self.placement == Placement::ExternallyConstrained;
        if constrained && self.allowed_cpus.is_empty() {
            return Err(error("case.allowed_cpus", RequestCause::Policy));
        }
        if !constrained && !self.allowed_cpus.is_empty() {
            return Err(error("case.allowed_cpus", RequestCause::Identity));
        }
        for (index, cpu) in self.allowed_cpus.iter().enumerate() {
            if self.allowed_cpus[..index].contains(cpu) {
                return Err(error("case.allowed_cpus", RequestCause::Identity));
            }
        }
        match self.operation {
            Operation::Add
            | Operation::Multiply
            | Operation::Divide
            | Operation::FmaAddend
            | Operation::FmaMultiplicand
            | Operation::MultiplyAdd
            | Operation::SquareRoot
            | Operation::Read
            | Operation::Copy
            | Operation::Gemm
            | Operation::Gemv
            | Operation::Factor
            | Operation::Solve => {
                if !self.inputs.is_empty() || !self.cycles.is_empty() {
                    return Err(error("case.inputs", RequestCause::Identity));
                }
            }
            Operation::Write | Operation::FusedStream => {
                let [values] = self.inputs.as_slice() else {
                    return Err(error("case.inputs", RequestCause::Identity));
                };
                let [value] = values.as_slice() else {
                    return Err(error("case.inputs", RequestCause::Identity));
                };
                if !f64::from_bits(value.0).is_normal() || !self.cycles.is_empty() {
                    return Err(error("case.inputs", RequestCause::Policy));
                }
            }
            Operation::ForwardFused | Operation::ForwardSeparate => {
                self.validate_lc_shape(4)?;
                self.positive_array(0)?;
                self.positive_array(1)?;
                if !self.cycles.is_empty() {
                    return Err(error("case.cycles", RequestCause::Identity));
                }
            }
            Operation::Inductance | Operation::Capacitance => {
                self.validate_lc_shape(2)?;
                self.positive_array(0)?;
                self.positive_array(1)?;
                if !self.cycles.is_empty() {
                    return Err(error("case.cycles", RequestCause::Identity));
                }
            }
            Operation::InverseTime => {
                if self.inputs.len() != 3 || self.cycles.is_empty() {
                    return Err(error("case.inputs", RequestCause::Identity));
                }
                self.positive_array(1)?;
                self.positive_array(2)?;
                let shape = [
                    u64::try_from(self.inputs[0].len()).ok(),
                    u64::try_from(self.cycles.len()).ok(),
                ];
                if shape
                    != self
                        .shape
                        .iter()
                        .copied()
                        .map(Some)
                        .collect::<Vec<_>>()
                        .as_slice()
                    || self.inputs[0].is_empty()
                {
                    return Err(error("case.shape", RequestCause::Identity));
                }
                if self.inputs[1].len() != 1 || self.inputs[2].len() != 1 {
                    return Err(error("case.inputs", RequestCause::Identity));
                }
            }
        }
        Ok(())
    }

    /// Match the explicit nonsquare input axes, without inferring a default population.
    fn validate_lc_shape(&self, arrays: usize) -> Result<(), RequestError> {
        if self.inputs.len() != arrays || self.shape.len() != arrays {
            return Err(error("case.shape", RequestCause::Identity));
        }
        for (array, count) in self.inputs.iter().zip(&self.shape) {
            if array.is_empty() || u64::try_from(array.len()).ok() != Some(*count) {
                return Err(error("case.shape", RequestCause::Identity));
            }
        }
        Ok(())
    }

    /// Positive finite-normal axes and shared denominators precede any timed preparation.
    fn positive_array(&self, index: usize) -> Result<(), RequestError> {
        let values = self
            .inputs
            .get(index)
            .ok_or_else(|| error("case.inputs", RequestCause::Identity))?;
        if values.is_empty() {
            return Err(error("case.inputs", RequestCause::Policy));
        }
        for bits in values {
            let value = f64::from_bits(bits.0);
            if !value.is_normal() || value <= 0.0 {
                return Err(error("case.inputs", RequestCause::Policy));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) mod tests;
