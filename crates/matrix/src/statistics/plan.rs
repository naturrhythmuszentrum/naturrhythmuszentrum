// SPDX-License-Identifier: AGPL-3.0-only
//! Preflight the complete statistical phase before borrowing numerical views.

use super::{StatisticsError, capture::Input, store::Lineage};
use crate::binding::NativeRecipe;
use crate::{CheckedOperation, ExecutionLimits, Handle, Heap, JointLimits, OperationError, Value};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Preserve ordinary resource admission while quantity statistics debit their original caller.
pub(crate) enum PlanBudget<'a> {
    /// Existing binding clients provide immutable payload ceilings.
    Independent(&'a nrz_bindings::Budget),
    /// Quantity clients retain one mutable cumulative Work ledger.
    Quantity(&'a mut nrz_bindings::Budget),
}

impl PlanBudget<'_> {
    /// Borrow the same payload and policy ceilings without changing any counter.
    pub(crate) const fn resources(&self) -> &nrz_bindings::Budget {
        match self {
            Self::Independent(budget) => budget,
            Self::Quantity(budget) => budget,
        }
    }

    /// Reserve the conservative complete statistical phase before numerical capture.
    fn admit_quantity(&mut self, work: usize) -> Result<(), OperationError> {
        let Self::Quantity(budget) = self else {
            return Err(StatisticsError::Contract.into());
        };
        let work = u64::try_from(work).map_err(|_| StatisticsError::ResourceLimit)?;
        budget.work(
            work,
            nrz_bindings::readings::algebra::Location::new("quantity statistics"),
        )?;
        Ok(())
    }
}

/// One caller-prepared derivative owner for the selected closed operation.
#[derive(Clone, Debug)]
pub(crate) enum PreparedDerivative<P: Scalar> {
    /// Numerical reciprocal derivative already bound to its original sources.
    Reciprocal(std::rc::Rc<crate::ReciprocalDerivativeEvaluation<P>>),
    /// Exact quantity identity prepared on the caller's cumulative ledger.
    Quantity(std::rc::Rc<nrz_bindings::CheckedSensitivity>),
}

