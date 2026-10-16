// SPDX-License-Identifier: AGPL-3.0-only
//! Semantic square solve admission precedes packing and backend arithmetic.

use super::{SolveError, SolvePolicy};
use crate::binding::NativeRecipe;
use crate::{
    CheckedOperation, CheckedShape, Coverage, ExecutionLimits, Handle, Heap, InputSnapshot,
    OperationDraft, OperationError, RegionContract, ShapeLimits, Value,
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Closed operation with independently checked result axes and scalar meaning.
#[derive(Clone, Debug)]
pub(crate) struct SolveContract {
    /// Operator followed by physical right-hand sides.
    pub(crate) inputs: Rc<[RegionContract]>,
    /// Concrete scalar admission of the derived solution envelope.
    pub(crate) output: RegionContract,
    /// Original homogeneous solution coordinates before optional numerical restoration.
    pub(crate) solution: RegionContract,
    /// Typed identity on the equation axis for explicit inverse publication.
    pub(crate) identity: Option<CheckedShape>,
    /// Optional exact affine restoration with its own evaluation-error region.
    pub(crate) restoration: Option<crate::ConversionContract>,
    /// Named output-norm budget, checked before backend entry.
    pub(crate) policy: SolvePolicy,
    /// Conservative bound on numerical work and simultaneous scalar scratch.
    pub(crate) work: usize,
}

impl OperationDraft {
    /// Propose a positive square solve with an explicitly typed solution shape.
    ///
    /// `operator: X -> Y`, `rhs: W -> Y` and `output: W -> X` must retain the
    /// same ordered coordinate identities and declaration owner. The output supplies
    /// concrete scalar kinds for the formally derived solution quotients.
    /// # Errors
    /// Refuses empty/rectangular operators, absent RHS columns, incompatible axes,
    /// scalar kinds, unsupported homogeneous storage or exhausted shape limits.
    pub fn solve(
        operator: CheckedShape,
        rhs: CheckedShape,
        output: CheckedShape,
        limits: ShapeLimits,
    ) -> Result<Self, OperationError> {
        let derived = operator.solve(&rhs, limits)?;
        let output = result_contract(derived, output)?;
        let work = estimate_work(operator.extent().rows(), rhs.extent().columns())?;
        let inputs = vec![
            RegionContract::check(operator)?,
            RegionContract::check(rhs)?,
        ]
        .into();
        Ok(Self {
            solve: Some(Box::new(SolveContract {
                inputs,
                solution: output.clone(),
                output,
                work,
                identity: None,
                restoration: None,
                policy: SolvePolicy::default(),
            })),
            inputs: Vec::new(),
            kernel: None,
            conversions: None,
            precision: None,
        })
    }
}

impl SolveContract {
    /// Seal the numerical schedule only after its complete work count is admitted.
    pub(crate) fn check(
        self: Box<Self>,
        limits: ExecutionLimits,
    ) -> Result<CheckedOperation, OperationError> {
        self.policy.validate(&self.output)?;
        limits.check(self.inputs.len(), self.work)?;
        Ok(CheckedOperation {
            inputs: Rc::clone(&self.inputs),
            outputs: vec![self.output.clone()].into(),
            recipe: NativeRecipe::Solve(self),
            revision: 1,
        })
    }

    /// Admit carried statistics at the former refusal site; the joint plan checks every source.
    pub(crate) fn preflight<P: Scalar + 'static>(
        &self,
        heap: &Heap,
        inputs: &[Handle<Value, P>],
        limits: ExecutionLimits,
    ) -> Result<(), OperationError> {
        limits.check(inputs.len(), self.work)?;
        for &handle in inputs {
            match heap.refuse_joint_operation(handle) {
                Err(OperationError::Heap(crate::HeapError::MissingCoverage)) => {
                    heap.capture_joint_operation(handle)?;
                }
                result => result?,
            }
        }
        Ok(())
    }

    /// Compatible input totals cover both measurement and previous computational contributions once.
    pub(crate) fn coverage<P: Scalar>(
        &self,
        inputs: &[InputSnapshot<P>],
    ) -> Result<Coverage, OperationError> {
        if inputs.len() != self.inputs.len() {
            return Err(OperationError::InputCount);
        }
        let coverage = inputs.first().ok_or(OperationError::InputCount)?.coverage;
        for (slot, input) in inputs.iter().enumerate() {
            if input.coverage != coverage {
                return Err(SolveError::CoverageMismatch.into());
            }
            if coverage == Coverage::DeclaredBounds && input.companion().is_none() {
                return Err(SolveError::MissingCoverage { slot }.into());
            }
        }
        Ok(coverage)
    }
}

