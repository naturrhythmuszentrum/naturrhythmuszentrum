// SPDX-License-Identifier: AGPL-3.0-only
//! Original clause signatures, separate from unordered native case syntax.

use crate::graph::{LogicalId, Reference};
use crate::model::Scope;

/// Original form interpretation; both retain their exact argument order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFormKind {
    /// First reached, defined and true guard wins.
    OrderedCases,
    /// Explicit unordered mathematical associations with ordered source occurrences.
    CaseLiteral,
}

/// One original guard/value or region/value association.
#[derive(Clone, Debug)]
pub struct SourceClauseDraft {
    /// Guard expression for `OrderedCases`, region for `CaseLiteral`.
    pub selector: Reference,
    /// Original branch value expression.
    pub value: Reference,
}

/// A complete original form signature; it supplies no traversal receipt.
#[derive(Clone, Debug)]
pub struct SourceFormDraft {
    /// Actual semantic header.
    pub header: Reference,
    /// Explicit binder scope.
    pub scope: Scope,
    /// Ordered source interpretation.
    pub kind: SourceFormKind,
    /// Stated source domain.
    pub domain: Reference,
    /// Every clause in original source order.
    pub clauses: Vec<SourceClauseDraft>,
    /// Explicit otherwise value, never an invented default.
    pub otherwise: Option<Reference>,
}

/// Graph-owned original semantic signature without physical source authentication.
#[derive(Clone, Debug)]
pub(crate) struct SourceForm {
    /// Actual semantic header.
    pub(crate) header: LogicalId,
    /// Explicit binder scope.
    pub(crate) scope: Scope,
    /// Interpretation of selectors.
    pub(crate) kind: SourceFormKind,
    /// Stated domain.
    pub(crate) domain: LogicalId,
    /// Ordered selector/value dependencies.
    pub(crate) clauses: Vec<(LogicalId, LogicalId)>,
    /// Explicit otherwise value.
    pub(crate) otherwise: Option<LogicalId>,
}
