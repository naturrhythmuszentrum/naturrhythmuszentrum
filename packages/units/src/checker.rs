// SPDX-License-Identifier: AGPL-3.0-only
//! Original-operation checking with bounded declaration expansion.

use std::collections::{BTreeMap, BTreeSet};

use nrz_algebra::{Error, Expr, Node, Rational, Symbol};

use crate::{
    BinaryOp, CheckedExpression, Companion, CompanionEvidence, Context, Conversion, Declaration,
    Descriptor, Diagnostic, Expectation, Failure, FunctionSignature, Kind, Location, Measure,
    NodeEvidence, Role, UncertaintyMode, UnitError, uncertainty,
};

impl Context {
    /// Check every original operation before exposing a coordinate normal form.
    ///
    /// TODO ´todo:code:dispatch-future-shaped-checks-without-treating´: dispatch future shaped checks without treating scalar traversal as matrix multiplication evidence under rule:nrz:shape-data-time-evidence.
    ///
    /// # Errors
    /// Reports unknown names, cycles, metadata operands, physical refusals and limits.
    pub fn check(&self, expression: &Expr) -> Result<CheckedExpression, Diagnostic> {
        expression
            .validate_limits(self.limits)
            .map_err(|error| Diagnostic::declaration(error.into()))?;
        let mut state = State {
            context: self,
            remaining: self.limits.work,
            path: Vec::new(),
            stack: Vec::new(),
            definitions: Vec::new(),
            expected: None,
            nodes: Vec::new(),
            declarations: BTreeMap::new(),
        };
        let value = state.walk(expression, 1)?;
        Ok(CheckedExpression {
            original: expression.clone(),
            descriptor: value.descriptor,
            nodes: state.nodes,
            coordinates: value.coordinates,
            declarations: state.declarations,
            output: None,
            companion: value.companion,
        })
    }

    /// Check an original expression against an explicit output descriptor.
    ///
    /// TODO ´todo:code:compare-declared-output-axes-and-companion´: compare declared output axes and companion extents before exposing converted matrix results under rule:nrz:shape-data-time-evidence.
    ///
    /// # Errors
    /// Reports expression failures or a dimension, kind, role or measure mismatch.
    pub fn check_against(
        &self,
        expression: &Expr,
        expected: &Descriptor,
    ) -> Result<CheckedExpression, Diagnostic> {
        let mut checked = self.check(expression)?;
        let failure = |error: UnitError| Diagnostic {
            location: Box::new(Location {
                path: Vec::new(),
                span: expression.span(),
                definitions: Vec::new(),
            }),
            expected: Expectation::Descriptor(Box::new(expected.clone())),
            derived: vec![checked.descriptor.clone()],
            failure: error.into(),
        };
        self.registry.validate(expected).map_err(failure)?;
        checked.output = Some(
            checked
                .descriptor
                .coherent()
                .conversion_to(expected)
                .map_err(failure)?,
        );
        Ok(checked)
    }

    /// Check both sides of an equation for physical compatibility, without solving it.
    ///
    /// TODO ´todo:code:check-ordered-equation-axes-and-typed´: check ordered equation axes and typed entry correspondence on both sides under rule:nrz:shape-entry-contract.
    ///
    /// # Errors
    /// Reports either original expression's failure or incompatible side descriptors.
    pub fn check_equation(
        &self,
        left: &Expr,
        right: &Expr,
    ) -> Result<(CheckedExpression, CheckedExpression), Diagnostic> {
        let left = self.check(left).map_err(|mut error| {
            error.location.path.insert(0, 0);
            error
        })?;
        let right = self
            .check_against(right, left.descriptor())
            .map_err(|mut error| {
                error.location.path.insert(0, 1);
                error
            })?;
        Ok((left, right))
    }

    /// Check each scalar entry of a homogeneous assembled block against its descriptor.
    ///
    /// This is descriptor validation, not commutative rewriting of matrix operations.
    ///
    /// TODO ´todo:code:validate-block-extents-partitions-and-quotient´: validate block extents partitions and quotient axes beyond individually checked scalar entries under def:nrz:matrix-shape and rule:nrz:shape-entry-contract.
    ///
    /// # Errors
    /// Returns the first invalid entry with its index prepended to the original path.
    pub fn check_block(
        &self,
        entries: &[Expr],
        expected: &Descriptor,
    ) -> Result<Vec<CheckedExpression>, Diagnostic> {
        entries
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.check_against(expression, expected)
                    .map_err(|mut error| {
                        error.location.path.insert(0, index);
                        error
                    })
            })
            .collect()
    }

    /// Validate all numerical declarations, including unused observables and cycles.
    ///
    /// TODO ´todo:code:include-unused-shape-declarations-and-stale´: include unused shape declarations and stale indexed companion references in context validation under rule:nrz:shape-data-time-evidence.
    ///
    /// # Errors
    /// Reports the first failing declaration in deterministic symbol order.
    pub fn check_all(&self) -> Result<BTreeMap<Symbol, CheckedExpression>, Diagnostic> {
        let mut result = BTreeMap::new();
        for (name, declaration) in &self.declarations {
            if declaration.descriptor().is_some() {
                let expression = Expr::new(Node::Symbol(name.clone()), self.limits)
                    .map_err(|error| Diagnostic::declaration(error.into()))?;
                result.insert(name.clone(), self.check(&expression)?);
            }
        }
        Ok(result)
    }
}

