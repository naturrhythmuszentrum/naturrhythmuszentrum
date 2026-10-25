// SPDX-License-Identifier: AGPL-3.0-only
//! Exact source derivatives and versioned numerical operating points have separate lifetimes.

mod numeric;
mod program;

use super::{LiveBindingError, ReadingBinding};
use crate::{Heap, HeapError};
use nrz_bindings::{
    Budget,
    readings::{
        ReadingError, ReciprocalJacobian, SourceJacobianUsage,
        algebra::{Cas, Location, limits::BudgetKind},
    },
};
use nrz_scalar::{ArithmeticContext, Enclosure, LiteralLimits, Scalar};
use std::rc::Rc;

/// A refused derivative lowering or stale numerical evaluation.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ReciprocalDerivativeError {
    /// A selected reading or its complete version inventory is unavailable.
    #[error("derivative input: {0}")]
    Live(#[from] LiveBindingError),
    /// Complete exact physical evidence could not be admitted.
    #[error("derivative definition: {0}")]
    Reading(#[from] ReadingError),
    /// Caller cumulative work or exact inspection refused.
    #[error("derivative resources: {0}")]
    Exact(#[from] nrz_bindings::readings::algebra::Fault),
    /// Numerical storage shape or allocation refused.
    #[error("derivative storage: {0}")]
    Heap(#[from] HeapError),
    /// Outward arithmetic or exact literal conversion refused.
    #[error("derivative arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// Only original scalar identity readings and the checked rational grammar are executable.
    #[error("unsupported derivative reading or expression")]
    Unsupported,
    /// Original input identity or arithmetic revision differs.
    #[error("derivative identity mismatch")]
    Identity,
}

/// Shared exact derivative and finite numerical program prepared before any heap access.
#[derive(Clone, Debug)]
pub struct ReciprocalDerivativeContract(Rc<Contract>);

/// Immutable exact evidence and the completely lowered finite arithmetic program.
#[derive(Debug)]
struct Contract {
    /// Original physical and whole-family derivative evidence.
    exact: ReciprocalJacobian,
    /// Bounded syntax obtained only from the checked native Jacobian matrix.
    program: program::Program,
    /// Exact constant import policy.
    literals: LiteralLimits,
    /// Complete retained exact and program payload.
    bytes: u64,
    /// Complete cumulative snapshots around numerical-program lowering.
    usage: SourceJacobianUsage,
}

impl ReciprocalDerivativeContract {
    /// Lower checked operator-coefficient partials from their retained original graph.
    /// # Errors
    /// Refuses foreign evidence, unsupported rational syntax and cumulative limits.
    pub fn new(
        graph: &Cas,
        exact: ReciprocalJacobian,
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<Self, ReciprocalDerivativeError> {
        let before = budget.usage();
        let program =
            budget.with_retained_payload(exact.retained_payload_bytes(), AT, |budget| {
                program::compile(graph, &exact, budget)
            })?;
        let bytes = exact
            .retained_payload_bytes()
            .checked_add(program.bytes)
            .ok_or(HeapError::ExtentOverflow)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        Ok(Self(Rc::new(Contract {
            exact,
            program,
            literals,
            bytes,
            usage: SourceJacobianUsage {
                before,
                after: budget.usage(),
            },
        })))
    }
    /// Exact evidence shared by all compatible numerical uses.
    #[must_use]
    pub fn exact(&self) -> &ReciprocalJacobian {
        &self.0.exact
    }
    /// Cumulative caller usage before and after lowering.
    #[must_use]
    pub fn usage(&self) -> SourceJacobianUsage {
        self.0.usage
    }
}

/// Numerical derivative at one represented operating point with all selected-copy dependencies.
#[derive(Clone, Debug)]
pub struct ReciprocalDerivativeEvaluation<P: Scalar> {
    /// Shared original proof and coefficient program.
    contract: ReciprocalDerivativeContract,
    /// Every selected original reading, including fixed inputs and copy discrepancies.
    inputs: Vec<ReadingBinding<P>>,
    /// Represented original input centers; never exact symbolic premises.
    point: Vec<P>,
    /// Outward partial enclosures in logical column order.
    values: Vec<Enclosure<P>>,
    /// Scalar arithmetic revision used to enclose the coefficients and operations.
    revision: u32,
    /// Complete caller-ledger snapshots for this evaluation.
    usage: SourceJacobianUsage,
}

impl<P: Scalar + 'static> ReciprocalDerivativeEvaluation<P> {
    /// Existing retained allowance for point arrays and the shared original contract.
    pub(crate) fn native_payload_bytes(&self) -> Result<u64, HeapError> {
        let count = self
            .point
            .len()
            .checked_add(self.values.len())
            .ok_or(HeapError::ExtentOverflow)?;
        u64::try_from(count)
            .map_err(|_| HeapError::ExtentOverflow)?
            .checked_mul(256)
            .and_then(|bytes| bytes.checked_add(self.contract.0.bytes))
            .ok_or(HeapError::ExtentOverflow)
    }

    /// Numerical partials are lent only inside a binding that validated this exact point.
    pub(crate) fn native_values(&self) -> &[Enclosure<P>] {
        &self.values
    }

    /// Native admission authenticates each original selected reading under its exclusive lease.
    pub(crate) fn validate_native(
        &self,
        heap: &Heap,
        handles: &[crate::Handle<crate::Value, P>],
        context: &ArithmeticContext,
    ) -> Result<(), crate::OperationError> {
        if self.revision != context.revision() || self.inputs.len() != handles.len() {
            return Err(crate::StatisticsError::Contract.into());
        }
        for (input, handle) in self.inputs.iter().zip(handles) {
            let expected = input.selected();
            expected.validate(heap)?;
            let actual = heap
                .symbols
                .selection(handle.key)
                .ok_or(HeapError::MissingBinding)?;
            actual.validate(heap)?;
            if !Rc::ptr_eq(&actual.group, &expected.group) || actual.index != expected.index {
                return Err(crate::StatisticsError::Contract.into());
            }
        }
        Ok(())
    }

    /// Exact theorem, which remains valid when this numerical evaluation becomes stale.
    #[must_use]
    pub const fn contract(&self) -> &ReciprocalDerivativeContract {
        &self.contract
    }
    /// Original selected readings preserve every projection and per-copy discrepancy.
    #[must_use]
    pub fn inputs(&self) -> &[ReadingBinding<P>] {
        &self.inputs
    }
    /// Complete cumulative usage through this numerical evaluation.
    #[must_use]
    pub const fn usage(&self) -> SourceJacobianUsage {
        self.usage
    }
    /// Revalidate every selected version before exposing an evaluated derivative.
    /// # Errors
    /// Refuses a changed operating point, stale selected entry or arithmetic revision.
    pub fn values<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<&'a [Enclosure<P>], ReciprocalDerivativeError> {
        self.validate(heap, context, budget)?;
        Ok(&self.values)
    }
    /// Exact represented inputs at which the numerical derivative was enclosed.
    /// # Errors
    /// Refuses the same stale dependencies as `Self::values`.
    pub fn operating_point<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<&'a [P], ReciprocalDerivativeError> {
        self.validate(heap, context, budget)?;
        Ok(&self.point)
    }
    /// No equal replacement value can authorize an old captured entry version.
    fn validate(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<(), ReciprocalDerivativeError> {
        if self.revision != context.revision() {
            return Err(ReciprocalDerivativeError::Identity);
        }
        budget.work(
            u64::try_from(self.inputs.len()).map_err(|_| HeapError::ExtentOverflow)?,
            AT,
        )?;
        for input in &self.inputs {
            heap.validate_reading(input)?;
        }
        Ok(())
    }
}

impl Heap {
    /// Enclose the checked Jacobian at represented positive inputs under the caller ledger.
    ///
    /// Results cover numerical coefficient and arithmetic error at the represented point.
    /// They establish no covariance, independence or enclosure over an uncertain physical family.
    /// Every input, including a fixed direction, retains its original selected-copy lineage.
    /// # Errors
    /// Refuses stale or reordered readings, changed physical references, nonpositive inputs,
    /// unsupported reading maps and exhausted cumulative work or payload allowances.
    pub fn evaluate_reciprocal_derivative<P: Scalar + 'static>(
        &self,
        contract: &ReciprocalDerivativeContract,
        inputs: &[ReadingBinding<P>],
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<ReciprocalDerivativeEvaluation<P>, ReciprocalDerivativeError> {
        let before = budget.usage();
        let (point, values) = budget.with_retained_payload(contract.0.bytes, AT, |budget| {
            numeric::evaluate(self, contract, inputs, context, budget)
        })?;
        let count = point
            .len()
            .checked_add(values.len())
            .ok_or(HeapError::ExtentOverflow)?;
        let bytes = u64::try_from(count)
            .map_err(|_| HeapError::ExtentOverflow)?
            .checked_mul(256)
            .and_then(|value| value.checked_add(contract.0.bytes))
            .ok_or(HeapError::ExtentOverflow)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(ReciprocalDerivativeEvaluation {
            contract: contract.clone(),
            inputs: inputs.to_vec(),
            point,
            values,
            revision: context.revision(),
            usage: SourceJacobianUsage {
                before,
                after: budget.usage(),
            },
        })
    }
}

/// Shared location for program construction, numerical lowering and version checks.
const AT: Location = Location::new("reciprocal derivative");
