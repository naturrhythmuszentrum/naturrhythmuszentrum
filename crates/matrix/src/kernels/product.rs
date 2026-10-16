// SPDX-License-Identifier: AGPL-3.0-only
//! Matrix multiplication combines faer centers with independent nominal and interaction bounds.

use super::arithmetic::{Entry, finish};
use super::contract::{KernelContract, Recipe, regions, work};
use crate::binding::buffer;
use crate::contractions::{ContractionWork, product};
use crate::{
    CheckedShape, NormalizedView, OperationDraft, OperationError, RegionContract, ShapeLimits,
};
use nrz_scalar::{ArithmeticContext, Scalar};

/// One product entry with separate overlapping derivation contributions.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductEntry<P: Scalar> {
    /// Candidate center, complete radius and new evaluation error.
    pub(crate) result: Entry<P>,
    /// Ordered left/right linear terms and nonlinear interaction.
    pub(crate) contributions: [P; 3],
}

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
        Ok(super::draft::kernel(KernelContract {
            inputs: regions(vec![left, right])?,
            output: RegionContract::check(output)?,
            recipe: Recipe::Product,
            work: capacity,
        }))
    }
}

/// Each faer candidate is checked against an independent real dot-product enclosure.
pub(crate) fn evaluate<P: Scalar + 'static>(
    views: &[NormalizedView<'_, P>],
    context: &ArithmeticContext,
) -> Result<Vec<ProductEntry<P>>, OperationError> {
    let left = views.first().ok_or(OperationError::InputCount)?;
    let right = views.get(1).ok_or(OperationError::InputCount)?;
    let centers = super::backend_product::product(left, right)?;
    let rows = left.extent().rows();
    let terms = work(centers.len(), left.extent().columns())?;
    let mut allowance = ContractionWork::new(terms);
    let mut entries = buffer(centers.len())?;
    for (index, center) in centers.into_iter().enumerate() {
        let enclosed = product(
            left,
            right,
            index % rows,
            index / rows,
            &mut allowance,
            context,
        )?;
        let [first, second, interaction] = enclosed.contributions;
        let propagated = first
            .add(second, context)
            .and_then(|sum| sum.add(interaction, context))
            .map_err(super::KernelError::from)?;
        entries.push(ProductEntry {
            result: finish(center, enclosed.nominal, propagated, context)?,
            contributions: [first.get(), second.get(), interaction.get()],
        });
    }
    Ok(entries)
}