/// A checked subtree, with coordinates in coherent measure.
///
/// TODO ´todo:code:retain-original-entry-identities-and-logical´: retain original entry identities and logical maps for shaped intermediate values under def:nrz:companion-shape.
struct Value {
    /// Physical interpretation of the original subtree.
    descriptor: Descriptor,
    /// Converted scalar coordinates after original checks.
    coordinates: Expr,
    /// Explicit nonzero assumption or exact literal evidence only.
    nonzero: bool,
    /// Known bare or typed ratio constant, never general numerical evaluation.
    literal: Option<Rational>,
    /// Original symbol identity, absent after any operation.
    origin: Option<Symbol>,
    /// Value and reference identities established by supported uncertainty operations.
    companion: Option<CompanionEvidence>,
    /// Dependencies and uncertainty operands reached before any algebraic rewriting.
    trace: Trace,
}

/// Dependency and contributor evidence collected from original expression operands.
///
/// TODO ´todo:code:track-indexed-dependencies-and-paired-uncertainty´: track indexed dependencies and paired uncertainty contributors before zero or cancellation rewrites under rule:nrz:shape-data-time-evidence.
#[derive(Default)]
struct Trace {
    /// All declarations reached by this subtree, including resolved derived dependencies.
    dependencies: BTreeSet<Symbol>,
    /// Bound uncertainty operands, indexed by their original declaration names.
    contributors: BTreeMap<Symbol, Companion>,
    /// An uncertainty operand lacked a checked value relationship.
    unbound: bool,
}

impl Trace {
    /// Combine independently checked operands without discarding cancelled contributions.
    ///
    /// TODO ´todo:code:merge-contributor-entry-maps-without-erasing´: merge contributor entry maps without erasing shared source dependence under def:nrz:companion-shape and rule:nrz:shape-sensitivity-propagation.
    fn merge(&mut self, other: Self) {
        self.dependencies.extend(other.dependencies);
        self.contributors.extend(other.contributors);
        self.unbound |= other.unbound;
    }
}

/// Value dependencies that may supply uncertainty contributors to an explicit propagation.
///
/// TODO ´todo:code:restrict-covariance-contributors-to-the-declared´: restrict covariance contributors to the declared paired target closures and contraction axes under rule:nrz:shape-data-time-evidence.
struct PropagationScope {
    /// The primary value and, for covariance, the second value's dependency closures.
    values: BTreeSet<Symbol>,
    /// Propagation requires a derived primary value or an explicit covariance pair.
    admitted: bool,
}

/// One bounded traversal and its original-operation evidence.
struct State<'a> {
    /// Immutable physical declarations and rules.
    context: &'a Context,
    /// Remaining visits, including expanded definitions.
    remaining: usize,
    /// Current original operand path.
    path: Vec<usize>,
    /// Active declaration chain used for cycle detection.
    stack: Vec<Symbol>,
    /// Declarations whose expression or relationship is the current diagnostic source.
    definitions: Vec<Symbol>,
    /// Expected companion used to disambiguate ratio references from scaling coefficients.
    expected: Option<Companion>,
    /// Postorder evidence retained in the successful result.
    nodes: Vec<NodeEvidence>,
    /// Owned snapshots of declarations reached by this traversal.
    declarations: BTreeMap<Symbol, Declaration>,
}

