// SPDX-License-Identifier: AGPL-3.0-only
//! Matrix multiplication combines faer centers with independent nominal and interaction bounds.
//!
//! # Verified products and overlapping contributions
//!
//! `OperationDraft::matmul` checks ordered matrix composition and publishes one result rectangle. Safe faer multiplication supplies candidate centers. A separate increasing-index outward contraction encloses each exact represented-center dot product and measures its displacement from that candidate; it does not assume faer's reduction order. The total is the outward sum of `abs(A) R_B`, `R_A abs(B)`, `R_A R_B` and the new evaluation error. Empty ratio contractions publish typed zero results after checking their physical pairing; unsupported scalar quotient meanings retain the existing typed refusal.
//!
//! `ProductTerm::RightInput`, `LeftInput` and `Interaction` name three separate companion regions exposed by `PublishedBundle::contribution`, `ValueView::contribution` and `Heap::selected_contribution`. They carry the result's radius descriptor and exact target identity/version. These derivation terms overlap the total; they are not additional radii to add to it. Copy and entry transformations retain each term in the same logical order. Further arithmetic consumes the prior total once and retains the earlier derivation in provenance. Reservations, scratch admission, dependency validation, replacement, retirement and compaction include all three regions. Statistical operands use the complete joint product linearization documented in `src/statistics/`.

use super::arithmetic::{Entry, finish};
use super::contract::work;
use crate::binding::buffer;
use crate::contractions::{ContractionWork, product};
use crate::{NormalizedView, OperationError};
use nrz_scalar::{ArithmeticContext, Scalar};

/// One product entry with separate overlapping derivation contributions.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductEntry<P: Scalar> {
    /// Candidate center, complete radius and new evaluation error.
    pub(crate) result: Entry<P>,
    /// Ordered left/right linear terms and nonlinear interaction.
    pub(crate) contributions: [P; 3],
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
