// SPDX-License-Identifier: AGPL-3.0-only
//! Preflight the complete statistical phase before borrowing numerical views.

use super::{StatisticsError, capture::Input, store::Lineage};
use crate::binding::NativeRecipe;
use crate::{CheckedOperation, ExecutionLimits, Handle, Heap, JointLimits, OperationError, Value};
use nrz_scalar::{ArithmeticContext, Scalar};

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
    /// Original statistical allocation plan, absent only for source-free operations.
    pub(crate) inventory: Option<super::inventory::Run>,
}
impl<P: Scalar + 'static> Plan<P> {
    /// Replace unconditional refusal with complete source, recipe and work validation.
    pub(crate) fn capture<Q: Scalar + 'static>(
        heap: &Heap,
        operation: &CheckedOperation,
        handles: &[Handle<Value, P>],
        limits: ExecutionLimits,
        trace: &crate::execution::Trace,
        budget: &nrz_bindings::Budget,
    ) -> Result<Self, OperationError> {
        let joint = JointLimits::new(limits.scratch, limits.scratch, limits.work);
        let mut result = Self {
            inputs: Vec::new(),
            lineage: None,
            work: 0,
            limits: joint,
            points: Vec::new(),
            derivative: None,
            inventory: None,
        };
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
            return Ok(result);
        }
        if !operation.carries_statistics() {
            return Err(crate::HeapError::MissingCoverage.into());
        }
        let context = ArithmeticContext::new()?;
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
        result.derivative = super::derivative::check(operation)?;
        let run = super::inventory::Run::new::<Q>(bindings, Some(operation), trace, budget)?;
        if !matches!(operation.recipe, NativeRecipe::Solve(_)) {
            run.backend_allowance(result.work, size_of::<Q>())?;
        }
        for slot in 0..handles.len() {
            result
                .inputs
                .push(super::capture::input(heap, slot, &run, &context)?);
        }
        super::structure::inputs(operation, &result.inputs, &context, joint, &run)?;
        result.points = super::operating::capture(heap, handles, &context, &run)?;
        result.inventory = Some(run);
        result.classify(operation, &context)?;
        result
            .inventory
            .as_ref()
            .ok_or(StatisticsError::Contract)?
            .law(
                result
                    .lineage
                    .as_ref()
                    .ok_or(StatisticsError::Contract)?
                    .assurance,
            );
        Ok(result)
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
    pub(crate) const fn statistical_recipe(&self) -> bool {
        if matches!(self.recipe, NativeRecipe::Reciprocal { .. }) {
            return false;
        }
        matches!(
            self.recipe,
            NativeRecipe::Copy
                | NativeRecipe::Solve(_)
                | NativeRecipe::Kernel(_)
                | NativeRecipe::Affine(_)
                | NativeRecipe::Precision { .. }
        )
    }
}

impl<P: Scalar, Q: Scalar> crate::Bound<'_, P, Q> {
    /// Conservative scalar work and scratch allowance charged for complete statistical propagation.
    #[must_use]
    pub const fn statistical_work(&self) -> usize {
        self.statistics.work
    }
}
