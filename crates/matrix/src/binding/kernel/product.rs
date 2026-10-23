// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor admission prepares a numerical schedule after checking original meanings.

use super::{KernelDraft, regions};
use crate::kernels::contract::{Recipe, work};
use crate::{CheckedShape, OperationDraft, OperationError, RegionContract, ShapeLimits};

impl OperationDraft {
    /// Propose matrix multiplication with full deterministic interaction propagation.
    /// # Errors
    /// Refuses wrong ordered contraction axes, snapshots, scalar meanings or shape limits.
    pub fn matmul(
        left: CheckedShape,
        right: CheckedShape,
        limits: ShapeLimits,
    ) -> Result<Self, OperationError> {
        let output = super::linear::product_shape(&left, &right, limits)?;
        let terms = work(output.extent().entries(), left.extent().columns())?;
        let capacity = work(terms, 16)?
            .checked_add(work(output.extent().entries(), 12)?)
            .and_then(|count| count.checked_add(left.extent().entries().checked_mul(2)?))
            .and_then(|count| count.checked_add(right.extent().entries().checked_mul(2)?))
            .ok_or(OperationError::ResourceLimit)?;
        Ok(super::draft::kernel(KernelDraft {
            reciprocal: None,
            inputs: regions(vec![left, right])?,
            output: RegionContract::check(output)?,
            recipe: Recipe::Product,
            work: capacity,
        }))
    }
}
