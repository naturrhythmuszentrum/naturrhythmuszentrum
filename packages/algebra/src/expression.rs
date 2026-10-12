// SPDX-License-Identifier: AGPL-3.0-only
//! Original expressions, identities, locations and bounded substitution.

use std::collections::{BTreeMap, BTreeSet};

use crate::{ArithmeticError, Rational};

/// A typed refusal from expression construction or normalization.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    /// Exact arithmetic failed.
    #[error(transparent)]
    Arithmetic(#[from] ArithmeticError),
    /// An identifier is outside the portable identifier grammar.
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
    /// A source interval runs backwards.
    #[error("source span ends before it starts")]
    InvalidSpan,
    /// A configured resource bound was exceeded.
    #[error("expression resource limit: {0}")]
    Limit(&'static str),
}

/// An opaque identifier, compared by exact spelling.
///
/// TODO ´todo:code:document-client-name-resolution-into-declared´: document client name resolution into declared computation slots under def:nrz:plan-binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Symbol(String);

impl Symbol {
    /// Construct a portable identifier: ASCII letter/underscore, then letters/digits/underscores.
    ///
    /// # Errors
    /// Refuses an empty or malformed name.
    pub fn new(name: impl Into<String>) -> Result<Self, Error> {
        let name = name.into();
        let mut bytes = name.bytes();
        if !bytes
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(Error::InvalidIdentifier(name));
        }
        Ok(Self(name))
    }

    /// The exact spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A half-open byte interval in a caller-owned source string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    /// Inclusive start offset.
    start: usize,
    /// Exclusive end offset.
    end: usize,
}

impl Span {
    /// Construct an ordered interval.
    ///
    /// # Errors
    /// Refuses an end preceding the start.
    pub const fn new(start: usize, end: usize) -> Result<Self, Error> {
        if end < start {
            Err(Error::InvalidSpan)
        } else {
            Ok(Self { start, end })
        }
    }

    /// Inclusive start offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Exclusive end offset.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }
}

/// Explicit processing budgets; defaults bound recursive depth and total tree size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Maximum tree or parser recursion depth, capped by the implementation at 128.
    pub depth: usize,
    /// Maximum number of original tree nodes.
    pub nodes: usize,
    /// Maximum counted normalization or substitution steps.
    pub work: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            depth: 128,
            nodes: 10_000,
            work: 100_000,
        }
    }
}

/// An original operation. Construction never performs algebraic simplification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Node {
    /// Exact dimension-neutral coefficient.
    Constant(Rational),
    /// Opaque value identity.
    Symbol(Symbol),
    /// Original unary negation.
    Neg(Box<Expr>),
    /// Original addition, in source order.
    Add(Box<Expr>, Box<Expr>),
    /// Original subtraction, in source order.
    Sub(Box<Expr>, Box<Expr>),
    /// Original multiplication, in source order.
    Mul(Box<Expr>, Box<Expr>),
    /// Original division, in source order.
    Div(Box<Expr>, Box<Expr>),
    /// Exact rational power with the entire original base retained.
    Power(Box<Expr>, Rational),
    /// Uninterpreted named function and ordered arguments.
    Call(Symbol, Vec<Expr>),
}

impl Node {
    /// Original children in operand order.
    #[must_use]
    pub fn children(&self) -> Vec<&Expr> {
        match self {
            Self::Constant(_) | Self::Symbol(_) => Vec::new(),
            Self::Neg(value) | Self::Power(value, _) => vec![value],
            Self::Add(a, b) | Self::Sub(a, b) | Self::Mul(a, b) | Self::Div(a, b) => vec![a, b],
            Self::Call(_, arguments) => arguments.iter().collect(),
        }
    }
}

/// A bounded immutable source tree; algebraic equality is supplied separately by `NormalForm`.
///
/// TODO ´todo:code:document-client-retention-of-original-operations´: document client retention of original operations across repeated evaluations under rule:nrz:plan-validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Expr {
    /// Original operation.
    node: Node,
    /// Optional byte location in its source.
    span: Option<Span>,
    /// Cached depth checked at construction.
    depth: usize,
    /// Cached node count checked at construction.
    size: usize,
}

