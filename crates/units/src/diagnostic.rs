// SPDX-License-Identifier: AGPL-3.0-only
//! Structured failures retain both source location and descriptor evidence.

use nrz_cas::{Fault, syntax::Span};

use crate::{Descriptor, UnitError};

/// An original operation's position, including any expanded declaration chain.
///
/// TODO ´todo:code:locate-axis-block-and-paired-covariance´: locate axis block and paired covariance indices alongside original expression paths under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:preserve-original-locations-when-clients-add´: preserve original locations when clients add binding point and node context under def:nrz:operation-record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    /// Zero-based operand indices from the current root.
    pub path: Vec<usize>,
    /// Optional parser byte range in the current declaration's source.
    pub span: Option<Span>,
    /// Declarations entered while reaching this node, in resolution order.
    pub definitions: Vec<String>,
}

/// A diagnostic expectation can be a concrete descriptor or an operation rule.
///
/// TODO ´todo:code:describe-expected-operator-and-companion-shapes´: describe expected operator and companion shapes including contraction coordinate meanings under def:nrz:matrix-shape and def:nrz:companion-shape.
///
/// TODO ´todo:code:identify-the-client-slot-contract-that´: identify the client slot contract that a bound descriptor violates under rule:nrz:binding-validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expectation {
    /// A caller or declaration supplied this exact descriptor constraint.
    Descriptor(Box<Descriptor>),
    /// The operation requires this named constraint.
    Rule(&'static str),
}

/// The reason a check could not establish its contract.
///
/// TODO ´todo:code:distinguish-malformed-extents-from-incompatible-axes´: distinguish malformed extents from incompatible axes and stale companion targets under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:distinguish-stale-checking-snapshots-from-physical´: distinguish stale checking snapshots from physical descriptor refusals at client boundaries under rule:nrz:binding-validation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Failure {
    /// Descriptor algebra refused the operation.
    #[error(transparent)]
    Units(#[from] UnitError),
    /// The original source does not follow the admitted expression grammar.
    #[error("invalid original expression syntax: {0:?}")]
    Syntax(nrz_cas::source::SyntaxCode),
    /// Generic expression construction or its resource limits failed.
    #[error(transparent)]
    Algebra(#[from] Fault),
    /// Expanded or converted coordinates exceeded their construction limits.
    #[error("coordinate construction failed: {0}")]
    Coordinates(#[source] Fault),
    /// A value name has no declaration.
    #[error("unknown value symbol: {}", .0.as_str())]
    UnknownSymbol(String),
    /// Metadata cannot appear as a numerical operand.
    #[error("metadata used as a numerical operand: {}", .0.as_str())]
    Metadata(String),
    /// The application has no declared interpretation.
    #[error("unknown function: {}", .0.as_str())]
    UnknownFunction(String),
    /// The application has the wrong number of arguments.
    #[error("function expects {expected} arguments, found {derived}")]
    Arity {
        /// Declared arity.
        expected: usize,
        /// Original argument count.
        derived: usize,
    },
    /// The declaration dependency graph contains a cycle.
    #[error("cyclic derived declaration: {0:?}")]
    Cycle(Vec<String>),
    /// Names are immutable once declared in a context.
    #[error("duplicate or reserved declaration: {}", .0.as_str())]
    Duplicate(String),
}

/// A refusal with original location, expected constraint and derived operands.
///
/// TODO ´todo:code:report-expected-and-derived-indexed-shapes´: report expected and derived indexed shapes without implying numerical certification under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:allow-callers-to-attach-point-node´: allow callers to attach point node and slot context to binding refusals under rule:nrz:binding-validation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{failure} at {location:?}; expected {expected:?}; derived {derived:?}")]
pub struct Diagnostic {
    /// Boxed location keeps the error value small while retaining complete paths.
    pub location: Box<Location>,
    /// Concrete expectation or operation contract.
    pub expected: Expectation,
    /// Descriptors of the original operands that were successfully checked.
    pub derived: Vec<Descriptor>,
    /// Typed failure reason.
    pub failure: Failure,
}

impl Diagnostic {
    /// Construct a declaration-level diagnostic before an expression location exists.
    pub(super) fn declaration(failure: Failure) -> Self {
        Self {
            location: Box::new(Location {
                path: Vec::new(),
                span: None,
                definitions: Vec::new(),
            }),
            expected: Expectation::Rule("a consistent unique declaration"),
            derived: Vec::new(),
            failure,
        }
    }
}
