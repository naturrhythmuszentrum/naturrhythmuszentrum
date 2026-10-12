// SPDX-License-Identifier: AGPL-3.0-only
//! Structured failures retain both source location and descriptor evidence.

use nrz_algebra::{Error, Span, Symbol};

use crate::{Descriptor, UnitError};

/// An original operation's position, including any expanded declaration chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    /// Zero-based operand indices from the current root.
    pub path: Vec<usize>,
    /// Optional parser byte range in the current declaration's source.
    pub span: Option<Span>,
    /// Declarations entered while reaching this node, in resolution order.
    pub definitions: Vec<Symbol>,
}

/// A diagnostic expectation can be a concrete descriptor or an operation rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expectation {
    /// A caller or declaration supplied this exact descriptor constraint.
    Descriptor(Box<Descriptor>),
    /// The operation requires this named constraint.
    Rule(&'static str),
}

/// The reason a check could not establish its contract.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Failure {
    /// Descriptor algebra refused the operation.
    #[error(transparent)]
    Units(#[from] UnitError),
    /// Generic expression construction or its resource limits failed.
    #[error(transparent)]
    Algebra(#[from] Error),
    /// Expanded or converted coordinates exceeded their construction limits.
    #[error("coordinate construction failed: {0}")]
    Coordinates(#[source] Error),
    /// A value name has no declaration.
    #[error("unknown value symbol: {}", .0.as_str())]
    UnknownSymbol(Symbol),
    /// Metadata cannot appear as a numerical operand.
    #[error("metadata used as a numerical operand: {}", .0.as_str())]
    Metadata(Symbol),
    /// The application has no declared interpretation.
    #[error("unknown function: {}", .0.as_str())]
    UnknownFunction(Symbol),
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
    Cycle(Vec<Symbol>),
    /// Names are immutable once declared in a context.
    #[error("duplicate or reserved declaration: {}", .0.as_str())]
    Duplicate(Symbol),
}

/// A refusal with original location, expected constraint and derived operands.
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