/// Retain the derived solve relation while admitting concrete output scalar kinds.
fn result_contract(
    mut derived: CheckedShape,
    declared: CheckedShape,
) -> Result<RegionContract, OperationError> {
    derived.same_snapshot(&declared)?;
    derived.domain().same_coordinates(declared.domain())?;
    derived.codomain().same_coordinates(declared.codomain())?;
    if derived.extent() != declared.extent() {
        return Err(SolveError::Contract.into());
    }
    for (entry, supplied) in derived.entries.iter_mut().zip(declared.entries()) {
        let quantity = supplied.scalar_descriptor().ok_or(SolveError::Contract)?;
        entry.conversion = Some(
            entry
                .pairing
                .check_entry(derived.declarations.registry(), quantity)
                .map_err(crate::ShapeError::from)?,
        );
        entry.scalar = Some(quantity.clone());
    }
    derived.parents.push(Rc::new(declared));
    RegionContract::check(derived)
}

/// Bound factorization, two solves, independent dot products, norms and private copies.
fn estimate_work(order: usize, columns: usize) -> Result<usize, OperationError> {
    order
        .checked_mul(order)
        .and_then(|square| {
            order
                .checked_add(columns)
                .and_then(|width| square.checked_mul(width))
        })
        .and_then(|terms| terms.checked_mul(128))
        .ok_or(OperationError::ResourceLimit)
}

impl CheckedOperation {
    /// Conservative solve work/scratch requirement, absent for another native recipe.
    #[must_use]
    pub const fn solve_work(&self) -> Option<usize> {
        match &self.recipe {
            NativeRecipe::Solve(contract) => Some(contract.work),
            _ => None,
        }
    }
}

impl OperationDraft {
    /// Propose an explicit inverse `A^-1: Y -> X` using a source-free typed identity on Y.
    ///
    /// Its published inverse has its own residual and total certificate. The private
    /// inverse used only to certify another solve never becomes this output.
    /// # Errors
    /// Refuses nonpositive or rectangular operators, incompatible output axes/kinds or limits.
    pub fn inverse(
        operator: CheckedShape,
        output: CheckedShape,
        limits: ShapeLimits,
    ) -> Result<Self, OperationError> {
        let derived = operator.inverse(limits)?;
        let output = result_contract(derived, output)?;
        let order = operator.extent().rows();
        let identity = CheckedShape::identity(
            operator.codomain(),
            Rc::clone(&operator.declarations),
            limits,
        )?;
        let inputs = vec![RegionContract::check(operator)?].into();
        Ok(Self {
            solve: Some(Box::new(SolveContract {
                inputs,
                solution: output.clone(),
                output,
                work: estimate_work(order, order)?,
                identity: Some(identity),
                restoration: None,
                policy: SolvePolicy::default(),
            })),
            inputs: Vec::new(),
            kernel: None,
            conversions: None,
            precision: None,
        })
    }

    /// Attach a local output-norm budget to this solve or inverse draft.
    /// # Errors
    /// Refuses a different recipe or incompatible output budget coordinates.
    pub fn with_solve_policy(mut self, policy: SolvePolicy) -> Result<Self, OperationError> {
        let contract = self.solve.as_mut().ok_or(SolveError::Contract)?;
        policy.validate(&contract.output)?;
        contract.policy = policy;
        Ok(self)
    }

    /// Restore the solution with a checked affine map in the same atomic publication.
    ///
    /// Bounds use only the absolute scale; an affine origin shifts centers only.
    /// Coefficient and evaluation errors are retained in both overlapping output views.
    /// # Errors
    /// Refuses a different recipe, repeated restoration, wrong source axes/descriptor,
    /// snapshot mismatch or a budget that no longer names the output coordinates.
    pub fn restore_solution(
        mut self,
        conversion: crate::ConversionContract,
    ) -> Result<Self, OperationError> {
        let contract = self.solve.as_mut().ok_or(SolveError::Contract)?;
        if contract.restoration.is_some()
            || conversion.input.descriptor() != contract.solution.descriptor()
        {
            return Err(SolveError::Contract.into());
        }
        let original = contract.solution.shape();
        original.same_snapshot(conversion.input.shape())?;
        original
            .domain()
            .same_coordinates(conversion.input.shape().domain())?;
        original
            .codomain()
            .same_coordinates(conversion.input.shape().codomain())?;
        contract.policy.validate(&conversion.output)?;
        contract.output = conversion.output.clone();
        contract.restoration = Some(conversion);
        Ok(self)
    }
}
