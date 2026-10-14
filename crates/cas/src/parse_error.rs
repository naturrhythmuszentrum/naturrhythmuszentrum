// SPDX-License-Identifier: AGPL-3.0-only
//! Source-located parser refusals, including ordinary graph and budget faults.

use std::fmt;

use crate::{Fault, syntax::Span};

/// Stable classifications for failures of the declared expression grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxCode {
    /// The next token cannot occur at this grammar position.
    UnexpectedToken,
    /// An opening parenthesis has no matching closing parenthesis.
    UnclosedParenthesis,
    /// A unary or binary operator has no following operand at end of input.
    TrailingOperator,
    /// A decimal point or scientific exponent lacks contiguous digits.
    InvalidLiteral,
    /// A power exponent is not a signed integer or parenthesized integer fraction.
    InvalidExponent,
}

/// A syntax, declaration or checked-construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseFailure<'a> {
    /// Invalid expression syntax, independent of resource exhaustion.
    Syntax(SyntaxCode),
    /// No declaration has this exact key.
    UnknownName {
        /// The original identifier spelling, borrowed without allocation.
        name: &'a str,
    },
    /// The named declaration has no function signature.
    NotCallable {
        /// The original identifier spelling.
        name: &'a str,
    },
    /// A declared function received the wrong number of arguments.
    Arity {
        /// The original function spelling.
        name: &'a str,
        /// Number of declared argument slots.
        expected: usize,
        /// Number of written arguments.
        actual: usize,
    },
    /// Exact arithmetic, typed admission or resource accounting refused work.
    Fault(Fault),
}

/// A parser refusal borrowing the caller's source identity and diagnostic spelling.
///
/// Borrowing keeps even a zero-budget refusal allocation-free. No graph prefix is
/// published on failure; the caller's work ledger still records consumed work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError<'a> {
    /// Caller source identity, including when the source could not be admitted.
    pub source_key: &'a str,
    /// Offending half-open UTF-8 byte interval; end-of-input intervals may be empty.
    pub span: Span,
    /// First unconsumed byte when parsing stopped, including skipped whitespace.
    pub parsed_to: u64,
    /// Stable syntax/declaration classification or the unchanged underlying fault.
    pub failure: ParseFailure<'a>,
}

impl fmt::Display for ParseError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.failure {
            ParseFailure::Syntax(code) => {
                return write!(
                    formatter,
                    "{code:?} at bytes {}..{} (parsed to {})",
                    self.span.start, self.span.end, self.parsed_to,
                );
            }
            ParseFailure::UnknownName { .. } => "unknown name",
            ParseFailure::NotCallable { .. } => "name is not callable",
            ParseFailure::Arity { .. } => "wrong call arity",
            ParseFailure::Fault(fault) => {
                return write!(
                    formatter,
                    "{fault} at bytes {}..{} (parsed to {})",
                    self.span.start, self.span.end, self.parsed_to,
                );
            }
        };
        write!(
            formatter,
            "{reason} at bytes {}..{} (parsed to {})",
            self.span.start, self.span.end, self.parsed_to
        )
    }
}

impl std::error::Error for ParseError<'_> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.failure {
            ParseFailure::Fault(fault) => Some(fault),
            _ => None,
        }
    }
}