/// Frozen statistical inputs and checked work allowance for one execution.
#[derive(Debug)]
pub(crate) struct Plan<P: Scalar> {
    /// Empty only when no operand carries statistics.
    pub(crate) inputs: Vec<Input<P>>,
    /// Complete shared lineage, retaining model and source identities.
    pub(crate) lineage: Option<Lineage>,
    /// Conservative complete-phase scalar work and scratch admission count.
    pub(crate) work: usize,
    /// Numerical model access limits derived from the execution policy.
    pub(crate) limits: JointLimits,
    /// Captured represented operating points and their complete deterministic boxes.
    pub(crate) points: Vec<super::matrix::Matrix<P>>,
    /// Replayed bindings-owned native derivative relation, when required.
    pub(crate) derivative: Option<std::rc::Rc<nrz_bindings::CheckedSensitivity>>,
    /// Caller-owned numerical derivative held only during this validated execution.
    pub(crate) reciprocal: Option<std::rc::Rc<crate::ReciprocalDerivativeEvaluation<P>>>,
    /// Original statistical allocation plan, absent only for source-free operations.
    pub(crate) inventory: Option<super::inventory::Run>,
}
impl<P: Scalar + 'static> Plan<P> {
    /// Replace unconditional refusal with complete source, recipe and work validation.
    pub(crate) fn prepare<Q: Scalar + 'static>(
        heap: &Heap,
        operation: &CheckedOperation,
        handles: &[Handle<Value, P>],
        limits: ExecutionLimits,
        trace: &crate::execution::Trace,
        budget: &mut PlanBudget<'_>,
        prepared: Option<PreparedDerivative<P>>,
    ) -> Result<Self, OperationError> {
        let (reciprocal, quantity) = match prepared {
            Some(PreparedDerivative::Reciprocal(value)) => (Some(value), None),
            Some(PreparedDerivative::Quantity(value)) => (None, Some(value)),
            None => (None, None),
        };
        let joint = JointLimits::new(limits.scratch, limits.scratch, limits.work);
        let mut result = Self {
            inputs: Vec::new(),
            lineage: None,
            work: 0,
            limits: joint,
            points: Vec::new(),
            derivative: None,
            inventory: None,
            reciprocal,
        };
        super::reciprocal::validate(heap, operation, handles, result.reciprocal.as_deref())?;
        let mut present = false;
        for &handle in handles {
            present |= super::capture::present(heap, handle)?;
        }
        if !present
            && let NativeRecipe::Affine(contracts) = &operation.recipe
            && let Some(slot) = contracts
                .iter()
                .position(|contract| contract.mode == crate::ConversionMode::Statistical)
        {
            return Err(StatisticsError::MissingSource { slot, entry: 0 }.into());
        }
        if !present {
            if result.reciprocal.is_some() {
                return Err(StatisticsError::MissingSource { slot: 0, entry: 0 }.into());
            }
            return Ok(result);
        }
        if let NativeRecipe::Kernel(contract) = &operation.recipe
            && matches!(
                contract.recipe,
                crate::kernels::contract::Recipe::Quantity(_)
            )
            && quantity.is_none()
        {
            return Err(StatisticsError::NonlinearStatistics.into());
        }
        if !operation.carries_statistics() {
            return Err(crate::HeapError::MissingCoverage.into());
        }
        let mut bindings = crate::binding::buffer(handles.len())?;
        for (slot, &handle) in handles.iter().enumerate() {
            let binding = super::capture::inspect(heap, handle, slot, joint)?;
            result.merge(&binding.lineage)?;
            bindings.push(binding);
        }
        let lineage = result.lineage.as_ref().ok_or(StatisticsError::Contract)?;
        result.work = phase_work(operation, lineage)?;
        limits.check(handles.len(), result.work)?;
        lineage.model.require(&lineage.columns, joint)?;
        result.derivative = if let Some(proof) = quantity {
            super::quantity::validate(operation, &proof)?;
            budget.admit_quantity(result.work)?;
            Some(proof)
        } else {
            super::derivative::check(operation)?
        };
        let run =
            super::inventory::Run::new::<Q>(bindings, Some(operation), trace, budget.resources())?;
        if !matches!(operation.recipe, NativeRecipe::Solve(_)) {
            run.backend_allowance(result.work, size_of::<Q>())?;
        }
        result.inventory = Some(run);
        Ok(result)
    }
    /// Numerical capture starts only after the complete common physical plan is admitted.
    pub(crate) fn capture(
        &mut self,
        heap: &Heap,
        operation: &CheckedOperation,
        handles: &[Handle<Value, P>],
    ) -> Result<(), OperationError> {
        let Some(run) = &self.inventory else {
            return Ok(());
        };
        let context = ArithmeticContext::new()?;
        self.inputs = super::resources::owner_buffer(handles.len())?;
        for slot in 0..handles.len() {
            self.inputs
                .push(super::capture::input(heap, slot, run, &context)?);
        }
        super::structure::inputs(operation, &self.inputs, &context, self.limits, run)?;
        self.points = super::operating::capture(heap, handles, &context, run)?;
        self.record_law(operation, &context)
    }
    /// Record the classified law on the same immutable allocation inventory.
    fn record_law(
        &mut self,
        operation: &CheckedOperation,
        context: &ArithmeticContext,
    ) -> Result<(), OperationError> {
        self.classify(operation, context)?;
        let assurance = self
            .lineage
            .as_ref()
            .ok_or(StatisticsError::Contract)?
            .assurance;
        self.inventory
            .as_ref()
            .ok_or(StatisticsError::Contract)?
            .law(assurance);
        Ok(())
    }

    /// Require one model; union ordered source indices without inferring independence.
    fn merge(&mut self, incoming: &Lineage) -> Result<(), OperationError> {
        if let Some(lineage) = &mut self.lineage {
            if lineage.model.id != incoming.model.id {
                return Err(StatisticsError::MissingJointModel.into());
            }
            lineage.columns.extend_from_slice(&incoming.columns);
            lineage.columns.sort_unstable();
            lineage.columns.dedup();
            lineage.roots.extend_from_slice(&incoming.roots);
            lineage.roots.sort_unstable();
            lineage.roots.dedup();
            if incoming.assurance == super::MomentAssurance::LinearizedMoments {
                lineage.assurance = super::MomentAssurance::LinearizedMoments;
            }
        } else {
            self.lineage = Some(incoming.clone());
        }
        Ok(())
    }
}

