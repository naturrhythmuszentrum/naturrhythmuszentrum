// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-written recursive descent with heap continuations and ordinary graph drafts.

use crate::budget::{Budget, BudgetKind};
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, OccurrenceDraft, Reference, size};
use crate::model::{AtomicSort, Environment, Operator, Sort, Span};
use crate::parse_error::{ParseError, ParseFailure, SyntaxCode};
use crate::source_factory::OriginalForm;
use crate::{Fault, InvalidCode, Location, Rational};

/// Location shared by parser accounting and internal invariant refusals.
const AT: Location = Location::new("original expression parsing");

/// Parse a complete original expression using an admitted declaration header.
///
/// Names match declaration keys, never display names. Literal scalars use the
/// header's field, and general powers retain its explicit branch convention.
/// Parentheses and unary plus widen the enclosed occurrence's span. No receipt
/// or equality is minted: callers still complete `Cas::walk_original`.
///
/// # Errors
/// Returns a located syntax/declaration refusal or an unchanged `Fault` with
/// parse progress. All source, expression and occurrence drafts publish atomically.
pub fn parse_original<'a>(
    cas: &mut Cas,
    source_key: &'a str,
    text: &'a str,
    declarations: &Handle,
    budget: &mut Budget,
) -> Result<OriginalForm, ParseError<'a>> {
    let previous = budget.restrict_storage(&cas.limits);
    let result = parse_bounded(cas, source_key, text, declarations, budget);
    budget.restore_limits(previous);
    result
}

/// Stage a complete parse before admitting it through the ordinary transaction.
fn parse_bounded<'a>(
    cas: &mut Cas,
    source_key: &'a str,
    text: &'a str,
    declarations: &Handle,
    budget: &mut Budget,
) -> Result<OriginalForm, ParseError<'a>> {
    let input = Input {
        source_key,
        text,
        position: 0,
    };
    let id = cas
        .checked(declarations)
        .map_err(|fault| input.fault(fault))?;
    let header = cas.state.header(id).map_err(|fault| input.fault(fault))?;
    let mut parser = Parser {
        input,
        environment: &header.environment,
        header: declarations,
        budget,
        drafts: Vec::new(),
        frames: Vec::new(),
        value: None,
        originals: cas.counts().original_occurrences,
        vertices: size(cas.counts().vertices, AT).map_err(|fault| input.fault(fault))?,
        scratch: cas.counts().payload_bytes,
        metadata_bytes: size(header.environment.canonical_bytes().len(), AT)
            .map_err(|fault| input.fault(fault))?,
    };
    parser.initialize()?;
    let (drafts, root, end) = parser.parse()?;
    let handles = cas
        .admit(&drafts, budget)
        .map_err(|fault| end.fault(fault))?;
    let get = |index: u32| {
        handles
            .get(index as usize)
            .cloned()
            .ok_or_else(|| end.fault(Fault::invalid(InvalidCode::Reference, AT)))
    };
    Ok(OriginalForm {
        syntax: get(root.expression)?,
        root: get(root.occurrence)?,
    })
}

/// Borrowed input and a cursor which always lies on a UTF-8 boundary.
#[derive(Clone, Copy)]
struct Input<'a> {
    /// Caller identity, borrowed even for pre-admission errors.
    source_key: &'a str,
    /// Exact bytes to retain on successful admission.
    text: &'a str,
    /// Next unconsumed byte.
    position: usize,
}

impl<'a> Input<'a> {
    /// Machine-sized offsets were bounded by the source length before scanning.
    fn offset(self) -> u64 {
        u64::try_from(self.position).unwrap_or(u64::MAX)
    }

    /// Name the next complete UTF-8 scalar, or the empty end-of-input interval.
    fn span(self) -> Span {
        let width = self
            .text
            .get(self.position..)
            .and_then(|tail| tail.chars().next())
            .map_or(0, char::len_utf8);
        Span {
            start: self.offset(),
            end: self.offset() + u64::try_from(width).unwrap_or(0),
        }
    }

    /// Preserve structured diagnostics without allocating source or name copies.
    fn error(self, span: Span, failure: ParseFailure<'a>) -> ParseError<'a> {
        ParseError {
            source_key: self.source_key,
            span,
            parsed_to: self.offset(),
            failure,
        }
    }

    /// Attach current progress to a checked failure.
    fn fault(self, fault: Fault) -> ParseError<'a> {
        self.error(self.span(), ParseFailure::Fault(fault))
    }
}

