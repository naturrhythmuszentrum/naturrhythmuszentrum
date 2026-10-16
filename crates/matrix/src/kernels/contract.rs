// SPDX-License-Identifier: AGPL-3.0-only
//! Checked kernel metadata and evidence retain operation and arithmetic identity.

use super::{KernelError, LinearMap};
use crate::binding::{NativeRecipe, buffer};
use crate::{
    CheckedOperation, Coverage, EntryMap, ExecutionLimits, InputSnapshot, OperationError,
    RegionContract,
};
use nrz_scalar::{Precision, Scalar};
use std::rc::Rc;

/// The closed deterministic law used for a published output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelKind {
    /// Independent sum with both operand bounds retained.
    Add,
    /// Independent difference with both operand bounds retained.
    Subtract,
    /// Fixed source-free affine map, including coefficient representation error.
    LinearMap,
    /// Exact logical selection, duplication, reshape or dual transpose.
    TransformEntries,
}

/// A private construction contract for one output and its complete ordered operands.
#[derive(Clone, Debug)]
pub(crate) struct KernelContract {
    /// Checked coherent operand meanings.
    pub(crate) inputs: Rc<[RegionContract]>,
    /// Independently derived result meaning and extent.
    pub(crate) output: RegionContract,
    /// Closed numerical schedule, never a caller callback.
    pub(crate) recipe: Recipe,
    /// Conservative complete phase work and scratch admission count.
    pub(crate) work: usize,
}

/// Kernel schedules retain exact entry maps and immutable coefficient definitions.
#[derive(Clone, Debug)]
pub(crate) enum Recipe {
    /// An aligned binary operation; repeated handles retain multiplicity.
    Sum {
        /// Select subtraction rather than addition.
        subtract: bool,
        /// Right operand entries in left operand order.
        right: EntryMap,
    },
    /// Fixed matrix and offsets, with no uncertain coefficient operand hidden inside.
    Linear(Box<LinearMap>),
    /// Bit-preserving logical entry transformation.
    Transform(EntryMap),
}

impl KernelContract {
    /// Apply execution policy before constructing an executable sealed recipe.
    pub(crate) fn check(self, limits: ExecutionLimits) -> Result<CheckedOperation, OperationError> {
        limits.check(self.inputs.len(), self.work)?;
        Ok(CheckedOperation {
            inputs: Rc::clone(&self.inputs),
            outputs: vec![self.output.clone()].into(),
            recipe: NativeRecipe::Kernel(Box::new(self)),
            revision: 1,
        })
    }

    /// Identify the exact deterministic propagation rule.
    pub(crate) const fn kind(&self) -> KernelKind {
        match &self.recipe {
            Recipe::Sum {
                subtract: false, ..
            } => KernelKind::Add,
            Recipe::Sum { subtract: true, .. } => KernelKind::Subtract,
            Recipe::Linear(_) => KernelKind::LinearMap,
            Recipe::Transform(_) => KernelKind::TransformEntries,
        }
    }

    /// Preserve a single scope; mixed physical/represented declarations refuse explicitly.
    pub(crate) fn coverage<P: Scalar>(
        &self,
        inputs: &[InputSnapshot<P>],
    ) -> Result<Coverage, OperationError> {
        let coverage = inputs.first().ok_or(OperationError::InputCount)?.coverage;
        if inputs.len() != self.inputs.len()
            || inputs.iter().any(|input| input.coverage != coverage)
        {
            return Err(KernelError::CoverageMismatch.into());
        }
        Ok(coverage)
    }
}

/// Numerical proof facts for a complete deterministic output and its companion regions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelEvidence {
    /// Closed operation law, independently checked during publication.
    pub(crate) kind: KernelKind,
    /// Number of coordinates covered by each output region.
    pub(crate) entries: usize,
    /// Original deterministic input scope.
    pub(crate) coverage: Coverage,
    /// Candidate and verification format.
    pub(crate) precision: Precision,
    /// Scalar rounding proof revision.
    pub(crate) revision: u32,
    /// Charged upper bound for this complete evaluation phase.
    pub(crate) work: usize,
}

impl KernelEvidence {
    /// Deterministic operation that established this enclosure.
    #[must_use]
    pub const fn kind(&self) -> KernelKind {
        self.kind
    }
    /// Candidate and independent verifier precision.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        self.precision
    }
    /// Scalar arithmetic proof revision used during evaluation.
    #[must_use]
    pub const fn arithmetic_revision(&self) -> u32 {
        self.revision
    }
    /// Conservative admitted complete-phase work count.
    #[must_use]
    pub const fn work(&self) -> usize {
        self.work
    }
}

/// Checked arithmetic for phase accounting before any numerical allocation.
pub(crate) fn work(entries: usize, factor: usize) -> Result<usize, OperationError> {
    entries
        .checked_mul(factor)
        .ok_or(OperationError::ResourceLimit)
}

/// Collect coherent input contracts without granting numerical authority.
pub(crate) fn regions(
    shapes: Vec<crate::CheckedShape>,
) -> Result<Rc<[RegionContract]>, OperationError> {
    let mut inputs = buffer(shapes.len())?;
    for shape in shapes {
        inputs.push(RegionContract::check(shape)?);
    }
    Ok(inputs.into())
}

impl CheckedOperation {
    /// Exact ordered source closure of one output; copy siblings retain separate ranges.
    pub(crate) fn input_slots(
        &self,
        slot: usize,
    ) -> Result<std::ops::Range<usize>, crate::HeapError> {
        if slot >= self.outputs.len() {
            return Err(crate::HeapError::InvalidLayout);
        }
        if matches!(self.recipe, NativeRecipe::Kernel(_)) {
            return Ok(0..self.inputs.len());
        }
        let end = slot.checked_add(1).ok_or(crate::HeapError::Capacity)?;
        if end > self.inputs.len() {
            return Err(crate::HeapError::InvalidLayout);
        }
        Ok(slot..end)
    }
}