/// Account both contraction stages, derivative composition, means and all retained regions.
fn phase_work(operation: &CheckedOperation, lineage: &Lineage) -> Result<usize, OperationError> {
    let sources = lineage.columns.len();
    let inputs = operation.inputs().iter().try_fold(0_usize, |sum, region| {
        add(sum, region.shape().extent().entries())
    })?;
    let mut total = product(&[sources, sources, lineage.model.terms, 8])?;
    for output in operation.outputs() {
        let entries = output.shape().extent().entries();
        total = add(total, product(&[entries, inputs, sources, 8])?)?;
        total = add(total, product(&[entries, sources, sources, 8])?)?;
        total = add(total, product(&[entries, entries, sources, 8])?)?;
        total = add(total, product(&[entries, inputs, 16])?)?;
        total = add(total, product(&[entries, entries, 8])?)?;
        total = add(total, product(&[entries, sources, 8])?)?;
        total = add(total, product(&[entries, 16])?)?;
    }
    if let Some(exponent) = operation.quantity_integer_exponent() {
        // Each bit admits integer construction and power multiplication, including interval endpoints.
        let bits = 64 - exponent.unsigned_abs().leading_zeros() as usize;
        total = add(total, product(&[bits + 1, 32])?)?;
    }
    if let NativeRecipe::Solve(contract) = &operation.recipe {
        total = add(total, crate::solve::sensitivity::work(contract)?)?;
    }
    Ok(total)
}
/// Checked phase addition cannot wrap into a smaller allowance.
fn add(left: usize, right: usize) -> Result<usize, OperationError> {
    left.checked_add(right)
        .ok_or_else(|| StatisticsError::ResourceLimit.into())
}
/// Check every dimension factor before any matrix allocation.
fn product(factors: &[usize]) -> Result<usize, OperationError> {
    factors.iter().try_fold(1_usize, |value, factor| {
        value
            .checked_mul(*factor)
            .ok_or_else(|| StatisticsError::ResourceLimit.into())
    })
}

impl CheckedOperation {
    /// Capability of the currently implemented closed statistical recipes.
    pub(crate) fn statistical_recipe(&self) -> bool {
        use crate::kernels::contract::Recipe;
        match &self.recipe {
            NativeRecipe::Reciprocal { .. } => false,
            NativeRecipe::Kernel(contract) => match &contract.recipe {
                Recipe::PreparedExpm1 => false,
                Recipe::Quantity(_) => matches!(
                    contract.kind(),
                    crate::KernelKind::ScalarQuantityProduct
                        | crate::KernelKind::ScalarQuantityQuotient
                        | crate::KernelKind::ScalarQuantityIntegerPower
                        | crate::KernelKind::ScalarQuantitySquareRoot
                ),
                Recipe::Sum { .. }
                | Recipe::Linear(_)
                | Recipe::Transform(_)
                | Recipe::Product
                | Recipe::Elementwise(_)
                | Recipe::PositiveReciprocal(_) => true,
            },
            NativeRecipe::Copy
            | NativeRecipe::Solve(_)
            | NativeRecipe::Affine(_)
            | NativeRecipe::Precision { .. }
            | NativeRecipe::ReciprocalStatistics { .. } => true,
        }
    }
}

impl<P: Scalar, Q: Scalar> crate::Bound<'_, P, Q> {
    /// Conservative scalar work and scratch allowance charged for complete statistical propagation.
    #[must_use]
    pub const fn statistical_work(&self) -> usize {
        self.statistics.work
    }
}