/// References into the one flat draft batch, never an alternative expression tree.
#[derive(Clone, Copy)]
struct Parsed {
    /// Earlier mathematical draft.
    expression: u32,
    /// Earlier distinct occurrence draft.
    occurrence: u32,
    /// Span of this occurrence before any surrounding parentheses.
    span: Span,
    /// Operand depth for pre-admission checks.
    depth: u64,
    /// Numeric token bytes when a directly preceding minus may form a signed literal.
    literal_bytes: Option<u64>,
}

/// Suspended grammar calls; no source nesting uses the native call stack.
enum Frame {
    /// Enter an additive expression at this grammar nesting depth.
    Sum(u64),
    /// Continue consuming left-associated additive operators.
    SumTail(u64),
    /// Finish one additive right operand.
    SumRight(Parsed, bool, u64),
    /// Enter a multiplicative expression.
    Product(u64),
    /// Continue consuming left-associated multiplicative operators.
    ProductTail(u64),
    /// Finish one multiplicative right operand.
    ProductRight(Parsed, bool, u64),
    /// Enter a signed expression.
    Unary(u64),
    /// Finish a prefix sign after its more tightly bound operand.
    Sign(Span, bool),
    /// Finish at most one exact-literal power.
    Power,
    /// Close a grouping parenthesis.
    Group(Span),
    /// Finish an argument, preserving the function name and written order.
    Call(Call),
}

/// A suspended named application; arguments are draft references only.
struct Call {
    /// Resolved declaration index.
    declaration: usize,
    /// Identifier bytes for unknown-signature and arity diagnostics.
    name: Span,
    /// Opening parenthesis for missing-close diagnostics.
    opening: Span,
    /// Completed arguments in source order.
    arguments: Vec<Parsed>,
    /// Nesting depth of the call itself.
    depth: u64,
}

/// Parser workspace, with only unpublished graph drafts as expression storage.
struct Parser<'a, 'store, 'budget> {
    /// Source and diagnostic cursor.
    input: Input<'a>,
    /// Immutable declarations and branch policy from the caller's header.
    environment: &'store Environment,
    /// Actual store-owned declaration identity for every expression.
    header: &'store Handle,
    /// Shared monotone construction ledger.
    budget: &'budget mut Budget,
    /// Child-before-parent source, expression and occurrence drafts.
    drafts: Vec<Draft>,
    /// Explicit recursive-descent return stack.
    frames: Vec<Frame>,
    /// Return value of the most recently completed grammar call.
    value: Option<Parsed>,
    /// Existing plus newly staged original occurrences.
    originals: u64,
    /// Existing graph vertices for conservative staging checks.
    vertices: u64,
    /// Conservative cumulative workspace reservation, including existing payload.
    scratch: u64,
    /// Bound on a cloned declaration sort or branch payload.
    metadata_bytes: u64,
}

