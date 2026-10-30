// SPDX-License-Identifier: AGPL-3.0-only
//! Private candidates and consuming publication over a frozen checked binding.
//!
//! # Candidate validation and atomic commit
//!
//! `Bound::evaluate` accepts only its own `NormalizedInputs` bundle and issues one unique candidate token after successful recipe evaluation. A `Candidate` owns unpublished centers and complete companions; it is neither cloneable nor a value handle. Repeated evaluation and foreign view bundles refuse. `Bound::publish` consumes the binding and candidate, independently rechecks every copied bit, finite bound, recipe revision, output extent, reserved identity/version and both directions of every companion target. The final `Outcome<Vec<PublishedBundle<P>>>` exposes all outputs together. No prefix becomes visible when a later output fails.
//!
//! Publication fills the already reserved private slabs and prepares all immutable graph records, source edges and append capacities before changing visible state. The commit only installs prepared storage, schemas, descriptors and graph objects through the existing no-fail primitives. It runs no allocation, caller callback or numerical kernel. Internal bounds target the reserved fresh output identities and versions in that same commit. Existing inputs and their versions remain unchanged on every refusal. Scratch consists of the reserved slab set and candidate payload set, each bounded by `ExecutionLimits`; temporary preparation copies and ordinary allocator aborts remain subject to the heap's documented allocation contract.

mod commit;
mod decision;
pub use decision::{
    FrozenRatioKernel, LiveDecisionCount, LiveDecisionError, PreparedDecision, PublishedDecision,
    RatioKernelEvidence,
};
mod decision_inputs;
pub use decision_inputs::{DecisionInputSnapshot, DecisionSnapshotError, FrozenDecisionInputs};
mod lattice;
pub use lattice::{
    LiveLatticeIndex, LiveLatticeRepresentative, PreparedLatticeDecision, PublishedLatticeDecision,
};
mod lattice_enclosure;
pub use lattice_enclosure::FrozenLatticeEnclosure;
mod lattice_inputs;
pub use lattice_inputs::FrozenLatticeInputs;
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
    /// Ticket authority follows the original candidate arrays until their destruction.
    pub(crate) private: Option<crate::private::Run>,
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
    pub(crate) centers: crate::private::Buffer<P>,
    /// Unpublished complete declared-bound region, if carried by the input.
    pub(crate) bounds: Option<crate::private::Buffer<P>>,
    /// Separate numerical evaluation radii, already included in total bounds.
    pub(crate) evaluation: Option<crate::private::Buffer<P>>,
    /// Three overlapping product derivation regions, when carried or computed.
    pub(crate) contributions: Option<[Vec<P>; 3]>,
}
