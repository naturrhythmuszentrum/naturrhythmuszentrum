// SPDX-License-Identifier: AGPL-3.0-only
//! Complete constructor payloads for public typed source inspection.

use crate::domain_model::{AmbientDomain, Layer};
use crate::model::{Field, Operator, PredicateKind, Sort};
use crate::source_model::SourceFormKind;

/// Scalar or represented expression payload; dependencies are described separately.
#[derive(Clone, Debug)]
pub struct ExpressionSyntax {
    /// Exact mathematical result sort.
    pub sort: Sort,
    /// Exact operator, including literal values and declaration keys.
    pub operator: Operator,
    /// Mathematical interpretation layer.
    pub layer: Layer,
}

/// A declared test-space signature, with its domain supplied as a typed dependency.
#[derive(Clone, Debug)]
pub struct SpaceSyntax {
    /// Exact free coordinate key.
    pub coordinate: String,
    /// Exact coefficient field.
    pub field: Field,
    /// Declared test-function family.
    pub test_family: String,
}

/// Set constructor distinguishing otherwise identical dependency roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionConstructor {
    /// Whole ambient domain.
    Ambient,
    /// Empty region.
    Empty,
    /// Defined and true guard.
    GuardTrue,
    /// Defined and false guard.
    GuardFalse,
    /// Finite intersection.
    Meet,
    /// Finite union.
    Union,
    /// Relative set difference.
    Difference,
}

/// Every non-edge payload needed to reconstruct original typed syntax.
#[derive(Clone, Debug)]
pub enum SyntaxNode {
    /// Expression signature and exact operator.
    Expression(Box<ExpressionSyntax>),
    /// Exact predicate constructor.
    Condition(PredicateKind),
    /// Exact ambient domain and region constructor.
    Region(AmbientDomain, RegionConstructor),
    /// A region/value association.
    CaseArm,
    /// Boundary ambient domain; location is a separate dependency.
    Boundary(AmbientDomain),
    /// Exact declared space signature.
    Space(Box<SpaceSyntax>),
    /// Written source interpretation, retaining every clause dependency in order.
    SourceForm(SourceFormKind),
}
