// SPDX-License-Identifier: AGPL-3.0-only
//! A bounded explicit grammar with exact literals and byte locations.

use std::fmt;

use crate::{ArithmeticError, Error, Expr, Limits, Node, Rational, Span, Symbol};

/// The reason parsing refused an expression.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ParseFailure {
    /// The next token does not fit the declared grammar.
    #[error("expected {0}")]
    Syntax(&'static str),
    /// Checked construction or exact arithmetic failed.
    #[error(transparent)]
    Core(#[from] Error),
}

/// A parser refusal at a half-open byte location.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{failure} at bytes {start}..{end}")]
pub struct ParseError {
    /// Inclusive offending byte.
    pub start: usize,
    /// Exclusive offending byte, equal to start at end of input.
    pub end: usize,
    /// Structured reason for refusal.
    pub failure: ParseFailure,
}

/// Parse a complete original expression without normalization or name resolution.
///
/// # Errors
/// Refuses unsupported syntax, unrepresentable exact literals and exhausted limits.
pub fn parse(source: &str, limits: Limits) -> Result<Expr, ParseError> {
    let mut parser = Parser {
        source,
        position: 0,
        limits,
    };
    if source.len() > limits.work {
        return Err(parser.core(Error::Limit("parser bytes")));
    }
    let result = parser.sum(1)?;
    parser.space();
    if parser.position != source.len() {
        return Err(parser.syntax("end of expression"));
    }
    Ok(result)
}

/// Cursor into a UTF-8 source; all admitted tokens are ASCII.
struct Parser<'a> {
    /// Entire source, retained by the caller.
    source: &'a str,
    /// Next byte to consume.
    position: usize,
    /// Construction and recursion bounds.
    limits: Limits,
}

