// SPDX-License-Identifier: AGPL-3.0-only
//! Public native constructors derive shape relations before execution policy is checked.

use super::linear::LinearMap;
use super::{KernelDraft, regions};
use crate::kernels::contract::{Recipe, work};
use crate::{
    Axis, AxisAlignment, CheckedShape, EntryMap, OperationDraft, OperationError, RegionContract,
    ShapeLimits,
};

impl OperationDraft {
    /// Propose an aligned sum of two homogeneous coherent difference regions.
    /// # Errors
    /// Refuses mismatched ordered axes, snapshots, kinds, structure or shape limits.
    pub fn add(
        left: CheckedShape,
        right: CheckedShape,
        limits: ShapeLimits,
    ) -> Result<Self, OperationError> {
        sum(left, right, false, limits)
    }

    /// Propose a difference, retaining both bounds even when the operands alias.
    /// # Errors
    /// Refuses mismatched ordered axes, snapshots, kinds, structure or shape limits.
    pub fn subtract(
        left: CheckedShape,
        right: CheckedShape,
        limits: ShapeLimits,
    ) -> Result<Self, OperationError> {
        sum(left, right, true, limits)
    }

    /// Propose a checked fixed linear or affine map over one value region.
    #[must_use]
    pub fn linear_map(map: LinearMap) -> Self {
        kernel(KernelDraft {
            reciprocal: None,
            inputs: vec![map.input.clone()].into(),
            output: map.output.clone(),
            work: map.work,
            recipe: Recipe::Linear(Box::new(map.schedule())),
        })
    }

    /// Propose a logical selection, reshape or duplication with explicitly checked axes.
    /// # Errors
    /// Refuses wrong source/target extents, incompatible new axes or shape limits.
    pub fn transform_entries(
        input: CheckedShape,
        domain: Axis,
        codomain: Axis,
        map: EntryMap,
        limits: ShapeLimits,
    ) -> Result<Self, OperationError> {
        let output = input.select(domain, codomain, map.clone(), limits)?;
        transform(input, output, map)
    }

    /// Propose the algebraic transpose with reciprocal dual axes and unchanged entry units.
    /// # Errors
    /// Refuses unsupported coordinate references, shapes or exhausted limits.
    pub fn dual_transpose(
        input: CheckedShape,
        limits: ShapeLimits,
    ) -> Result<Self, OperationError> {
        let output = input.dual_transpose(limits)?;
        let map = output
            .entry_maps()
            .first()
            .ok_or(OperationError::UnsupportedShape)?
            .clone();
        transform(input, output, map)
    }
}

/// Identity-order alignment still verifies the identities and physical meanings of both axes.
fn sum(
    left: CheckedShape,
    right: CheckedShape,
    subtract: bool,
    limits: ShapeLimits,
) -> Result<OperationDraft, OperationError> {
    let domain = AxisAlignment::identity(left.domain(), right.domain(), limits)?;
    let codomain = AxisAlignment::identity(left.codomain(), right.codomain(), limits)?;
    let output = left.sum(&right, &domain, &codomain, limits)?;
    let map = output
        .entry_maps()
        .first()
        .ok_or(OperationError::UnsupportedShape)?
        .clone();
    Ok(kernel(KernelDraft {
        reciprocal: None,
        work: work(output.extent().entries(), 8)?,
        output: RegionContract::check(output)?,
        inputs: regions(vec![left, right])?,
        recipe: Recipe::Sum {
            subtract,
            right: map,
        },
    }))
}

/// Exact entry maps retain one source dependency and allocate fresh complete result regions.
fn transform(
    input: CheckedShape,
    output: CheckedShape,
    map: EntryMap,
) -> Result<OperationDraft, OperationError> {
    Ok(kernel(KernelDraft {
        reciprocal: None,
        work: work(output.extent().entries(), 6)?,
        inputs: regions(vec![input])?,
        output: RegionContract::check(output)?,
        recipe: Recipe::Transform(map),
    }))
}

/// Fill the existing draft's closed recipe slot without exposing checked-state construction.
pub(crate) const fn kernel(contract: KernelDraft) -> OperationDraft {
    OperationDraft {
        inputs: Vec::new(),
        conversions: None,
        precision: None,
        kernel: Some(contract),
        solve: None,
    }
}