impl Expr {
    /// Construct an operation without rewriting it.
    ///
    /// # Errors
    /// Refuses a depth or size exceeding the supplied budget or the hard depth cap.
    pub fn new(node: Node, limits: Limits) -> Result<Self, Error> {
        let mut depth = 1;
        let mut size = 1_usize;
        for child in node.children() {
            depth = depth.max(child.depth + 1);
            size = size.checked_add(child.size).ok_or(Error::Limit("nodes"))?;
        }
        if depth > limits.depth.min(128) {
            return Err(Error::Limit("depth"));
        }
        if size > limits.nodes {
            return Err(Error::Limit("nodes"));
        }
        Ok(Self {
            node,
            span: None,
            depth,
            size,
        })
    }

    /// Original operation.
    #[must_use]
    pub const fn node(&self) -> &Node {
        &self.node
    }

    /// Original source location, when supplied.
    #[must_use]
    pub const fn span(&self) -> Option<Span> {
        self.span
    }

    /// Attach a source location without changing the operation.
    #[must_use]
    pub const fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Check this tree against another processing budget.
    ///
    /// # Errors
    /// Refuses excessive depth or node count.
    pub fn validate_limits(&self, limits: Limits) -> Result<(), Error> {
        if self.depth > limits.depth.min(128) {
            return Err(Error::Limit("depth"));
        }
        if self.size > limits.nodes {
            return Err(Error::Limit("nodes"));
        }
        Ok(())
    }

    /// Sorted value dependencies, excluding function names.
    #[must_use]
    pub fn symbols(&self) -> BTreeSet<Symbol> {
        let mut names = BTreeSet::new();
        if let Node::Symbol(name) = &self.node {
            names.insert(name.clone());
        }
        for child in self.node.children() {
            names.extend(child.symbols());
        }
        names
    }

    /// Replace symbols simultaneously, preserving operations and replacement locations.
    ///
    /// TODO ´todo:code:require-client-rechecking-of-substituted-original´: require client rechecking of substituted original expressions before reuse under rule:nrz:plan-validation.
    ///
    /// # Errors
    /// Refuses a substituted tree exceeding the supplied budgets.
    pub fn substitute(
        &self,
        replacements: &BTreeMap<Symbol, Self>,
        limits: Limits,
    ) -> Result<Self, Error> {
        self.validate_limits(limits)?;
        let mut work = limits.work;
        self.replace(replacements, limits, &mut work)
    }

    /// Recursive one-pass substitution; replacement trees are never traversed by this map.
    fn replace(
        &self,
        replacements: &BTreeMap<Symbol, Self>,
        limits: Limits,
        work: &mut usize,
    ) -> Result<Self, Error> {
        *work = work
            .checked_sub(1)
            .ok_or(Error::Limit("substitution work"))?;
        if let Node::Symbol(name) = &self.node
            && let Some(replacement) = replacements.get(name)
        {
            replacement.validate_limits(limits)?;
            *work = work
                .checked_sub(replacement.size)
                .ok_or(Error::Limit("substitution work"))?;
            return Ok(replacement.clone());
        }
        let mut child = |value: &Self| value.replace(replacements, limits, work).map(Box::new);
        let node = match &self.node {
            Node::Constant(value) => Node::Constant(*value),
            Node::Symbol(name) => Node::Symbol(name.clone()),
            Node::Neg(value) => Node::Neg(child(value)?),
            Node::Add(a, b) => Node::Add(child(a)?, child(b)?),
            Node::Sub(a, b) => Node::Sub(child(a)?, child(b)?),
            Node::Mul(a, b) => Node::Mul(child(a)?, child(b)?),
            Node::Div(a, b) => Node::Div(child(a)?, child(b)?),
            Node::Power(value, exponent) => Node::Power(child(value)?, *exponent),
            Node::Call(name, arguments) => Node::Call(
                name.clone(),
                arguments
                    .iter()
                    .map(|value| value.replace(replacements, limits, work))
                    .collect::<Result<_, _>>()?,
            ),
        };
        let mut result = Self::new(node, limits)?;
        result.span = self.span;
        Ok(result)
    }
}
