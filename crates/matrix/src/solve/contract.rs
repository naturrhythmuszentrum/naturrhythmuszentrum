// SPDX-License-Identifier: AGPL-3.0-only
//! Semantic square solve admission precedes packing and backend arithmetic.

use super::SolveError;
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
            solve: Some(SolveContract {
                inputs,
                output,
                work,
            }),
            inputs: Vec::new(),
            kernel: None,
            conversions: None,
            precision: None,
        })
    }
}

impl SolveContract {
    /// Seal the numerical schedule only after its complete work count is admitted.
    pub(crate) fn check(self, limits: ExecutionLimits) -> Result<CheckedOperation, OperationError> {
        limits.check(self.inputs.len(), self.work)?;
        Ok(CheckedOperation {
            inputs: Rc::clone(&self.inputs),
            outputs: vec![self.output.clone()].into(),
            recipe: NativeRecipe::Solve(Box::new(self)),
            revision: 1,
        })
    }

    /// Refuse statistical coverage explicitly before the generic snapshot capture.
    pub(crate) fn preflight<P: Scalar + 'static>(
        &self,
        heap: &Heap,
        inputs: &[Handle<Value, P>],
        limits: ExecutionLimits,
    ) -> Result<(), OperationError> {
        limits.check(inputs.len(), self.work)?;
        for (slot, &handle) in inputs.iter().enumerate() {
            match heap.refuse_joint_operation(handle) {
                Ok(()) => {}
                Err(OperationError::Heap(crate::HeapError::MissingCoverage)) => {
                    return Err(SolveError::StatisticalInput { slot }.into());
                }
                Err(cause) => return Err(cause),
            }
        }
        Ok(())
    }

    /// The first usable subset accepts only exact represented input coordinates.
    pub(crate) fn coverage<P: Scalar>(
        &self,
        inputs: &[InputSnapshot<P>],
    ) -> Result<Coverage, OperationError> {
        if inputs.len() != self.inputs.len() {
            return Err(OperationError::InputCount);
        }
        if inputs.iter().any(|input| {
            input.coverage != Coverage::RepresentedInputs || input.companion().is_some()
        }) {
            return Err(SolveError::BoundedInputsUnsupported.into());
        }
        Ok(Coverage::RepresentedInputs)
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
