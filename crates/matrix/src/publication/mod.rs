// SPDX-License-Identifier: AGPL-3.0-only
//! Private candidates and consuming publication over a frozen checked binding.

mod commit;
mod evaluate;
mod fill;
mod retirement;

use crate::{ExecutionId, Extent, ValueStamp};
use nrz_scalar::Scalar;

/// Owned unpublished numerical scratch tied to one unique issued execution token.
///
/// Candidates cannot be cloned, resolved as values, or used as evidence-bearing handles.
#[derive(Debug)]
pub struct Candidate<P: Scalar> {
    /// Binding that granted the normalized inputs.
    pub(crate) execution: ExecutionId,
    /// Unique process-local candidate token, consumed by publication.
    pub(crate) token: u64,
    /// Exact native recipe revision used for evaluation.
    pub(crate) revision: u32,
    /// Complete private outputs in operation order.
    pub(crate) outputs: Vec<Payload<P>>,
    /// Independently established per-output recipe evidence.
    pub(crate) evidence: Vec<crate::RecipeEvidence>,
    /// Complete private statistical parameters, published in the same transaction.
    pub(crate) statistics: Vec<crate::statistics::MomentPayload<P>>,
}

impl<P: Scalar> Candidate<P> {
    /// Binding identity; it does not authorize publication by another binding.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Number of private output candidates, including those with companion regions.
    #[must_use]
    pub const fn output_count(&self) -> usize {
        self.outputs.len()
    }
}

/// Private candidate centers and complete supported companions, with future target version.
#[derive(Debug)]
pub(crate) struct Payload<P: Scalar> {
    /// Reserved logical extent.
    pub(crate) extent: Extent,
    /// Exact reserved output identity and version.
    pub(crate) target: ValueStamp,
    /// Unpublished column-ordered centers.
    pub(crate) centers: Vec<P>,
    /// Unpublished complete declared-bound region, if carried by the input.
    pub(crate) bounds: Option<Vec<P>>,
    /// Separate numerical evaluation radii, already included in total bounds.
    pub(crate) evaluation: Option<Vec<P>>,
    /// Three overlapping product derivation regions, when carried or computed.
    pub(crate) contributions: Option<[Vec<P>; 3]>,
}