impl<'a> Parser<'a, '_, '_> {
    /// Reserve source storage before copying or inspecting caller-controlled bytes.
    fn initialize(&mut self) -> Result<(), ParseError<'a>> {
        let length = size(self.input.text.len(), AT).map_err(|f| self.input.fault(f))?;
        let key_length = size(self.input.source_key.len(), AT).map_err(|f| self.input.fault(f))?;
        self.check(BudgetKind::SourceBytes, length)?;
        self.check(BudgetKind::NameBytes, key_length)?;
        let bytes = length
            .checked_add(key_length)
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| {
                self.input
                    .fault(Fault::capacity(BudgetKind::StagedPayloadBytes, AT))
            })?;
        self.reserve(bytes)?;
        let key = self.copy(self.input.source_key)?;
        let text = self.copy(self.input.text)?;
        self.push_draft(Draft::Source { key, text })?;
        self.push(Frame::Sum(1))
    }

    /// Execute the grammar's continuations and require complete consumption.
    fn parse(mut self) -> Result<(Vec<Draft>, Parsed, Input<'a>), ParseError<'a>> {
        while let Some(frame) = self.frames.pop() {
            self.budget.work(1, AT).map_err(|f| self.input.fault(f))?;
            self.resume(frame)?;
        }
        self.space()?;
        if self.peek().is_some() {
            return Err(self.syntax(SyntaxCode::UnexpectedToken));
        }
        let root = self.result()?;
        Ok((self.drafts, root, self.input))
    }

    /// Dispatch one suspended production without recursive Rust calls.
    fn resume(&mut self, frame: Frame) -> Result<(), ParseError<'a>> {
        match frame {
            Frame::Sum(depth) => {
                self.check(BudgetKind::Depth, depth)?;
                self.push(Frame::SumTail(depth))?;
                self.push(Frame::Product(depth))
            }
            Frame::Product(depth) => {
                self.push(Frame::ProductTail(depth))?;
                self.push(Frame::Unary(depth))
            }
            Frame::SumTail(depth) => self.tail(false, depth),
            Frame::ProductTail(depth) => self.tail(true, depth),
            Frame::SumRight(left, subtract, depth) => {
                self.binary(
                    left,
                    if subtract {
                        Operator::Sub
                    } else {
                        Operator::Add
                    },
                )?;
                self.push(Frame::SumTail(depth))
            }
            Frame::ProductRight(left, divide, depth) => {
                self.binary(left, if divide { Operator::Div } else { Operator::Mul })?;
                self.push(Frame::ProductTail(depth))
            }
            Frame::Unary(depth) => self.unary(depth),
            Frame::Sign(span, negative) => self.sign(span, negative),
            Frame::Power => self.power(),
            Frame::Group(opening) => self.group(opening),
            Frame::Call(call) => self.argument(call),
        }
    }

    /// Consume one left-associated operator before suspending its right operand.
    fn tail(&mut self, product: bool, depth: u64) -> Result<(), ParseError<'a>> {
        self.space()?;
        let alternate = match (product, self.peek()) {
            (false, Some(b'+')) | (true, Some(b'*')) => false,
            (false, Some(b'-')) | (true, Some(b'/')) => true,
            _ => return Ok(()),
        };
        let operator = self.input.span();
        self.bump()?;
        self.require_operand(operator)?;
        let left = self.result()?;
        if product {
            self.push(Frame::ProductRight(left, alternate, depth))?;
            self.push(Frame::Unary(depth))
        } else {
            self.push(Frame::SumRight(left, alternate, depth))?;
            self.push(Frame::Product(depth))
        }
    }

    /// Signs bind below powers; their recursive calls live in the explicit stack.
    fn unary(&mut self, depth: u64) -> Result<(), ParseError<'a>> {
        self.space()?;
        self.check(BudgetKind::Depth, depth)?;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            let negative = self.peek() == Some(b'-');
            let span = self.input.span();
            self.bump()?;
            self.require_operand(span)?;
            self.push(Frame::Sign(span, negative))?;
            return self.push(Frame::Unary(self.deeper(depth)?));
        }
        self.push(Frame::Power)?;
        self.atom(depth)
    }

    /// Read a leaf, named call or parenthesized sum.
    fn atom(&mut self, depth: u64) -> Result<(), ParseError<'a>> {
        self.space()?;
        match self.peek() {
            Some(b'(') => {
                let opening = self.input.span();
                self.bump()?;
                self.open_operand(opening)?;
                self.push(Frame::Group(opening))?;
                self.push(Frame::Sum(self.deeper(depth)?))
            }
            Some(byte) if byte.is_ascii_digit() => {
                let start = self.input.position;
                self.number_token()?;
                let span = self.range(start);
                let value = self.rational(span)?;
                self.reserve(self.metadata_bytes)?;
                let sort = Sort::Value(AtomicSort::Scalar(self.environment.field.clone()));
                let mut node = self.build(Operator::Rational(value), sort, &[], span)?;
                node.literal_bytes = Some(span.end - span.start);
                self.value = Some(node);
                Ok(())
            }
            Some(byte) if byte.is_ascii_alphabetic() || byte == b'_' => self.name(depth),
            _ => Err(self.syntax(SyntaxCode::UnexpectedToken)),
        }
    }

    /// Finish a binary operation while retaining its written operand order.
    fn binary(&mut self, left: Parsed, operator: Operator) -> Result<(), ParseError<'a>> {
        let right = self.result()?;
        self.reserve(self.metadata_bytes)?;
        let sort = self.sort(left)?.clone();
        let span = Span {
            start: left.span.start,
            end: right.span.end,
        };
        self.value = Some(self.build(operator, sort, &[left, right], span)?);
        Ok(())
    }

    /// A grouping pair widens the enclosed occurrence without inventing syntax.
    fn group(&mut self, opening: Span) -> Result<(), ParseError<'a>> {
        self.close(opening, SyntaxCode::UnexpectedToken)?;
        let mut node = self.result()?;
        node.literal_bytes = None;
        self.widen(
            &mut node,
            Span {
                start: opening.start,
                end: self.input.offset(),
            },
        )?;
        self.value = Some(node);
        Ok(())
    }

    /// Apply a sign, folding only a directly written negative numeric literal.
    fn sign(&mut self, prefix: Span, negative: bool) -> Result<(), ParseError<'a>> {
        let mut node = self.result()?;
        let span = Span {
            start: prefix.start,
            end: node.span.end,
        };
        if negative {
            if let Some(bytes) = node.literal_bytes {
                self.check_at(BudgetKind::LiteralBytes, bytes.saturating_add(1), span)?;
                self.negate_literal(node)?;
            } else {
                self.reserve(self.metadata_bytes)?;
                let sort = self.sort(node)?.clone();
                node = self.build(Operator::Neg, sort, &[node], span)?;
            }
        }
        node.literal_bytes = None;
        self.widen(&mut node, span)?;
        self.value = Some(node);
        Ok(())
    }

    /// Resolve exactly one declaration key before constructing a name or call.
    fn name(&mut self, depth: u64) -> Result<(), ParseError<'a>> {
        let start = self.input.position;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            self.check(
                BudgetKind::NameBytes,
                self.range(start).end - self.range(start).start + 1,
            )?;
            self.bump()?;
        }
        let name = self.range(start);
        let spelling = self.spelling(name)?;
        let declaration = self.resolve(spelling, name)?;
        self.space()?;
        if self.peek() != Some(b'(') {
            self.reserve(self.metadata_bytes)?;
            let sort = self
                .environment
                .declarations
                .get(declaration)
                .ok_or_else(|| self.internal())?
                .sort
                .clone();
            let key = self.copy(spelling)?;
            self.value = Some(self.build(Operator::Free(key), sort, &[], name)?);
            return Ok(());
        }
        if !matches!(
            &self
                .environment
                .declarations
                .get(declaration)
                .ok_or_else(|| self.internal())?
                .sort,
            Sort::Function { .. }
        ) {
            return Err(self
                .input
                .error(name, ParseFailure::NotCallable { name: spelling }));
        }
        let opening = self.input.span();
        self.bump()?;
        self.open_operand(opening)?;
        let call = Call {
            declaration,
            name,
            opening,
            arguments: Vec::new(),
            depth,
        };
        if self.peek() == Some(b')') {
            self.bump()?;
            return self.finish_call(&call);
        }
        self.push(Frame::Call(call))?;
        self.push(Frame::Sum(self.deeper(depth)?))
    }

    /// Charge every key comparison, including an unsuccessful name lookup.
    fn resolve(&mut self, spelling: &'a str, span: Span) -> Result<usize, ParseError<'a>> {
        for (index, declaration) in self.environment.declarations.iter().enumerate() {
            let bytes = size(declaration.key.len().max(spelling.len()), AT)
                .map_err(|f| self.input.fault(f))?;
            self.budget
                .work(bytes.saturating_add(1), AT)
                .map_err(|f| self.input.fault(f))?;
            if declaration.key == spelling {
                return Ok(index);
            }
        }
        Err(self
            .input
            .error(span, ParseFailure::UnknownName { name: spelling }))
    }

    /// Keep arguments in source order and resume the next argument if present.
    fn argument(&mut self, mut call: Call) -> Result<(), ParseError<'a>> {
        let argument = self.result()?;
        self.reserve(64)?;
        call.arguments
            .try_reserve(1)
            .map_err(|_| self.input.fault(Fault::allocation(AT)))?;
        call.arguments.push(argument);
        self.space()?;
        match self.peek() {
            Some(b',') => {
                self.bump()?;
                self.open_operand(call.opening)?;
                let depth = self.deeper(call.depth)?;
                self.push(Frame::Call(call))?;
                self.push(Frame::Sum(depth))
            }
            Some(b')') => {
                self.bump()?;
                self.finish_call(&call)
            }
            None => Err(self.input.error(
                call.opening,
                ParseFailure::Syntax(SyntaxCode::UnclosedParenthesis),
            )),
            _ => Err(self.syntax(SyntaxCode::UnexpectedToken)),
        }
    }

    /// Refuse wrong arity at the function name, then admit its ordered syntax.
    fn finish_call(&mut self, call: &Call) -> Result<(), ParseError<'a>> {
        self.reserve(self.metadata_bytes)?;
        let declaration = self
            .environment
            .declarations
            .get(call.declaration)
            .ok_or_else(|| self.internal())?;
        let Sort::Function { inputs, output } = &declaration.sort else {
            return Err(self.internal());
        };
        let name = self.spelling(call.name)?;
        if inputs.len() != call.arguments.len() {
            return Err(self.input.error(
                call.name,
                ParseFailure::Arity {
                    name,
                    expected: inputs.len(),
                    actual: call.arguments.len(),
                },
            ));
        }
        let sort = Sort::Value(output.clone());
        let key = self.copy(name)?;
        let span = Span {
            start: call.name.start,
            end: self.input.offset(),
        };
        self.value = Some(self.build(Operator::Call(key), sort, &call.arguments, span)?);
        Ok(())
    }

    /// Powers have one literal exponent and never consume another power tail.
    fn power(&mut self) -> Result<(), ParseError<'a>> {
        self.space()?;
        if self.peek() != Some(b'^') {
            return Ok(());
        }
        let operator = self.input.span();
        self.bump()?;
        self.require_operand(operator)?;
        let base = self.result()?;
        let (exponent, exponent_span) = self.exponent()?;
        let integer = exponent
            .integer_exponent(self.budget)
            .map_err(|f| self.input.error(exponent_span, ParseFailure::Fault(f)))?;
        self.reserve(self.metadata_bytes)?;
        let sort = self.sort(base)?.clone();
        let span = Span {
            start: base.span.start,
            end: exponent_span.end,
        };
        let node = if let Some(value) = integer {
            self.build(Operator::IntegerPower(value), sort, &[base], span)?
        } else {
            let exponent_sort = Sort::Value(AtomicSort::Scalar(self.environment.field.clone()));
            let child = self.build(
                Operator::Rational(exponent),
                exponent_sort,
                &[],
                exponent_span,
            )?;
            self.build(
                Operator::GeneralPower(self.environment.branch.clone()),
                sort,
                &[base, child],
                span,
            )?
        };
        self.value = Some(node);
        Ok(())
    }

    /// Read an exact signed integer, optionally parenthesized with a positive denominator.
    fn exponent(&mut self) -> Result<(Rational, Span), ParseError<'a>> {
        self.space()?;
        let opening = (self.peek() == Some(b'(')).then(|| self.input.span());
        if let Some(parenthesis) = opening {
            self.bump()?;
            self.open_operand(parenthesis)?;
        }
        let numerator_span = self.integer_token(true)?;
        let mut value = self.rational(numerator_span)?;
        let span = if let Some(parenthesis) = opening {
            self.space()?;
            if self.peek() == Some(b'/') {
                self.bump()?;
                let denominator_span = self.integer_token(false)?;
                let bytes = (numerator_span.end - numerator_span.start)
                    .checked_add(denominator_span.end - denominator_span.start)
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| {
                        self.input
                            .fault(Fault::capacity(BudgetKind::LiteralBytes, AT))
                    })?;
                self.check_at(
                    BudgetKind::LiteralBytes,
                    bytes,
                    Span {
                        start: numerator_span.start,
                        end: denominator_span.end,
                    },
                )?;
                let denominator = self.rational(denominator_span)?;
                value = value
                    .div(&denominator, self.budget)
                    .map_err(|f| self.input.error(denominator_span, ParseFailure::Fault(f)))?;
            }
            self.close(parenthesis, SyntaxCode::InvalidExponent)?;
            Span {
                start: parenthesis.start,
                end: self.input.offset(),
            }
        } else {
            numerator_span
        };
        Ok((value, span))
    }

    /// Read a finite decimal/scientific token without admitting the rational reader's extensions.
    fn number_token(&mut self) -> Result<(), ParseError<'a>> {
        let start = self.input.position;
        self.digits(start, SyntaxCode::InvalidLiteral)?;
        if self.peek() == Some(b'.') {
            self.literal_bump(start)?;
            self.digits(start, SyntaxCode::InvalidLiteral)?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.literal_bump(start)?;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.literal_bump(start)?;
            }
            self.digits(start, SyntaxCode::InvalidLiteral)?;
        }
        Ok(())
    }

    /// Integer exponent signs are contiguous; denominator signs are forbidden.
    fn integer_token(&mut self, signed: bool) -> Result<Span, ParseError<'a>> {
        self.space()?;
        let start = self.input.position;
        if signed && matches!(self.peek(), Some(b'+' | b'-')) {
            self.literal_bump(start)?;
        }
        self.digits(start, SyntaxCode::InvalidExponent)?;
        Ok(self.range(start))
    }

    /// Consume a nonempty run of digits under the literal-byte ceiling.
    fn digits(&mut self, start: usize, code: SyntaxCode) -> Result<(), ParseError<'a>> {
        if !self.peek().is_some_and(|b| b.is_ascii_digit()) {
            return Err(self.syntax(code));
        }
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.literal_bump(start)?;
        }
        Ok(())
    }

    /// Check a literal's next byte before consuming it.
    fn literal_bump(&mut self, start: usize) -> Result<(), ParseError<'a>> {
        let span = self.range(start);
        self.check_at(
            BudgetKind::LiteralBytes,
            span.end - span.start + 1,
            Span {
                start: span.start,
                end: self.input.span().end,
            },
        )?;
        self.bump()
    }

    /// Construct the exact coefficient under the same active budget.
    fn rational(&mut self, span: Span) -> Result<Rational, ParseError<'a>> {
        let spelling = self.spelling(span)?;
        Rational::parse(spelling, self.budget)
            .map_err(|f| self.input.error(span, ParseFailure::Fault(f)))
    }

    /// Stage ordinary expression/occurrence pairs with the exact same ordered children.
    fn build(
        &mut self,
        operator: Operator,
        sort: Sort,
        children: &[Parsed],
        span: Span,
    ) -> Result<Parsed, ParseError<'a>> {
        let depth = children
            .iter()
            .map(|child| child.depth)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| self.input.fault(Fault::capacity(BudgetKind::Depth, AT)))?;
        self.check_at(BudgetKind::Depth, depth, span)?;
        let originals = self.originals.checked_add(1).ok_or_else(|| {
            self.input
                .fault(Fault::capacity(BudgetKind::OriginalOccurrences, AT))
        })?;
        self.check_at(BudgetKind::OriginalOccurrences, originals, span)?;
        let bytes = size(children.len(), AT)
            .map_err(|f| self.input.fault(f))?
            .checked_mul(64)
            .and_then(|n| n.checked_add(1024))
            .and_then(|n| n.checked_add(self.metadata_bytes))
            .ok_or_else(|| {
                self.input
                    .fault(Fault::capacity(BudgetKind::StagedPayloadBytes, AT))
            })?;
        self.reserve(bytes)?;
        let operands = self.references(children, false)?;
        let uses = self.references(children, true)?;
        let expression = self.push_draft(Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort,
            header: Reference::Existing(self.header.clone()),
            scope: Vec::new(),
            binding: None,
            operands,
        }))?;
        let occurrence = self.push_draft(Draft::Occurrence(OccurrenceDraft {
            expression: Reference::Local(expression),
            source: Some(Reference::Local(0)),
            span: Some(span),
            children: uses,
            provenance: None,
            binder_names: Vec::new(),
        }))?;
        self.originals = originals;
        Ok(Parsed {
            expression,
            occurrence,
            span,
            depth,
            literal_bytes: None,
        })
    }

    /// Build fallibly reserved ordered references into the same staging batch.
    fn references(
        &self,
        children: &[Parsed],
        occurrences: bool,
    ) -> Result<Vec<Reference>, ParseError<'a>> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(children.len())
            .map_err(|_| self.input.fault(Fault::allocation(AT)))?;
        result.extend(children.iter().map(|child| {
            Reference::Local(if occurrences {
                child.occurrence
            } else {
                child.expression
            })
        }));
        Ok(result)
    }

    /// Append only after checking the draft index and full candidate vertex bound.
    fn push_draft(&mut self, draft: Draft) -> Result<u32, ParseError<'a>> {
        let index = u32::try_from(self.drafts.len()).map_err(|_| {
            self.input
                .fault(Fault::capacity(BudgetKind::StagedVertices, AT))
        })?;
        let total = self
            .vertices
            .checked_add(u64::from(index))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| {
                self.input
                    .fault(Fault::capacity(BudgetKind::StagedVertices, AT))
            })?;
        self.check(BudgetKind::StagedVertices, total)?;
        self.drafts
            .try_reserve(1)
            .map_err(|_| self.input.fault(Fault::allocation(AT)))?;
        self.drafts.push(draft);
        Ok(index)
    }

    /// Inspect a staged result sort without constructing any parallel syntax tree.
    fn sort(&self, node: Parsed) -> Result<&Sort, ParseError<'a>> {
        match self.drafts.get(node.expression as usize) {
            Some(Draft::Expression(expression)) => Ok(&expression.sort),
            _ => Err(self.internal()),
        }
    }

    /// A signed numeric token changes its own unpublished literal, never an existing graph node.
    fn negate_literal(&mut self, node: Parsed) -> Result<(), ParseError<'a>> {
        let value = match self.drafts.get(node.expression as usize) {
            Some(Draft::Expression(ExpressionDraft {
                operator: Operator::Rational(value),
                ..
            })) => value
                .negated(self.budget)
                .map_err(|f| self.input.fault(f))?,
            _ => return Err(self.internal()),
        };
        match self.drafts.get_mut(node.expression as usize) {
            Some(Draft::Expression(expression)) => expression.operator = Operator::Rational(value),
            _ => return Err(self.internal()),
        }
        Ok(())
    }

    /// Preserve every delimiter in the source span without adding a mathematical operation.
    fn widen(&mut self, node: &mut Parsed, span: Span) -> Result<(), ParseError<'a>> {
        match self.drafts.get_mut(node.occurrence as usize) {
            Some(Draft::Occurrence(occurrence)) => occurrence.span = Some(span),
            _ => return Err(self.internal()),
        }
        node.span = span;
        Ok(())
    }

    /// Take exactly one grammar return value.
    fn result(&mut self) -> Result<Parsed, ParseError<'a>> {
        self.value.take().ok_or_else(|| self.internal())
    }

    /// Prepay continuation storage, including discarded frames conservatively.
    fn push(&mut self, frame: Frame) -> Result<(), ParseError<'a>> {
        self.reserve(128)?;
        self.frames
            .try_reserve(1)
            .map_err(|_| self.input.fault(Fault::allocation(AT)))?;
        self.frames.push(frame);
        Ok(())
    }

    /// Reserve cumulative parser workspace before any associated allocation or clone.
    fn reserve(&mut self, bytes: u64) -> Result<(), ParseError<'a>> {
        let next = self.scratch.checked_add(bytes).ok_or_else(|| {
            self.input
                .fault(Fault::capacity(BudgetKind::StagedPayloadBytes, AT))
        })?;
        self.check(BudgetKind::StagedPayloadBytes, next)?;
        self.budget
            .work(bytes, AT)
            .map_err(|f| self.input.fault(f))?;
        self.scratch = next;
        Ok(())
    }

    /// Copy only bounded strings, using a fallible reservation.
    fn copy(&mut self, text: &str) -> Result<String, ParseError<'a>> {
        self.reserve(size(text.len(), AT).map_err(|f| self.input.fault(f))?)?;
        let mut result = String::new();
        result
            .try_reserve_exact(text.len())
            .map_err(|_| self.input.fault(Fault::allocation(AT)))?;
        result.push_str(text);
        Ok(result)
    }

    /// Keep all resource faults unchanged while adding the active source position.
    fn check(&self, kind: BudgetKind, needed: u64) -> Result<(), ParseError<'a>> {
        self.check_at(kind, needed, self.input.span())
    }

    /// Attach the actual affected occurrence or literal to a size refusal.
    fn check_at(&self, kind: BudgetKind, needed: u64, span: Span) -> Result<(), ParseError<'a>> {
        self.budget
            .check(kind, needed, AT)
            .map_err(|f| self.input.error(span, ParseFailure::Fault(f)))
    }

    /// Increment grammar nesting with a checked counter; no fixed native-stack cap exists.
    fn deeper(&self, depth: u64) -> Result<u64, ParseError<'a>> {
        depth
            .checked_add(1)
            .ok_or_else(|| self.input.fault(Fault::capacity(BudgetKind::Depth, AT)))
    }

    /// ASCII punctuation and admitted name/literal bytes are read without allocation.
    fn peek(&self) -> Option<u8> {
        self.input.text.as_bytes().get(self.input.position).copied()
    }

    /// Consume one known ASCII byte, charging before advancing the cursor.
    fn bump(&mut self) -> Result<(), ParseError<'a>> {
        if !self.peek().is_some_and(|byte| byte.is_ascii()) {
            return Err(self.syntax(SyntaxCode::UnexpectedToken));
        }
        self.budget.work(1, AT).map_err(|f| self.input.fault(f))?;
        self.input.position += 1;
        Ok(())
    }

    /// Only ASCII whitespace belongs to the declared grammar.
    fn space(&mut self) -> Result<(), ParseError<'a>> {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.bump()?;
        }
        Ok(())
    }

    /// An absent operand is diagnosed at its operator, not an invented EOF token.
    fn require_operand(&mut self, operator: Span) -> Result<(), ParseError<'a>> {
        self.space()?;
        if self.peek().is_none() {
            return Err(self
                .input
                .error(operator, ParseFailure::Syntax(SyntaxCode::TrailingOperator)));
        }
        Ok(())
    }

    /// Diagnose an immediately unclosed group or call at its opening delimiter.
    fn open_operand(&mut self, opening: Span) -> Result<(), ParseError<'a>> {
        self.space()?;
        if self.peek().is_none() {
            return Err(self.input.error(
                opening,
                ParseFailure::Syntax(SyntaxCode::UnclosedParenthesis),
            ));
        }
        Ok(())
    }

    /// Consume a required closing delimiter with a distinct missing-close code.
    fn close(&mut self, opening: Span, other: SyntaxCode) -> Result<(), ParseError<'a>> {
        self.space()?;
        match self.peek() {
            Some(b')') => self.bump(),
            None => Err(self.input.error(
                opening,
                ParseFailure::Syntax(SyntaxCode::UnclosedParenthesis),
            )),
            _ => Err(self.syntax(other)),
        }
    }

    /// Span from a previously saved ASCII boundary to the current byte.
    fn range(&self, start: usize) -> Span {
        Span {
            start: u64::try_from(start).unwrap_or(u64::MAX),
            end: self.input.offset(),
        }
    }

    /// Borrow a previously scanned name or numeric spelling with checked indices.
    fn spelling(&self, span: Span) -> Result<&'a str, ParseError<'a>> {
        let start = usize::try_from(span.start).map_err(|_| self.internal())?;
        let end = usize::try_from(span.end).map_err(|_| self.internal())?;
        self.input
            .text
            .get(start..end)
            .ok_or_else(|| self.internal())
    }

    /// A grammar failure always covers a whole scalar or the empty end position.
    fn syntax(&self, code: SyntaxCode) -> ParseError<'a> {
        self.input
            .error(self.input.span(), ParseFailure::Syntax(code))
    }

    /// Internal shape failures remain values, even if a future grammar edit is inconsistent.
    fn internal(&self) -> ParseError<'a> {
        self.input.fault(Fault::invalid(InvalidCode::Reference, AT))
    }
}

