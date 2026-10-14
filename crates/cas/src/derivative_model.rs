// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed, independently replayed calculus results with explicit incomplete coverage.

use crate::graph::Handle;
use crate::proof_model::{Statement, VerifiedWitness};

/// Typed collections never infer mathematical nonexistence from missing work.
#[derive(Clone, Debug)]
pub enum ResultPayload {
    /// Replayed prefix before enough geometry was established to construct a map.
    Facts(Vec<VerifiedWitness>),
    /// Whole-domain derivative facts and their exact completeness evidence.
    DerivativeMap(Box<DerivativeMap>),
    /// Independently requested facts at a specified point.
    Boundary(Box<BoundaryFacts>),
}

/// An immutable map whose facts have all survived fresh-store replay.
#[derive(Clone, Debug)]
pub struct DerivativeMap {
    /// Original function, never replaced by a branch or previous derivative.
    pub(crate) function: Handle,
    /// Requested domain.
    pub(crate) domain: Handle,
    /// Requested coordinate.
    pub(crate) coordinate: String,
    /// Requested positive order.
    pub(crate) order: u32,
    /// Complete selector partition.
    pub(crate) partition: VerifiedWitness,
    /// Replayed open-cell and point facts, including lower-order prerequisites.
    pub(crate) facts: Vec<VerifiedWitness>,
    /// Separately proved derivative on the union of successful requested-order cells.
    pub(crate) defined: Option<VerifiedWitness>,
    /// Exact cells whose requested derivative remains unresolved.
    pub(crate) pending: Vec<PendingCell>,
    /// Present only after complete assembly of the requested derivative.
    pub(crate) complete: Option<VerifiedWitness>,
}

impl DerivativeMap {
    /// Original input.
    #[must_use]
    pub const fn function(&self) -> &Handle {
        &self.function
    }
    /// Exact requested region.
    #[must_use]
    pub const fn domain(&self) -> &Handle {
        &self.domain
    }
    /// Declared coordinate.
    #[must_use]
    pub fn coordinate(&self) -> &str {
        &self.coordinate
    }
    /// Requested derivative order.
    #[must_use]
    pub const fn order(&self) -> u32 {
        self.order
    }
    /// Replayed completeness certificate for the finite selector geometry.
    #[must_use]
    pub const fn partition(&self) -> &VerifiedWitness {
        &self.partition
    }
    /// Every retained mathematical fact, with its own sealed evidence.
    #[must_use]
    pub fn facts(&self) -> &[VerifiedWitness] {
        &self.facts
    }
    /// Derivative theorem on the proved derivative-defined region, which may be partial.
    #[must_use]
    pub const fn defined(&self) -> Option<&VerifiedWitness> {
        self.defined.as_ref()
    }
    /// Cells that still need a mathematical premise or further resources.
    #[must_use]
    pub fn pending(&self) -> &[PendingCell] {
        &self.pending
    }
    /// Complete derivative theorem, absent for mixed or unresolved coverage.
    #[must_use]
    pub const fn complete(&self) -> Option<&VerifiedWitness> {
        self.complete.as_ref()
    }
}

/// Immutable requested boundary facts; failure to obtain one leaves a pending outcome.
#[derive(Clone, Debug)]
pub struct BoundaryFacts {
    /// Function identity of the request.
    pub(crate) function: Handle,
    /// Exact requested point.
    pub(crate) boundary: Handle,
    /// Each retained theorem was independently replayed.
    pub(crate) facts: Vec<VerifiedWitness>,
}

impl BoundaryFacts {
    /// Function whose point was requested.
    #[must_use]
    pub const fn function(&self) -> &Handle {
        &self.function
    }
    /// Requested boundary object.
    #[must_use]
    pub const fn boundary(&self) -> &Handle {
        &self.boundary
    }
    /// Replayed ownership, jet, jump and point derivative evidence.
    #[must_use]
    pub fn facts(&self) -> &[VerifiedWitness] {
        &self.facts
    }
}

impl ResultPayload {
    /// All retained facts, including prerequisites, remain individually inspectable.
    pub(crate) fn facts(&self) -> &[VerifiedWitness] {
        match self {
            Self::DerivativeMap(value) => &value.facts,
            Self::Boundary(value) => &value.facts,
            Self::Facts(values) => values,
        }
    }

    /// Classify solely from checked statements after successful request completion.
    pub(crate) fn nonexistence(&self) -> bool {
        self.facts().iter().any(|fact| {
            matches!(
                fact.judgment().statement,
                Statement::DerivativeDoesNotExist(_)
            )
        })
    }

    /// A mixed whole-domain map cannot be labelled a complete derivative theorem.
    pub(crate) fn partial(&self) -> bool {
        matches!(self, Self::DerivativeMap(value) if value.complete.is_none())
    }

    /// Conditions on every component survive the collection boundary.
    pub(crate) fn discharged(&self) -> bool {
        self.facts().iter().all(VerifiedWitness::is_discharged)
    }

    /// Encode every sealed theorem rather than exporting only the primary witness.
    pub(crate) fn encode(
        &self,
        out: &mut crate::encoding::Encoder<'_>,
    ) -> Result<(), crate::error::Fault> {
        out.byte(match self {
            Self::DerivativeMap(_) => 0,
            Self::Boundary(_) => 1,
            Self::Facts(_) => 2,
        })?;
        self.metadata(out)?;
        out.count(self.facts().len())?;
        for fact in self.facts() {
            out.count(fact.bytes.len())?;
            out.raw(&fact.bytes)?;
        }
        Ok(())
    }
}

/// One unresolved cell; its blocker carries no mathematical nonexistence assertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingCell {
    /// Exact region from the complete partition.
    pub(crate) region: Handle,
    /// Positive derivative order being attempted.
    pub(crate) order: u32,
    /// Structured missing-premise or resource reason.
    pub(crate) blocker: crate::error::Fault,
}

impl PendingCell {
    /// Exact unresolved region.
    #[must_use]
    pub const fn region(&self) -> &Handle {
        &self.region
    }
    /// Attempted derivative order.
    #[must_use]
    pub const fn order(&self) -> u32 {
        self.order
    }
    /// Reason the producer stopped.
    #[must_use]
    pub const fn blocker(&self) -> &crate::error::Fault {
        &self.blocker
    }
}

impl ResultPayload {
    /// Response metadata keeps requested scope separate from the portable theorem bytes.
    fn metadata(&self, out: &mut crate::encoding::Encoder<'_>) -> Result<(), crate::error::Fault> {
        match self {
            Self::DerivativeMap(value) => {
                crate::graph::encode_id(value.function.logical_id(), out)?;
                crate::graph::encode_id(value.domain.logical_id(), out)?;
                out.name(&value.coordinate)?;
                out.unsigned(u64::from(value.order))?;
                out.byte(u8::from(value.complete.is_some()))?;
                out.byte(u8::from(value.defined.is_some()))?;
                out.count(value.pending.len())?;
                for cell in &value.pending {
                    crate::graph::encode_id(cell.region.logical_id(), out)?;
                    out.unsigned(u64::from(cell.order))?;
                }
            }
            Self::Boundary(value) => {
                crate::graph::encode_id(value.function.logical_id(), out)?;
                crate::graph::encode_id(value.boundary.logical_id(), out)?;
            }
            Self::Facts(_) => {}
        }
        Ok(())
    }
}
