// SPDX-License-Identifier: AGPL-3.0-only
//! Storage-neutral certified Integer admission for closed, physically checked expressions.

mod association;
mod case;
mod count;
mod expression;
mod lattice;
mod snapshot;
mod storage;

pub use association::{DecisionInputs, DecisionOperand};
pub use case::{DecisionAttachment, IntegerDecisionCase, IntegerDecisionSpec};
pub use count::{CountDescriptor, CountEvidence};
pub use expression::{DecisionCoordinate, DecisionExpression};
pub use lattice::{
    LatticeAttachment, LatticeAxis, LatticeDecisionCase, LatticeDecisionSpec, LatticeIndexKind,
    LatticeInputs, LatticeMetric, LatticePreparationUsage,
};
pub use snapshot::SelectedDecision;

/// Exact admission refusals; executable count capacity remains a separate operation.
#[derive(Debug, thiserror::Error)]
pub enum DecisionError {
    /// The original dependency is outside the supported closed constant subset.
    #[error("decision source has no closed constant input association")]
    Source,
    /// Numerical symbols do not match the sealed original input association.
    #[error("decision input association does not match")]
    Association,
    /// Storage-neutral numerical symbol declaration refused.
    #[error("decision input symbols: {0}")]
    Symbols(#[from] crate::AdmissionError),
    /// The original closed mathematical target does not match the physical expression.
    #[error("decision target does not match the bound expression")]
    Target,
    /// The certified family, context or original enclosure does not match the declared case.
    #[error("decision witness does not match the declared case")]
    Case,
    /// An attachment belongs to another source, map, case or parameter instance.
    #[error("decision attachment belongs to another declaration")]
    Attachment,
    /// The supported parameter instance requires exact closed Integer literals.
    #[error("decision parameter is not a closed Integer literal")]
    Parameter,
    /// A physical count needs an explicit counted kind and its domain.
    #[error("decision output requires a declared counted kind")]
    CountKind,
    /// A physical lattice needs complete ordered axes, columns and index meanings.
    #[error("physical lattice shape does not match")]
    LatticeShape,
    /// Original physical coordinates do not restore the declared mathematical entries.
    #[error("physical lattice coordinate correspondence does not match")]
    LatticeMeaning,
    /// A discrete coordinate needs an explicitly declared index or nonnegative counted kind.
    #[error("physical lattice index kind is not admitted")]
    LatticeIndexKind,
    /// Cell convention and normalized coordinate metric must be declared together.
    #[error("physical lattice metric does not match the convention")]
    LatticeMetric,
    /// Existing physical reading and restoration checks refused.
    #[error("physical lattice reading: {0}")]
    Reading(#[from] crate::readings::ReadingError),
    /// The bounded retained metadata cannot be represented.
    #[error("decision metadata capacity exhausted")]
    Capacity,
    /// Physical descriptor checking refused.
    #[error("decision units: {0}")]
    Units(#[from] crate::UnitError),
    /// Original declaration checking refused.
    #[error("decision declaration: {0}")]
    Declaration(#[from] crate::Diagnostic),
    /// Exact CAS construction, authentication or caller resources refused.
    #[error("decision algebra: {0}")]
    Algebra(#[from] nrz_cas::Fault),
}
