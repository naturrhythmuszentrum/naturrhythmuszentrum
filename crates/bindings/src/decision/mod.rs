// SPDX-License-Identifier: AGPL-3.0-only
//! Storage-neutral certified Integer admission for closed, physically checked expressions.

mod case;
mod expression;
mod snapshot;
mod storage;

pub use case::{DecisionAttachment, IntegerDecisionCase, IntegerDecisionSpec};
pub use expression::DecisionExpression;
pub use snapshot::SelectedDecision;

/// Exact admission refusals; executable count capacity remains a separate operation.
#[derive(Debug, thiserror::Error)]
pub enum DecisionError {
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
