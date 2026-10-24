// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh binary64 executions retain original input coverage and explicit conversion records.
//!
//! # Explicit precision recovery
//!
//! `Heap::solve_recovering` accepts a checked f32 solve binding. When its policy explicitly permits promotion and a recoverable numerical stop occurs, it widens the complete input bundle through the ordinary precision transaction and performs a fresh f64 factorization and solve. `RecoveredSolve` exposes the actual precision, the typed successful outcome, the initial failure and the conversion record. `RecoveryFailure` retains the original, conversion and promoted failures that exist. Attempts share the original finite allowance; conversion work is also charged. Widening preserves measurement, import and statistical parameter errors and cannot recover lost input information. A successful conversion can remain as an independently valid record when the later solve refuses; no unsuccessful solve publishes a solution handle.

use crate::binding::NativeRecipe;
use crate::{
    Binding, CheckedOperation, ExecutionLimits, Failure, Heap, OperationError, Outcome,
    PrecisionContract, PublishedBundle, RecipeEvidence, RecordId, SolveError,
};
use nrz_accuracy::{Limits, Promotion};
use nrz_scalar::{Precision, Scalar};

/// A successful bounded solve with an explicit scalar-format tag and retry history.
#[derive(Debug)]
pub struct RecoveredSolve {
    /// Exactly one published typed outcome.
    result: PrecisionResult,
    /// Original binary32 numerical refusal, when a wider solve was needed.
    initial: Option<crate::SolveFailure<f32>>,
    /// Separate atomic input conversion record, absent without promotion.
    conversion: Option<RecordId>,
}
/// Exactly one typed outcome from the successful precision execution.
#[derive(Debug)]
enum PrecisionResult {
    /// The original requested representation met its contract.
    Single(Outcome<Vec<PublishedBundle<f32>>>),
    /// A fresh wider execution met its own complete contract.
    Double(Outcome<Vec<PublishedBundle<f64>>>),
}
impl RecoveredSolve {
    /// Actual format of the published solve.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        match self.result {
            PrecisionResult::Single(_) => Precision::F32,
            PrecisionResult::Double(_) => Precision::F64,
        }
    }
    /// Original-format result, only when that execution succeeded.
    #[must_use]
    pub const fn single(&self) -> Option<&Outcome<Vec<PublishedBundle<f32>>>> {
        match &self.result {
            PrecisionResult::Single(result) => Some(result),
            PrecisionResult::Double(_) => None,
        }
    }
    /// Promoted result, only after explicit conversion and a fresh certificate.
    #[must_use]
    pub const fn double(&self) -> Option<&Outcome<Vec<PublishedBundle<f64>>>> {
        match &self.result {
            PrecisionResult::Double(result) => Some(result),
            PrecisionResult::Single(_) => None,
        }
    }
    /// Historical numerical failure that motivated promotion.
    #[must_use]
    pub fn initial_failure(&self) -> Option<&Failure<f32>> {
        self.initial
            .as_ref()
            .and_then(|failure| failure.record().numerical())
    }
    /// Original-to-widened dependency and error-companion record.
    #[must_use]
    pub const fn conversion_record(&self) -> Option<RecordId> {
        self.conversion
    }
}
/// A complete refused retry, retaining available numerical records and publishing no solve handle.
#[derive(Debug)]
pub struct RecoveryFailure {
    /// Exact final refusal; an unavailable bound never asserts actual inaccuracy.
    cause: OperationError,
    /// Original attempt's available certificates.
    initial: Option<crate::SolveFailure<f32>>,
    /// Conversion refusal, if widening could not publish its complete bundle.
    conversion_failure: Option<Box<Failure<f32>>>,
    /// Promoted attempt's available certificates.
    promoted: Option<crate::SolveFailure<f64>>,
    /// Successful conversion remains an independently valid historical operation.
    conversion: Option<RecordId>,
}
impl std::fmt::Display for RecoveryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.cause, formatter)
    }
}
impl std::error::Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
impl RecoveryFailure {
    /// Final typed refusal, including binding or conversion failures.
    #[must_use]
    pub const fn cause(&self) -> &OperationError {
        &self.cause
    }
    /// Original binary32 failure, absent for a binding refusal.
    #[must_use]
    pub fn initial_failure(&self) -> Option<&Failure<f32>> {
        self.initial
            .as_ref()
            .and_then(|failure| failure.record().numerical())
    }
    /// Failed widening transaction, if any.
    #[must_use]
    pub fn conversion_failure(&self) -> Option<&Failure<f32>> {
        self.conversion_failure.as_deref()
    }
    /// Failed binary64 execution, if promotion reached evaluation.
    #[must_use]
    pub fn promoted_failure(&self) -> Option<&Failure<f64>> {
        self.promoted
            .as_ref()
            .and_then(|failure| failure.record().numerical())
    }
    /// Successful input conversion, when a later solve failed.
    #[must_use]
    pub const fn conversion_record(&self) -> Option<RecordId> {
        self.conversion
    }
    /// Start a binding refusal without manufacturing an execution or partial proof.
    const fn binding(cause: OperationError) -> Self {
        Self {
            cause,
            initial: None,
            conversion_failure: None,
            promoted: None,
            conversion: None,
        }
    }
}

