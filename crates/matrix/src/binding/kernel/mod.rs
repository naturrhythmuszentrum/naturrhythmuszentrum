// SPDX-License-Identifier: AGPL-3.0-only
//! Physical kernel admission is retained beside a separate numerical execution contract.

mod draft;
pub(crate) mod linear;
mod nonlinear;
mod product;
mod reciprocal;

use super::{CheckedOperation, ExecutionLimits, NativeRecipe, OperationError, RegionContract};
use crate::kernels::contract::{KernelContract, Recipe};
use std::rc::Rc;

/// Descriptor-side construction evidence, discarded from the numerical contract.
#[derive(Clone, Debug)]
pub(crate) struct KernelDraft {
    /// Exact reciprocal correspondence remains outside the numerical kernel contract.
    pub(crate) reciprocal: Option<Rc<nrz_bindings::readings::PositiveReciprocalMap>>,
    /// Ordered bindings-admitted physical inputs.
    pub(crate) inputs: Rc<[RegionContract]>,
    /// Independently admitted output meaning and structure.
    pub(crate) output: RegionContract,
    /// Closed numerical law and coefficient definitions.
    pub(crate) recipe: Recipe,
    /// Unchanged numerical phase allowance.
    pub(crate) work: usize,
}

impl KernelDraft {
    /// Retain meanings in the operation while handing only numerical extents to its kernel.
    pub(crate) fn check(self, limits: ExecutionLimits) -> Result<CheckedOperation, OperationError> {
        limits.check(self.inputs.len(), self.work)?;
        let mut extents = super::buffer(self.inputs.len())?;
        extents.extend(self.inputs.iter().map(|input| input.shape().extent()));
        let numerical = KernelContract {
            inputs: extents.into(),
            output: self.output.shape().extent(),
            recipe: self.recipe,
            work: self.work,
        };
        let recipe = match self.reciprocal {
            Some(definition) => NativeRecipe::Reciprocal {
                kernel: Box::new(numerical),
                definition,
            },
            None => NativeRecipe::Kernel(Box::new(numerical)),
        };
        Ok(CheckedOperation {
            inputs: self.inputs,
            outputs: vec![self.output].into(),
            recipe,
            revision: 1,
        })
    }
}

/// Collect coherent input contracts without granting numerical authority.
pub(crate) fn regions(
    shapes: Vec<crate::CheckedShape>,
) -> Result<Rc<[RegionContract]>, OperationError> {
    let mut inputs = super::buffer(shapes.len())?;
    for shape in shapes {
        inputs.push(RegionContract::check(shape)?);
    }
    Ok(inputs.into())
}
