// SPDX-License-Identifier: AGPL-3.0-only
//! Complete solve attempts distinguish binding, factor, evaluation and publication boundaries.
//!
//! # Recorded solve lifecycles
//!
//! `Heap::solve_recorded` and `solve_factored_recorded` cover the full request lifecycle. A `SolveFailureRecord` distinguishes binding, factorization, evaluation and publication refusals, retaining the checked operation, every proposed operand and any numerical failure record. A binding refusal has no invented execution or certificate. A successful `OperationRecord` reports `SolveStage::Published` and its complete `solve_evidence`; current access still validates every input and output dependency, while historical records remain inspectable after replacement. `RecoveredSolve::initial_record` and `RecoveryFailure::initial_record`/`promoted_record` preserve this same lifecycle through precision recovery, including binding refusals. Optional `SolveRecordKeys` retain plan, binding, point and node identities under a combined 4096-byte limit through `SolvePolicy::with_record_keys`; they never authorize cache reuse.

use crate::binding::NativeRecipe;
use crate::{
    Binding, CheckedOperation, FactorRef, Failure, Handle, Heap, OperationError, Outcome,
    PublishedBundle, RecipeEvidence, SolveError, Value,
};
use nrz_scalar::Scalar;

/// Optional client identities retained verbatim, without changing numerical dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveRecordKeys {
    /// Plan, binding, point and node, in that order.
    values: [Option<String>; 4],
}
impl SolveRecordKeys {
    /// Retain four optional external keys under a combined 4096-byte metadata limit.
    /// # Errors
    /// Refuses excessive key storage before the operation is bound.
    pub fn new(
        plan: Option<String>,
        binding: Option<String>,
        point: Option<String>,
        node: Option<String>,
    ) -> Result<Self, SolveError> {
        let values = [plan, binding, point, node];
        let bytes = values
            .iter()
            .flatten()
            .try_fold(0_usize, |sum, value| sum.checked_add(value.len()))
            .ok_or(SolveError::ResourceLimit)?;
        if bytes > 4096 {
            return Err(SolveError::ResourceLimit);
        }
        Ok(Self { values })
    }
    /// Optional external plan identity.
    #[must_use]
    pub fn plan(&self) -> Option<&str> {
        self.values[0].as_deref()
    }
    /// Optional external binding identity.
    #[must_use]
    pub fn binding(&self) -> Option<&str> {
        self.values[1].as_deref()
    }
    /// Optional external point identity.
    #[must_use]
    pub fn point(&self) -> Option<&str> {
        self.values[2].as_deref()
    }
    /// Optional external node identity.
    #[must_use]
    pub fn node(&self) -> Option<&str> {
        self.values[3].as_deref()
    }
}
/// Final boundary reached by an executed solve request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveStage {
    /// Semantic, handle, coverage or reservation validation refused.
    Binding,
    /// Factor creation or dependency-specific reuse refused.
    Factorization,
    /// Candidate, certificate, statistical propagation or budget acceptance refused.
    Evaluation,
    /// Complete final storage/provenance publication refused.
    Publication,
    /// The complete result and companions published atomically.
    Published,
}
/// Historical failed solve request, including failures before an execution identity exists.
#[derive(Debug)]
pub struct SolveFailureRecord<P: Scalar> {
    /// Exact boundary that refused.
    stage: SolveStage,
    /// Original checked operation with all policies and external keys.
    operation: CheckedOperation,
    /// Every proposed operand locator, including an unresolved one if binding refused.
    inputs: Vec<Handle<Value, P>>,
    /// Exact typed final cause.
    cause: OperationError,
    /// Numerical record exists only after a validated execution began.
    numerical: Option<Box<Failure<P>>>,
    /// Complete initial numerical phase cost after successful binding.
    cost: u64,
}
impl<P: Scalar> SolveFailureRecord<P> {
    /// Final refusal boundary; no later phase is implied to have run.
    #[must_use]
    pub const fn stage(&self) -> SolveStage {
        self.stage
    }
    /// Checked request, including declared maps, budget, recovery and client identities.
    #[must_use]
    pub const fn operation(&self) -> &CheckedOperation {
        &self.operation
    }
    /// Proposed operation-ordered operands, without current-access authority.
    #[must_use]
    pub fn inputs(&self) -> &[Handle<Value, P>] {
        &self.inputs
    }
    /// Available numerical failure record, absent for a binding refusal.
    #[must_use]
    pub fn numerical(&self) -> Option<&Failure<P>> {
        self.numerical.as_deref()
    }
    /// Conservative cost of one complete numerical phase, zero before binding succeeds.
    #[must_use]
    pub const fn attempt_work(&self) -> u64 {
        self.cost
    }
}
/// Failed complete solve lifecycle with no candidate or published solution handle.
#[derive(Debug)]
pub struct SolveFailure<P: Scalar> {
    /// Boxed immutable facts keep the ordinary result error small.
    pub(crate) record: Box<SolveFailureRecord<P>>,
}
impl<P: Scalar> std::fmt::Display for SolveFailure<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.record.cause, formatter)
    }
}
impl<P: Scalar> std::error::Error for SolveFailure<P> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.record.cause)
    }
}
impl<P: Scalar> SolveFailure<P> {
    /// Exact typed refusal, preserving its original cause chain.
    #[must_use]
    pub const fn cause(&self) -> &OperationError {
        &self.record.cause
    }
    /// Historical request and final lifecycle state.
    #[must_use]
    pub const fn record(&self) -> &SolveFailureRecord<P> {
        &self.record
    }
}
/// Retain request metadata while each exclusive numerical lease advances.
struct Request<P: Scalar> {
    /// Checked original recipe and explicit policy.
    operation: CheckedOperation,
    /// Proposed concrete inputs, independent of whether all resolve.
    inputs: Vec<Handle<Value, P>>,
    /// Admitted complete numerical phase cost.
    cost: u64,
}
impl<P: Scalar> Request<P> {
    /// Refusals copy only immutable metadata and retain any already valid numerical evidence.
    fn refuse(
        &self,
        stage: SolveStage,
        cause: OperationError,
        numerical: Option<Failure<P>>,
    ) -> SolveFailure<P> {
        SolveFailure {
            record: Box::new(SolveFailureRecord {
                stage,
                operation: self.operation.clone(),
                inputs: self.inputs.clone(),
                cause,
                numerical: numerical.map(Box::new),
                cost: self.cost,
            }),
        }
    }
    /// Numerical stages retain their complete generic failure record.
    fn numerical(&self, stage: SolveStage, failure: Failure<P>) -> SolveFailure<P> {
        self.refuse(stage, failure.cause().clone(), Some(failure))
    }
}
impl Heap {
    /// Execute the complete solve lifecycle, retaining binding refusals as well as numerical failures.
    /// # Errors
    /// Returns the exact failed phase, original request and any independently valid partial proof.
    pub fn solve_recorded<P: Scalar + 'static>(
        &mut self,
        binding: Binding<P>,
    ) -> Result<Outcome<Vec<PublishedBundle<P>>>, SolveFailure<P>> {
        recorded(self, binding, None)
    }
    /// Execute with a reusable factor after validating its exact operator key.
    /// # Errors
    /// Refuses changed operators, invalid bindings or failed certificates/publication with full history.
    pub fn solve_factored_recorded<P: Scalar + 'static>(
        &mut self,
        binding: Binding<P>,
        factor: &FactorRef<P>,
    ) -> Result<Outcome<Vec<PublishedBundle<P>>>, SolveFailure<P>> {
        recorded(self, binding, Some(factor))
    }
}
/// Every started path returns either atomic publication or its precise failed boundary.
fn recorded<P: Scalar + 'static>(
    heap: &mut Heap,
    binding: Binding<P>,
    supplied: Option<&FactorRef<P>>,
) -> Result<Outcome<Vec<PublishedBundle<P>>>, SolveFailure<P>> {
    let mut request = Request {
        operation: binding.operation.clone(),
        inputs: binding.inputs.clone(),
        cost: 0,
    };
    if !matches!(request.operation.recipe, NativeRecipe::Solve(_)) {
        return Err(request.refuse(SolveStage::Binding, SolveError::Contract.into(), None));
    }
    let bound = binding
        .validate(heap)
        .map_err(|cause| request.refuse(SolveStage::Binding, cause, None))?;
    request.cost = bound
        .operation()
        .solve_work()
        .unwrap_or(0)
        .checked_add(bound.statistical_work())
        .and_then(|cost| u64::try_from(cost).ok())
        .ok_or_else(|| {
            request.refuse(SolveStage::Binding, SolveError::ResourceLimit.into(), None)
        })?;
    let generated = if supplied.is_none() {
        Some(
            bound
                .factor()
                .map_err(|failure| request.numerical(SolveStage::Factorization, failure))?,
        )
    } else {
        None
    };
    let factor = supplied.or(generated.as_ref()).ok_or_else(|| {
        request.refuse(SolveStage::Factorization, SolveError::Contract.into(), None)
    })?;
    bound
        .validate_factor(factor)
        .map_err(|failure| request.numerical(SolveStage::Factorization, failure))?;
    let views = bound
        .normalized_inputs()
        .map_err(|cause| request.refuse(SolveStage::Evaluation, cause, None))?;
    let candidate = bound
        .evaluate_factored(&views, factor)
        .map_err(|failure| request.numerical(SolveStage::Evaluation, failure))?;
    drop(views);
    bound
        .publish(candidate)
        .map_err(|failure| request.numerical(SolveStage::Publication, failure))
}
impl<P: Scalar + 'static, Q: Scalar + 'static> crate::Bound<'_, P, Q> {
    /// Check factor reuse against the actual operator without issuing a solve certificate.
    /// # Errors
    /// Refuses another recipe or a changed operator identity, version, selection or interpretation.
    pub fn validate_factor(&self, factor: &FactorRef<P>) -> Result<(), Failure<P>> {
        let result = match &self.operation.recipe {
            NativeRecipe::Solve(contract) => self
                .dependencies
                .first()
                .ok_or(OperationError::InputCount)
                .and_then(|input| factor.validate(input, contract)),
            _ => Err(SolveError::Contract.into()),
        };
        result.map_err(|cause| self.failure(cause, Vec::new()))
    }
}
impl crate::OperationRecord {
    /// A published solve's lifecycle state; other recipes return no solve state.
    #[must_use]
    pub fn solve_stage(&self) -> Option<SolveStage> {
        matches!(self.operation.recipe, NativeRecipe::Solve(_)).then_some(SolveStage::Published)
    }
    /// Historical solve proof, including factor reuse and recovery; use current heap access for authority.
    #[must_use]
    pub fn solve_evidence(&self) -> Option<&crate::SolveEvidence> {
        match self.evidence.first() {
            Some(RecipeEvidence::Solve(claim)) => Some(claim),
            _ => None,
        }
    }
}
impl CheckedOperation {
    /// Optional client identities for this solve request, unrelated to numerical cache eligibility.
    #[must_use]
    pub fn solve_record_keys(&self) -> Option<&SolveRecordKeys> {
        match &self.recipe {
            NativeRecipe::Solve(contract) => contract.policy.record_keys(),
            _ => None,
        }
    }
}
impl<P: Scalar> crate::FailureRecord<P> {
    /// Frozen input versions for an attempted numerical execution; no binding success is invented.
    #[must_use]
    pub fn inputs(&self) -> &[crate::InputSnapshot<P>] {
        &self.inputs
    }
}