/// Every retry uses the complete recorded solve lifecycle, including binding refusals.
fn execute<P: Scalar + 'static>(
    heap: &mut Heap,
    binding: Binding<P>,
) -> Result<Outcome<Vec<PublishedBundle<P>>>, crate::SolveFailure<P>> {
    heap.solve_recorded(binding)
}
impl Heap {
    /// Run a binary32 solve, optionally widening its complete inputs and retrying in binary64.
    ///
    /// The solve policy must explicitly permit promotion. Both executions share finite
    /// cumulative solve-attempt and work limits; conversion work is charged as well.
    /// Widening creates new typed values and retains all old deterministic and statistical
    /// parameter errors. Each precision execution has its own factor and certificate.
    /// # Errors
    /// Returns binding, numerical, conversion or budget refusal with available history.
    /// Converted inputs may remain published after a later solve refusal; no failed
    /// candidate becomes a solution handle.
    pub fn solve_recovering(
        &mut self,
        binding: Binding<f32>,
    ) -> Result<RecoveredSolve, RecoveryFailure> {
        let operation = binding.operation.clone();
        let inputs = binding.inputs.clone();
        let execution_limits = binding.limits;
        let settings = match &operation.recipe {
            NativeRecipe::Solve(contract) => contract.policy.recovery(),
            _ => None,
        };
        let failed = match execute(self, binding) {
            Ok(result) => {
                return Ok(RecoveredSolve {
                    result: PrecisionResult::Single(result),
                    initial: None,
                    conversion: None,
                });
            }
            Err(failed) => failed,
        };
        let used = usage(&failed);
        let mut failure = RecoveryFailure::binding(failed.cause().clone());
        failure.initial = Some(failed);
        let Some((policy, limits)) = settings else {
            return Err(failure);
        };
        if policy.promotion() != Promotion::F32ToF64
            || failure.initial_failure().is_none()
            || !retryable(&failure.cause)
        {
            return Err(failure);
        }
        promote(
            self,
            operation,
            inputs,
            execution_limits,
            limits,
            used,
            failure,
        )
    }
}
/// Read charged counters rather than inferring successful progress from the last iterate.
fn usage(failed: &crate::SolveFailure<f32>) -> (u32, u64) {
    if let Some(failure) = failed.record().numerical() {
        for claim in failure.record().historical_evidence() {
            if let RecipeEvidence::Solve(solve) = claim
                && let Some(recovery) = solve.recovery()
            {
                return (recovery.reserved_attempts(), recovery.work());
            }
        }
        return (1, failed.record().attempt_work());
    }
    (0, 0)
}
/// Only numerical recovery stops permit promotion; malformed source/binding contracts do not.
const fn retryable(cause: &OperationError) -> bool {
    matches!(
        cause,
        OperationError::Solve(
            SolveError::RefinementLimit
                | SolveError::Stagnation
                | SolveError::CandidateFailure
                | SolveError::CannotCertifyInverse
                | SolveError::Arithmetic(_)
        )
    )
}
/// Keep cumulative allowances through widening, then install the remaining budget on a fresh solve.
fn promote(
    heap: &mut Heap,
    mut operation: CheckedOperation,
    inputs: Vec<crate::Handle<crate::Value, f32>>,
    execution_limits: ExecutionLimits,
    limits: Limits,
    used: (u32, u64),
    mut failure: RecoveryFailure,
) -> Result<RecoveredSolve, RecoveryFailure> {
    let attempts = limits
        .max_attempts()
        .checked_sub(used.0)
        .filter(|value| *value > 0);
    let remaining = limits.max_work().checked_sub(used.1);
    let (Some(attempts), Some(remaining)) = (attempts, remaining) else {
        failure.cause = SolveError::ResourceLimit.into();
        return Err(failure);
    };
    let converted = widen(
        heap,
        &operation,
        inputs,
        execution_limits,
        remaining,
        &mut failure,
    )?;
    let (handles, record, conversion_work) = converted;
    failure.conversion = Some(record);
    let NativeRecipe::Solve(contract) = &mut operation.recipe else {
        failure.cause = SolveError::Contract.into();
        return Err(failure);
    };
    let Some((policy, _)) = contract.policy.recovery() else {
        failure.cause = SolveError::Contract.into();
        return Err(failure);
    };
    std::rc::Rc::make_mut(contract).policy = contract
        .policy
        .clone()
        .with_recovery(policy, Limits::new(attempts, remaining - conversion_work));
    match execute(heap, Binding::new(operation, handles, execution_limits)) {
        Ok(result) => Ok(RecoveredSolve {
            result: PrecisionResult::Double(result),
            initial: failure.initial,
            conversion: Some(record),
        }),
        Err(failed) => {
            failure.cause = failed.cause().clone();
            failure.promoted = Some(failed);
            Err(failure)
        }
    }
}
/// Explicit conversion uses the same complete target/parameter transaction as ordinary callers.
fn widen(
    heap: &mut Heap,
    operation: &CheckedOperation,
    inputs: Vec<crate::Handle<crate::Value, f32>>,
    limits: ExecutionLimits,
    allowance: u64,
    failure: &mut RecoveryFailure,
) -> Result<WidenedInputs, RecoveryFailure> {
    let result = widen_checked(heap, operation, inputs, limits, allowance);
    match result {
        Ok(result) => Ok(result),
        Err((cause, numerical)) => {
            failure.cause = cause;
            failure.conversion_failure = numerical;
            Err(std::mem::replace(
                failure,
                RecoveryFailure::binding(SolveError::Contract.into()),
            ))
        }
    }
}
/// Charge conversion work before evaluation and retain the conversion failure record itself.
fn widen_checked(
    heap: &mut Heap,
    operation: &CheckedOperation,
    inputs: Vec<crate::Handle<crate::Value, f32>>,
    limits: ExecutionLimits,
    allowance: u64,
) -> Result<WidenedInputs, WideningFailure> {
    let contracts = operation
        .inputs()
        .iter()
        .map(|input| PrecisionContract::new(input.shape().clone(), input.descriptor()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|cause| (cause, None))?;
    let entries = operation
        .inputs()
        .iter()
        .try_fold(0_usize, |sum, input| {
            sum.checked_add(input.shape().extent().entries())
        })
        .ok_or_else(|| (OperationError::from(SolveError::ResourceLimit), None))?;
    let conversion = crate::OperationDraft::precision(contracts, Precision::F32, Precision::F64)
        .check(limits)
        .map_err(|cause| (cause, None))?;
    let bound = Binding::<f32, f64>::convert_precision(conversion, inputs, limits)
        .validate(heap)
        .map_err(|cause| (cause, None))?;
    let cost = entries
        .checked_mul(16)
        .and_then(|count| count.checked_add(bound.statistical_work()))
        .and_then(|count| u64::try_from(count).ok())
        .filter(|cost| *cost <= allowance)
        .ok_or_else(|| (OperationError::from(SolveError::ResourceLimit), None))?;
    let views = bound.normalized_inputs().map_err(|cause| (cause, None))?;
    let candidate = bound
        .evaluate(views)
        .map_err(|failure| (failure.cause().clone(), Some(Box::new(failure))))?;
    let outcome = bound
        .publish(candidate)
        .map_err(|failure| (failure.cause().clone(), Some(Box::new(failure))))?;
    Ok((
        outcome
            .output()
            .iter()
            .map(|bundle| bundle.value())
            .collect(),
        outcome.record(),
        cost,
    ))
}

/// Converted input handles with their independent publication and charged work.
type WidenedInputs = (Vec<crate::Handle<crate::Value, f64>>, RecordId, u64);
/// Exact widening cause and its numerical record when evaluation began.
type WideningFailure = (OperationError, Option<Box<Failure<f32>>>);
impl RecoveredSolve {
    /// Complete original request and failed phase, retained even after successful promotion.
    #[must_use]
    pub fn initial_record(&self) -> Option<&crate::SolveFailureRecord<f32>> {
        self.initial.as_ref().map(crate::SolveFailure::record)
    }
}
impl RecoveryFailure {
    /// Complete original request, including a binding refusal with unresolved operand locators.
    #[must_use]
    pub fn initial_record(&self) -> Option<&crate::SolveFailureRecord<f32>> {
        self.initial.as_ref().map(crate::SolveFailure::record)
    }
    /// Complete promoted request and final phase, even when its binding refused.
    #[must_use]
    pub fn promoted_record(&self) -> Option<&crate::SolveFailureRecord<f64>> {
        self.promoted.as_ref().map(crate::SolveFailure::record)
    }
}