impl State<'_> {
    /// Capture the current source location before any rewriting.
    fn location(&self, source: &Expr) -> Location {
        Location {
            path: self.path.clone(),
            span: source.span(),
            definitions: self.definitions.clone(),
        }
    }

    /// Make a structured refusal at the original operation.
    fn error(
        &self,
        source: &Expr,
        failure: Failure,
        expected: Expectation,
        derived: Vec<Descriptor>,
    ) -> Diagnostic {
        Diagnostic {
            location: Box::new(self.location(source)),
            expected,
            derived,
            failure,
        }
    }

    /// Report a physical operation's rule and its derived operand descriptors.
    fn units(&self, source: &Expr, failure: UnitError, derived: Vec<Descriptor>) -> Diagnostic {
        self.error(
            source,
            failure.into(),
            Expectation::Rule(
                "compatible dimensions, explicit kinds, affine roles and count domains",
            ),
            derived,
        )
    }

    /// Construct coordinates under the same explicit resource limits.
    fn build(&self, source: &Expr, node: Node) -> Result<Expr, Diagnostic> {
        Expr::new(node, self.context.limits).map_err(|error| {
            self.error(
                source,
                Failure::Coordinates(error),
                Expectation::Rule("bounded exact coordinate expression"),
                Vec::new(),
            )
        })
    }

    /// Apply an exact coordinate map without normalizing original operations.
    ///
    /// TODO ´todo:code:apply-axis-conversions-to-values-and´: apply axis conversions to values and both covariance indices while excluding affine uncertainty offsets under def:nrz:covariance-shape.
    fn convert(
        &self,
        source: &Expr,
        mut coordinates: Expr,
        conversion: Conversion,
    ) -> Result<Expr, Diagnostic> {
        if conversion.scale != Rational::ONE {
            let scale = self.build(source, Node::Constant(conversion.scale))?;
            coordinates = self.build(source, Node::Mul(Box::new(scale), Box::new(coordinates)))?;
        }
        if conversion.shift != Rational::ZERO {
            let shift = self.build(source, Node::Constant(conversion.shift))?;
            coordinates = self.build(source, Node::Add(Box::new(coordinates), Box::new(shift)))?;
        }
        Ok(coordinates)
    }

    /// Enter a child while restoring the path on both success and failure.
    fn child(&mut self, source: &Expr, index: usize, depth: usize) -> Result<Value, Diagnostic> {
        self.path.push(index);
        let result = self.walk(source, depth + 1);
        self.path.pop();
        result
    }

    /// Visit every original child before checking its parent's physical operation.
    fn walk(&mut self, source: &Expr, depth: usize) -> Result<Value, Diagnostic> {
        if depth > self.context.limits.depth.min(128) || self.remaining == 0 {
            return Err(self.error(
                source,
                Error::Limit("units traversal").into(),
                Expectation::Rule("bounded original traversal and declaration expansion"),
                Vec::new(),
            ));
        }
        self.remaining -= 1;
        let mut arguments = Vec::new();
        let value = match source.node() {
            Node::Constant(value) => Value {
                descriptor: Descriptor::ratio(),
                coordinates: source.clone(),
                nonzero: *value != Rational::ZERO,
                literal: Some(*value),
                origin: None,
                companion: None,
                trace: Trace::default(),
            },
            Node::Symbol(name) => self.symbol(source, name, depth)?,
            Node::Neg(child) => {
                let value = self.child(child, 0, depth)?;
                let descriptor = self
                    .context
                    .registry
                    .negate(&value.descriptor)
                    .map_err(|error| self.units(source, error, vec![value.descriptor.clone()]))?;
                let literal = value
                    .literal
                    .map(Rational::checked_neg)
                    .transpose()
                    .map_err(|error| self.units(source, error.into(), vec![value.descriptor]))?;
                Value {
                    descriptor,
                    coordinates: self.build(source, Node::Neg(Box::new(value.coordinates)))?,
                    nonzero: false,
                    literal,
                    origin: None,
                    companion: value.companion,
                    trace: value.trace,
                }
            }
            Node::Add(left, right) => self.binary(source, BinaryOp::Add, left, right, depth)?,
            Node::Sub(left, right) => {
                self.binary(source, BinaryOp::Subtract, left, right, depth)?
            }
            Node::Mul(left, right) => {
                self.binary(source, BinaryOp::Multiply, left, right, depth)?
            }
            Node::Div(left, right) => self.binary(source, BinaryOp::Divide, left, right, depth)?,
            Node::Power(base, exponent) => {
                let base = self.child(base, 0, depth)?;
                self.power(source, base, *exponent)?
            }
            Node::Call(name, values) => {
                let mut checked = Vec::with_capacity(values.len());
                for (index, value) in values.iter().enumerate() {
                    checked.push(self.child(value, index, depth)?);
                }
                let (value, conversions) = self.call(source, name, checked)?;
                arguments = conversions;
                value
            }
        };
        let conversion = value
            .descriptor
            .conversion_to(&value.descriptor.coherent())
            .map_err(|error| self.units(source, error, vec![value.descriptor.clone()]))?;
        self.nodes.push(NodeEvidence {
            location: self.location(source),
            descriptor: value.descriptor.clone(),
            conversion,
            arguments,
        });
        Ok(value)
    }

    /// Validate and expand a numerical declaration, retaining its evidence snapshot.
    fn symbol(&mut self, source: &Expr, name: &Symbol, depth: usize) -> Result<Value, Diagnostic> {
        if depth > self.context.limits.depth.min(128) || self.remaining == 0 {
            return Err(self.error(
                source,
                Error::Limit("declaration depth").into(),
                Expectation::Rule("bounded declaration expansion"),
                Vec::new(),
            ));
        }
        self.remaining -= 1;
        if self.stack.contains(name) {
            let mut cycle = self.stack.clone();
            cycle.push(name.clone());
            return Err(self.error(
                source,
                Failure::Cycle(cycle),
                Expectation::Rule("acyclic declarations"),
                Vec::new(),
            ));
        }
        let declaration = self
            .context
            .declarations
            .get(name)
            .ok_or_else(|| {
                self.error(
                    source,
                    Failure::UnknownSymbol(name.clone()),
                    Expectation::Rule("a declared numerical symbol"),
                    Vec::new(),
                )
            })?
            .clone();
        self.declarations.insert(name.clone(), declaration.clone());
        let nonzero = declaration.is_nonzero();
        let descriptor = declaration
            .descriptor()
            .ok_or_else(|| {
                self.error(
                    source,
                    Failure::Metadata(name.clone()),
                    Expectation::Rule("a numerical operand"),
                    Vec::new(),
                )
            })?
            .clone();
        let has_definition = matches!(
            declaration,
            Declaration::Derived(_)
                | Declaration::Uncertainty { .. }
                | Declaration::UncertaintyInput { .. }
        );
        self.stack.push(name.clone());
        if has_definition {
            self.definitions.push(name.clone());
        }
        let result = self.resolve(source, declaration, &descriptor, depth);
        if has_definition {
            self.definitions.pop();
        }
        self.stack.pop();
        let mut value = result?;
        value.descriptor = descriptor;
        value.origin = Some(name.clone());
        value.nonzero = nonzero;
        value.trace.dependencies.insert(name.clone());
        if let Some(evidence) = &value.companion {
            value.trace.contributors =
                BTreeMap::from([(name.clone(), evidence.companion().clone())]);
            value.trace.unbound = false;
        } else if matches!(value.descriptor.kind(), Kind::Uncertainty(..))
            && value.trace.contributors.is_empty()
        {
            value.trace.unbound = true;
        }
        Ok(value)
    }

    /// Resolve one declaration while its cycle and diagnostic scopes are active.
    fn resolve(
        &mut self,
        source: &Expr,
        declaration: Declaration,
        descriptor: &Descriptor,
        depth: usize,
    ) -> Result<Value, Diagnostic> {
        let conversion = descriptor
            .conversion_to(&descriptor.coherent())
            .map_err(|error| self.units(source, error, vec![descriptor.clone()]))?;
        let mut literal = None;
        let mut companion = None;
        let coordinates = match declaration {
            Declaration::Input { .. } => self.convert(source, source.clone(), conversion)?,
            Declaration::Constant { value, .. } => {
                if descriptor.kind() == &Kind::Ratio {
                    literal =
                        Some(conversion.apply(value).map_err(|error| {
                            self.units(source, error, vec![descriptor.clone()])
                        })?);
                }
                let coordinates = self.build(source, Node::Constant(value))?;
                self.convert(source, coordinates, conversion)?
            }
            Declaration::UncertaintyInput {
                companion: relation,
                ..
            } => {
                let location = self.build(source, source.node().clone())?;
                self.companion(&location, &relation, descriptor, depth)?;
                companion = Some(CompanionEvidence::Established(relation));
                self.convert(source, source.clone(), conversion)?
            }
            Declaration::Derived(observable) => {
                return self.observable(&observable.expression, descriptor, None, depth);
            }
            Declaration::Uncertainty {
                observable,
                companion,
            } => {
                self.path.push(0);
                let scope = self.companion(&observable.expression, &companion, descriptor, depth);
                self.path.pop();
                let scope = scope?;
                return self.observable(
                    &observable.expression,
                    descriptor,
                    Some((&companion, &scope)),
                    depth,
                );
            }
            Declaration::Metadata(_) => {
                return Err(self.units(source, UnitError::KindMismatch, Vec::new()));
            }
        };
        Ok(Value {
            descriptor: descriptor.clone(),
            coordinates,
            nonzero: false,
            literal,
            origin: None,
            companion,
            trace: Trace::default(),
        })
    }

    /// Validate a body and use the same root path for diagnostics and node evidence.
    fn observable(
        &mut self,
        expression: &Expr,
        descriptor: &Descriptor,
        companion: Option<(&Companion, &PropagationScope)>,
        depth: usize,
    ) -> Result<Value, Diagnostic> {
        self.path.push(0);
        let result = self.observable_body(expression, descriptor, companion, depth);
        self.path.pop();
        result
    }

    /// Check physical compatibility before establishing or propagating a relationship.
    ///
    /// TODO ´todo:code:check-shaped-observable-targets-and-retain´: check shaped observable targets and retain the operating point for declared sensitivities under rule:nrz:shape-sensitivity-propagation and rule:nrz:shape-data-time-evidence.
    fn observable_body(
        &mut self,
        expression: &Expr,
        descriptor: &Descriptor,
        companion: Option<(&Companion, &PropagationScope)>,
        depth: usize,
    ) -> Result<Value, Diagnostic> {
        expression
            .validate_limits(self.context.limits)
            .map_err(|error| {
                self.error(
                    expression,
                    error.into(),
                    Expectation::Rule("bounded observable expression"),
                    Vec::new(),
                )
            })?;
        let previous = std::mem::replace(
            &mut self.expected,
            companion.map(|(companion, _)| companion.clone()),
        );
        let result = self.walk(expression, depth + 1);
        self.expected = previous;
        let mut value = result?;
        value
            .descriptor
            .conversion_to(descriptor)
            .map_err(|error| {
                self.error(
                    expression,
                    error.into(),
                    Expectation::Descriptor(Box::new(descriptor.clone())),
                    vec![value.descriptor.clone()],
                )
            })?;
        if let Some((companion, scope)) = companion
            && value.companion.as_ref().map(CompanionEvidence::companion) != Some(companion)
        {
            self.propagate(expression, descriptor, companion, scope, &mut value)?;
        }
        Ok(value)
    }

    /// Bind an explicitly declared propagation only to contributors inside its value closures.
    ///
    /// TODO ´todo:code:validate-indexed-covariance-contractions-while-leaving´: validate indexed covariance contractions while leaving derivative correctness and numerical assurance separate under rule:nrz:shape-data-time-evidence.
    fn propagate(
        &self,
        expression: &Expr,
        descriptor: &Descriptor,
        companion: &Companion,
        scope: &PropagationScope,
        value: &mut Value,
    ) -> Result<(), Diagnostic> {
        let admitted = scope.admitted
            && companion.reference.is_none()
            && !value.trace.unbound
            && !value.trace.contributors.is_empty()
            && value.trace.contributors.values().all(|contributor| {
                scope.values.contains(&contributor.value)
                    && contributor
                        .other
                        .as_ref()
                        .is_none_or(|other| scope.values.contains(other))
            });
        if !admitted {
            let mut derived = vec![value.descriptor.clone()];
            for name in std::iter::once(&companion.value)
                .chain(companion.other.as_ref())
                .chain(companion.reference.as_ref())
            {
                derived.extend(
                    self.context
                        .declarations
                        .get(name)
                        .and_then(Declaration::descriptor)
                        .cloned(),
                );
            }
            return Err(self.error(
                expression,
                UnitError::UncertaintyIdentity.into(),
                Expectation::Descriptor(Box::new(descriptor.clone())),
                derived,
            ));
        }
        value.companion = Some(CompanionEvidence::Propagated {
            companion: companion.clone(),
            contributors: value.trace.contributors.clone(),
        });
        Ok(())
    }

    /// Resolve a relationship name without borrowing a span from another expression.
    fn relationship_value(&mut self, name: &Symbol, depth: usize) -> Result<Value, Diagnostic> {
        let source = Expr::new(Node::Symbol(name.clone()), self.context.limits)
            .map_err(|error| Diagnostic::declaration(error.into()))?;
        self.symbol(&source, name, depth + 1)
    }

    /// Check the companion's descriptors at its own expression source.
    ///
    /// TODO ´todo:code:resolve-companion-entry-axes-and-relative´: resolve companion entry axes and relative references before accepting relationship evidence under def:nrz:companion-shape and rule:nrz:correlation-and-relative-shapes.
    fn companion(
        &mut self,
        source: &Expr,
        companion: &Companion,
        descriptor: &Descriptor,
        depth: usize,
    ) -> Result<PropagationScope, Diagnostic> {
        let value = self.relationship_value(&companion.value, depth)?;
        let other = companion
            .other
            .as_ref()
            .map(|name| self.relationship_value(name, depth))
            .transpose()?;
        let reference = companion
            .reference
            .as_ref()
            .map(|name| self.relationship_value(name, depth))
            .transpose()?;
        let mut derived = vec![descriptor.clone(), value.descriptor.clone()];
        derived.extend(other.as_ref().map(|value| value.descriptor.clone()));
        derived.extend(reference.as_ref().map(|value| value.descriptor.clone()));
        let expected = uncertainty(
            companion.mode,
            &value.descriptor,
            other.as_ref().map(|value| &value.descriptor),
            reference
                .as_ref()
                .map(|value| (&value.descriptor, value.nonzero)),
        )
        .map_err(|error| self.units(source, error, derived.clone()))?;
        descriptor.conversion_to(&expected).map_err(|error| {
            self.error(
                source,
                error.into(),
                Expectation::Descriptor(Box::new(expected)),
                derived,
            )
        })?;
        let mut values = value.trace.dependencies;
        if let Some(other) = other {
            values.extend(other.trace.dependencies);
        }
        let admitted = companion.mode == UncertaintyMode::Covariance
            || matches!(
                self.context.declarations.get(&companion.value),
                Some(Declaration::Derived(_))
            );
        Ok(PropagationScope { values, admitted })
    }

    /// Check both original operands and then build their coherent coordinate operation.
    fn binary(
        &mut self,
        source: &Expr,
        operation: BinaryOp,
        left: &Expr,
        right: &Expr,
        depth: usize,
    ) -> Result<Value, Diagnostic> {
        let left = self.child(left, 0, depth)?;
        let right = self.child(right, 1, depth)?;
        let relative = operation == BinaryOp::Divide && self.relative_requested(&left, &right);
        let descriptor = self
            .binary_descriptor(operation, &left, &right, relative)
            .map_err(|error| {
                self.units(
                    source,
                    error,
                    vec![left.descriptor.clone(), right.descriptor.clone()],
                )
            })?;
        let companion = Self::binary_companion(operation, &left, &right, &descriptor, relative);
        let mut trace = left.trace;
        trace.merge(right.trace);
        trace.unbound |= matches!(descriptor.kind(), Kind::Uncertainty(..))
            && !matches!(left.descriptor.kind(), Kind::Uncertainty(..))
            && !matches!(right.descriptor.kind(), Kind::Uncertainty(..))
            && trace.contributors.is_empty();
        let pair = (Box::new(left.coordinates), Box::new(right.coordinates));
        let node = match operation {
            BinaryOp::Add => Node::Add(pair.0, pair.1),
            BinaryOp::Subtract => Node::Sub(pair.0, pair.1),
            BinaryOp::Multiply => Node::Mul(pair.0, pair.1),
            BinaryOp::Divide => Node::Div(pair.0, pair.1),
        };
        Ok(Value {
            descriptor,
            coordinates: self.build(source, node)?,
            nonzero: false,
            literal: None,
            origin: None,
            companion,
            trace,
        })
    }

    /// Distinguish a ratio coefficient from a named value or explicitly declared reference.
    ///
    /// TODO ´todo:code:match-relative-references-to-individual-target´: match relative references to individual target entries rather than shape equality under rule:nrz:correlation-and-relative-shapes.
    fn relative_requested(&self, left: &Value, right: &Value) -> bool {
        let Kind::Uncertainty(UncertaintyMode::Radius | UncertaintyMode::Standard, value, None) =
            left.descriptor.kind()
        else {
            return false;
        };
        if value.as_ref() != right.descriptor.kind() {
            return false;
        }
        if right.descriptor.kind() != &Kind::Ratio {
            return true;
        }
        left.companion.as_ref().is_some_and(|evidence| {
            let companion = evidence.companion();
            right.origin.as_ref() == Some(&companion.value)
                || self.expected.as_ref().is_some_and(|expected| {
                    expected.value == companion.value
                        && expected.reference.is_some()
                        && expected.reference == right.origin
                })
        })
    }

    /// Consult semantic rules before falling back to literal count scaling or symbol identity.
    fn binary_descriptor(
        &self,
        operation: BinaryOp,
        left: &Value,
        right: &Value,
        relative: bool,
    ) -> Result<Descriptor, UnitError> {
        let registry = &self.context.registry;
        if relative {
            if left.companion.is_none() {
                return Err(UnitError::UncertaintyIdentity);
            }
            return registry.relative(&left.descriptor, &right.descriptor, right.nonzero);
        }
        let result = registry.binary(operation, &left.descriptor, &right.descriptor);
        let same_symbol = left.origin.is_some() && left.origin == right.origin;
        let standards = matches!(
            left.descriptor.kind(),
            Kind::Uncertainty(UncertaintyMode::Standard, _, _)
        ) && matches!(
            right.descriptor.kind(),
            Kind::Uncertainty(UncertaintyMode::Standard, _, _)
        );
        if operation == BinaryOp::Multiply {
            if matches!(result, Err(UnitError::UnsupportedKindRule)) && same_symbol && standards {
                return registry.power(&left.descriptor, Rational::integer(2));
            }
            if let Ok(descriptor) = &result
                && standards
                && matches!(
                    descriptor.kind(),
                    Kind::Uncertainty(UncertaintyMode::Variance, _, _)
                )
                && !same_symbol
            {
                return Err(UnitError::UncertaintyIdentity);
            }
            if matches!(result, Err(UnitError::CountDomain)) {
                if left.descriptor.kind().count_domain().is_some()
                    && right.descriptor.kind() == &Kind::Ratio
                {
                    return registry.scale_count(
                        &left.descriptor,
                        right.literal.ok_or(UnitError::CountDomain)?,
                    );
                }
                if right.descriptor.kind().count_domain().is_some()
                    && left.descriptor.kind() == &Kind::Ratio
                {
                    return registry.scale_count(
                        &right.descriptor,
                        left.literal.ok_or(UnitError::CountDomain)?,
                    );
                }
            }
        }
        result
    }

    /// Recognize a standard/variance kind pair without inventing another value relationship.
    fn variance_pair(standard: &Kind, variance: &Kind) -> bool {
        matches!(
            (standard, variance),
            (
                Kind::Uncertainty(UncertaintyMode::Standard, left, None),
                Kind::Uncertainty(UncertaintyMode::Variance, right, None)
            ) if left == right
        )
    }

    /// Preserve identities only through explicitly supported uncertainty operations.
    ///
    /// TODO ´todo:code:preserve-covariance-pair-identities-through-admitted´: preserve covariance pair identities through admitted contractions and reject unrelated equal shapes under rule:nrz:shape-sensitivity-propagation.
    fn binary_companion(
        operation: BinaryOp,
        left: &Value,
        right: &Value,
        result: &Descriptor,
        relative: bool,
    ) -> Option<CompanionEvidence> {
        let Kind::Uncertainty(mode, _, _) = result.kind() else {
            return None;
        };
        if relative {
            let mut companion = left.companion.clone()?;
            companion.transform(*mode, right.origin.clone());
            return Some(companion);
        }
        let source = if operation == BinaryOp::Multiply
            && left.literal.is_some()
            && result.kind() == right.descriptor.kind()
        {
            right
        } else if (right.literal.is_some() && result.kind() == left.descriptor.kind())
            || (operation == BinaryOp::Multiply
                && left.origin.is_some()
                && left.origin == right.origin
                && Self::variance_pair(left.descriptor.kind(), result.kind()))
            || (matches!(operation, BinaryOp::Add | BinaryOp::Subtract)
                && left.companion == right.companion)
        {
            left
        } else {
            return None;
        };
        let mut companion = source.companion.clone()?;
        let reference = companion.companion().reference.clone();
        companion.transform(*mode, reference);
        Some(companion)
    }

    /// Check a power and preserve identity only through identity powers or variance transitions.
    ///
    /// TODO ´todo:code:gate-matrix-powers-by-endomorphism-and´: gate matrix powers by endomorphism and scalar scale contracts outside the scalar walker under rule:nrz:shape-powers-and-functions.
    fn power(&self, source: &Expr, base: Value, exponent: Rational) -> Result<Value, Diagnostic> {
        let descriptor = self
            .context
            .registry
            .power(&base.descriptor, exponent)
            .map_err(|error| self.units(source, error, vec![base.descriptor.clone()]))?;
        let preserves = (exponent == Rational::ONE && descriptor.kind() == base.descriptor.kind())
            || (exponent == Rational::integer(2)
                && Self::variance_pair(base.descriptor.kind(), descriptor.kind()))
            || (exponent.numerator() == 1
                && exponent.denominator() == 2
                && Self::variance_pair(descriptor.kind(), base.descriptor.kind()));
        let companion = if let Kind::Uncertainty(mode, _, _) = descriptor.kind()
            && preserves
        {
            base.companion.map(|mut companion| {
                let reference = companion.companion().reference.clone();
                companion.transform(*mode, reference);
                companion
            })
        } else {
            None
        };
        let mut trace = base.trace;
        trace.unbound |= matches!(descriptor.kind(), Kind::Uncertainty(..))
            && !matches!(base.descriptor.kind(), Kind::Uncertainty(..))
            && trace.contributors.is_empty();
        Ok(Value {
            descriptor,
            coordinates: self.build(source, Node::Power(Box::new(base.coordinates), exponent))?,
            nonzero: false,
            literal: None,
            origin: None,
            companion,
            trace,
        })
    }

    /// Check special relationships or a fixed function signature after all argument visits.
    ///
    /// TODO ´todo:code:require-declared-shape-and-dependency-signatures´: require declared shape and dependency signatures for matrix valued functions under rule:nrz:shape-data-time-evidence.
    fn call(
        &self,
        source: &Expr,
        name: &Symbol,
        mut values: Vec<Value>,
    ) -> Result<(Value, Vec<Conversion>), Diagnostic> {
        if matches!(name.as_str(), "sqrt" | "count_ratio") {
            let expected = if name.as_str() == "sqrt" { 1 } else { 2 };
            self.arity(source, expected, &values)?;
            let first = values.remove(0);
            if name.as_str() == "sqrt" {
                let half = Rational::new(1, 2)
                    .map_err(|error| self.units(source, error.into(), Vec::new()))?;
                return Ok((self.power(source, first, half)?, Vec::new()));
            }
            let second = values.remove(0);
            let descriptor = self
                .context
                .registry
                .count_ratio(&first.descriptor, &second.descriptor)
                .map_err(|error| {
                    self.units(source, error, vec![first.descriptor, second.descriptor])
                })?;
            let coordinates = self.build(
                source,
                Node::Div(Box::new(first.coordinates), Box::new(second.coordinates)),
            )?;
            let mut trace = first.trace;
            trace.merge(second.trace);
            return Ok((
                Value {
                    descriptor,
                    coordinates,
                    nonzero: false,
                    literal: None,
                    origin: None,
                    companion: None,
                    trace,
                },
                Vec::new(),
            ));
        }
        let signature = self.signature(source, name)?;
        self.arity(source, signature.parameters.len(), &values)?;
        let mut conversions = Vec::new();
        let mut coordinates = Vec::new();
        let mut trace = Trace::default();
        for (index, (value, expected)) in values.into_iter().zip(&signature.parameters).enumerate()
        {
            let conversion = value
                .descriptor
                .coherent()
                .conversion_to(expected)
                .map_err(|error| {
                    let mut diagnostic = self.error(
                        source,
                        error.into(),
                        Expectation::Descriptor(Box::new(expected.clone())),
                        vec![value.descriptor.clone()],
                    );
                    diagnostic.location.path.push(index);
                    if let Node::Call(_, arguments) = source.node() {
                        diagnostic.location.span = arguments.get(index).and_then(Expr::span);
                    }
                    diagnostic
                })?;
            coordinates.push(self.convert(source, value.coordinates, conversion)?);
            conversions.push(conversion);
            trace.merge(value.trace);
        }
        let result = self.build(source, Node::Call(name.clone(), coordinates))?;
        let conversion = signature
            .result
            .conversion_to(&signature.result.coherent())
            .map_err(|error| self.units(source, error, vec![signature.result.clone()]))?;
        trace.unbound |= matches!(signature.result.kind(), Kind::Uncertainty(..));
        let value = Value {
            descriptor: signature.result,
            coordinates: self.convert(source, result, conversion)?,
            nonzero: false,
            literal: None,
            origin: None,
            companion: None,
            trace,
        };
        Ok((value, conversions))
    }

    /// Check arity with all successfully derived argument descriptors in the diagnostic.
    fn arity(&self, source: &Expr, expected: usize, values: &[Value]) -> Result<(), Diagnostic> {
        if values.len() != expected {
            return Err(self.error(
                source,
                Failure::Arity {
                    expected,
                    derived: values.len(),
                },
                Expectation::Rule("declared function arity"),
                values
                    .iter()
                    .map(|value| value.descriptor.clone())
                    .collect(),
            ));
        }
        Ok(())
    }

    /// Supply explicitly angle-aware elementary signatures or a client declaration.
    fn signature(&self, source: &Expr, name: &Symbol) -> Result<FunctionSignature, Diagnostic> {
        let angle = || {
            Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)
                .map_err(|error| self.units(source, error, Vec::new()))
        };
        match name.as_str() {
            "sin" | "cos" | "tan" => Ok(FunctionSignature {
                parameters: vec![angle()?],
                result: Descriptor::ratio(),
            }),
            "asin" | "acos" | "atan" => Ok(FunctionSignature {
                parameters: vec![Descriptor::ratio()],
                result: angle()?,
            }),
            "exp" | "log" => Ok(FunctionSignature {
                parameters: vec![Descriptor::ratio()],
                result: Descriptor::ratio(),
            }),
            _ => self.context.functions.get(name).cloned().ok_or_else(|| {
                self.error(
                    source,
                    Failure::UnknownFunction(name.clone()),
                    Expectation::Rule("a declared function signature"),
                    Vec::new(),
                )
            }),
        }
    }
}