impl Parser<'_> {
    /// Current byte without advancing.
    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }

    /// Skip portable ASCII whitespace.
    fn space(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.position += 1;
        }
    }

    /// Consume one optional punctuation token.
    fn take(&mut self, token: u8) -> bool {
        self.space();
        if self.peek() == Some(token) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// Report the current token, or the zero-width end position.
    fn syntax(&self, expected: &'static str) -> ParseError {
        ParseError {
            start: self.position,
            end: (self.position + 1).min(self.source.len()),
            failure: ParseFailure::Syntax(expected),
        }
    }

    /// Attach the current position to a checked-construction refusal.
    const fn core(&self, error: Error) -> ParseError {
        ParseError {
            start: self.position,
            end: self.position,
            failure: ParseFailure::Core(error),
        }
    }

    /// Construct an original operation with its complete source interval.
    fn build(&self, node: Node, start: usize) -> Result<Expr, ParseError> {
        let span = Span::new(start, self.position).map_err(|error| self.core(error))?;
        Expr::new(node, self.limits)
            .map(|expr| expr.with_span(span))
            .map_err(|error| self.core(error))
    }

    /// Guard every recursive grammar entry before descending.
    fn depth(&self, depth: usize) -> Result<(), ParseError> {
        if depth > self.limits.depth.min(128) {
            Err(self.core(Error::Limit("parser depth")))
        } else {
            Ok(())
        }
    }

    /// Parse left-associated addition and subtraction.
    fn sum(&mut self, depth: usize) -> Result<Expr, ParseError> {
        self.depth(depth)?;
        self.space();
        let start = self.position;
        let mut left = self.product(depth)?;
        loop {
            let subtract = if self.take(b'+') {
                false
            } else if self.take(b'-') {
                true
            } else {
                break;
            };
            let right = self.product(depth)?;
            let node = if subtract {
                Node::Sub(Box::new(left), Box::new(right))
            } else {
                Node::Add(Box::new(left), Box::new(right))
            };
            left = self.build(node, start)?;
        }
        Ok(left)
    }

    /// Parse left-associated multiplication and division.
    fn product(&mut self, depth: usize) -> Result<Expr, ParseError> {
        self.space();
        let start = self.position;
        let mut left = self.unary(depth)?;
        loop {
            let divide = if self.take(b'*') {
                false
            } else if self.take(b'/') {
                true
            } else {
                break;
            };
            let right = self.unary(depth)?;
            let node = if divide {
                Node::Div(Box::new(left), Box::new(right))
            } else {
                Node::Mul(Box::new(left), Box::new(right))
            };
            left = self.build(node, start)?;
        }
        Ok(left)
    }

    /// Unary signs bind less tightly than powers; signed literals admit i64 minimum.
    fn unary(&mut self, depth: usize) -> Result<Expr, ParseError> {
        self.depth(depth)?;
        self.space();
        let start = self.position;
        if self.take(b'+') {
            return self.unary(depth + 1);
        }
        if self.take(b'-') {
            self.space();
            if self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                let negative = self.number(true)?;
                self.space();
                if self.peek() != Some(b'^') {
                    return self.build(Node::Constant(negative), start);
                }
                let positive = negative
                    .checked_neg()
                    .map_err(|error| self.core(error.into()))?;
                let base = self.build(Node::Constant(positive), start + 1)?;
                let value = self.power_tail(base, start + 1)?;
                return self.build(Node::Neg(Box::new(value)), start);
            }
            let value = self.unary(depth + 1)?;
            return self.build(Node::Neg(Box::new(value)), start);
        }
        let base = self.atom(depth)?;
        self.power_tail(base, start)
    }

    /// Parse at most one power with an exact literal exponent.
    fn power_tail(&mut self, base: Expr, start: usize) -> Result<Expr, ParseError> {
        if !self.take(b'^') {
            return Ok(base);
        }
        let parenthesized = self.take(b'(');
        let numerator = self.signed_integer()?;
        let denominator = if parenthesized && self.take(b'/') {
            self.space();
            if !self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                return Err(self.syntax("positive integer denominator"));
            }
            self.signed_integer()?
        } else {
            1
        };
        if parenthesized && !self.take(b')') {
            return Err(self.syntax("closing exponent parenthesis"));
        }
        let exponent =
            Rational::new(numerator, denominator).map_err(|error| self.core(error.into()))?;
        self.build(Node::Power(Box::new(base), exponent), start)
    }

    /// Parse constants, names, applications and parenthesized expressions.
    fn atom(&mut self, depth: usize) -> Result<Expr, ParseError> {
        self.space();
        let start = self.position;
        if self.take(b'(') {
            let expression = self.sum(depth + 1)?;
            if !self.take(b')') {
                return Err(self.syntax("closing parenthesis"));
            }
            let span = Span::new(start, self.position).map_err(|error| self.core(error))?;
            return Ok(expression.with_span(span));
        }
        if self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            let value = self.number(false)?;
            return self.build(Node::Constant(value), start);
        }
        if !self
            .peek()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        {
            return Err(self.syntax("constant, identifier or parenthesized expression"));
        }
        while self
            .peek()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            self.position += 1;
        }
        let name =
            Symbol::new(&self.source[start..self.position]).map_err(|error| self.core(error))?;
        if !self.take(b'(') {
            return self.build(Node::Symbol(name), start);
        }
        let mut arguments = Vec::new();
        if !self.take(b')') {
            loop {
                arguments.push(self.sum(depth + 1)?);
                if self.take(b')') {
                    break;
                }
                if !self.take(b',') {
                    return Err(self.syntax("comma or closing application parenthesis"));
                }
            }
        }
        self.build(Node::Call(name, arguments), start)
    }

    /// Parse a contiguous signed integer exponent.
    fn signed_integer(&mut self) -> Result<i64, ParseError> {
        self.space();
        let start = self.position;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.position += 1;
        }
        let digits = self.position;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        if digits == self.position {
            return Err(self.syntax("integer exponent"));
        }
        self.source[start..self.position]
            .parse()
            .map_err(|_| self.core(ArithmeticError::Overflow.into()))
    }

    /// Parse a finite decimal/scientific literal directly into exact integer parts.
    fn number(&mut self, negative: bool) -> Result<Rational, ParseError> {
        let mut numerator = 0_i128;
        while let Some(byte) = self.peek().filter(u8::is_ascii_digit) {
            numerator = numerator
                .checked_mul(10)
                .and_then(|n| n.checked_add(i128::from(byte - b'0')))
                .ok_or_else(|| self.core(ArithmeticError::Overflow.into()))?;
            self.position += 1;
        }
        let mut places = 0_i64;
        if self.peek() == Some(b'.') {
            self.position += 1;
            while let Some(byte) = self.peek().filter(u8::is_ascii_digit) {
                numerator = numerator
                    .checked_mul(10)
                    .and_then(|n| n.checked_add(i128::from(byte - b'0')))
                    .ok_or_else(|| self.core(ArithmeticError::Overflow.into()))?;
                places += 1;
                self.position += 1;
            }
            if places == 0 {
                return Err(self.syntax("digits after decimal point"));
            }
        }
        let exponent = if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;
            if self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
                return Err(self.syntax("contiguous scientific exponent"));
            }
            self.signed_integer()?
        } else {
            0
        };
        let scale = exponent
            .checked_sub(places)
            .ok_or_else(|| self.core(ArithmeticError::Overflow.into()))?;
        let magnitude = u32::try_from(scale.unsigned_abs())
            .map_err(|_| self.core(ArithmeticError::Overflow.into()))?;
        let power = 10_i128
            .checked_pow(magnitude)
            .ok_or_else(|| self.core(ArithmeticError::Overflow.into()))?;
        let denominator = if scale < 0 { power } else { 1 };
        if scale >= 0 {
            numerator = numerator
                .checked_mul(power)
                .ok_or_else(|| self.core(ArithmeticError::Overflow.into()))?;
        }
        if negative {
            numerator = -numerator;
        }
        Rational::from_wide(numerator, denominator).map_err(|error| self.core(error.into()))
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.node() {
            Node::Constant(value) => {
                if value.is_integer() {
                    write!(f, "{value}")
                } else {
                    write!(f, "({value})")
                }
            }
            Node::Symbol(name) => f.write_str(name.as_str()),
            Node::Neg(value) => write!(f, "(-{value})"),
            Node::Add(a, b) => write!(f, "({a}+{b})"),
            Node::Sub(a, b) => write!(f, "({a}-{b})"),
            Node::Mul(a, b) => write!(f, "({a}*{b})"),
            Node::Div(a, b) => write!(f, "({a}/{b})"),
            Node::Power(base, exponent) => write!(f, "({base}^({exponent}))"),
            Node::Call(name, arguments) => {
                write!(f, "{}(", name.as_str())?;
                for (index, argument) in arguments.iter().enumerate() {
                    if index != 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{argument}")?;
                }
                f.write_str(")")
            }
        }
    }
}