#[cfg(test)]
mod tests {
    use super::{Input, ParseFailure, Span, SyntaxCode};
    use crate::{Fault, InvalidCode, Location};

    /// Offending tokens cover complete UTF-8 scalars, including empty EOF spans.
    /// ´test:unit:parser-diagnostic-boundaries´
    #[test]
    fn parser_diagnostic_boundaries() {
        for (position, expected) in [
            (0, Span { start: 0, end: 1 }),
            (1, Span { start: 1, end: 3 }),
            (3, Span { start: 3, end: 7 }),
            (7, Span { start: 7, end: 7 }),
        ] {
            let input = Input {
                source_key: "unicode",
                text: "xβ🦀",
                position,
            };
            let error = input.error(
                input.span(),
                ParseFailure::Syntax(SyntaxCode::UnexpectedToken),
            );
            assert_eq!(error.span, expected);
            assert_eq!(error.parsed_to, expected.start);
            assert_eq!(error.source_key, "unicode");
        }
    }

    /// The parser wrapper preserves the original error as a standard error source.
    /// ´test:unit:parser-fault-source´
    #[test]
    fn parser_fault_source() {
        use std::error::Error;
        let input = Input {
            source_key: "fault",
            text: "",
            position: 0,
        };
        let fault = Fault::invalid(InvalidCode::SourceConflict, Location::new("source"));
        let wrapped = input.fault(fault);
        assert_eq!(wrapped.failure, ParseFailure::Fault(fault));
        assert_eq!(
            wrapped.source().map(ToString::to_string),
            Some(fault.to_string())
        );
        let syntax = input.error(
            input.span(),
            ParseFailure::Syntax(SyntaxCode::UnexpectedToken),
        );
        assert!(syntax.source().is_none());
    }
}
