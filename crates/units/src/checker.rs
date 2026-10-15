// SPDX-License-Identifier: AGPL-3.0-only
//! Original-operation checking with bounded declaration expansion.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

mod memo;
use memo::{DepthCheck, Memo};

#[cfg(test)]
mod tests;

use crate::coordinates::{Coordinate, Coordinates};
use crate::exact::ratio;
use crate::original::{Source, spans, unsupported};
use nrz_cas::limits::Budget;
use nrz_cas::source::{OriginalForm, WalkEvent};
use nrz_cas::syntax::{Branch, Operator};
use nrz_cas::{Cas, Handle, Location as CasLocation, Rational, Reference};

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
    /// TODO ´todo:code:provide-reusable-original-expression-evidence-for´: provide reusable original expression evidence for every declared plan alternative under rule:nrz:plan-validation.
    ///
    /// # Errors
    /// Reports unknown names, cycles, metadata operands, physical refusals and limits.
    pub fn check(&self, expression: &OriginalForm) -> Result<CheckedExpression, Diagnostic> {
        let mut cas = self
            .engine()
            .map_err(|error| Diagnostic::declaration(error.into()))?;
        let mut state = State::new(self, &mut cas)?;
        let (value, _) = state.walk(expression, 1)?;
        let coordinates = state.publish(&value.coordinates)?;
        Ok(CheckedExpression {
            original: expression.clone(),
            descriptor: value.descriptor,
            nodes: state.nodes,
            coordinates,
            declarations: state.declarations,
            output: None,
            companion: value.companion,
        })
    }

    /// Check an original expression against an explicit output descriptor.
    ///
    /// TODO ´todo:code:compare-declared-output-axes-and-companion´: compare declared output axes and companion extents before exposing converted matrix results under rule:nrz:shape-data-time-evidence.
    ///
    /// TODO ´todo:code:recheck-bound-output-conversions-when-declaration´: recheck bound output conversions when declaration snapshots differ between bindings under rule:nrz:binding-validation.
    ///
    /// # Errors
    /// Reports expression failures or a dimension, kind, role or measure mismatch.
    pub fn check_against(
        &self,
        expression: &OriginalForm,
        expected: &Descriptor,
    ) -> Result<CheckedExpression, Diagnostic> {
        let mut checked = self.check(expression)?;
        let failure = |error: UnitError| Diagnostic {
            location: Box::new(Location {
                path: Vec::new(),
                span: checked.nodes.last().and_then(|node| node.location.span),
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
    /// TODO ´todo:code:retain-equation-side-evidence-when-clients´: retain equation side evidence when clients assign checked expressions to plan nodes under rule:nrz:plan-validation.
    ///
    /// # Errors
    /// Reports either original expression's failure or incompatible side descriptors.
    pub fn check_equation(
        &self,
        left: &OriginalForm,
        right: &OriginalForm,
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
    /// TODO ´todo:code:reuse-block-checks-only-for-matching´: reuse block checks only for matching declaration snapshots and slot schemas under rule:nrz:binding-validation.
    ///
    /// # Errors
    /// Returns the first invalid entry with its index prepended to the original path.
    pub fn check_block(
        &self,
        entries: &[OriginalForm],
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
    /// TODO ´todo:code:check-declarations-for-every-permitted-alternative´: check declarations for every permitted alternative before client binding selection under rule:nrz:plan-validation.
    ///
    /// # Errors
    /// Reports the first failing declaration in deterministic symbol order.
    pub fn check_all(&self) -> Result<BTreeMap<String, CheckedExpression>, Diagnostic> {
        let mut result = BTreeMap::new();
        for (name, declaration) in &self.declarations {
            if declaration.descriptor().is_some() {
                let expression = self.parse(&format!("units:probe:{name}"), name)?;
                result.insert(name.to_owned(), self.check(&expression)?);
            }
        }
        Ok(result)
    }
}

/// A checked subtree, with coordinates in coherent measure.
///
/// TODO ´todo:code:retain-original-entry-identities-and-logical´: retain original entry identities and logical maps for shaped intermediate values under def:nrz:companion-shape.
///
/// TODO ´todo:code:separate-reusable-subtree-contracts-from-concrete´: separate reusable subtree contracts from concrete binding value assignments under def:nrz:plan-binding.
#[derive(Clone)]
struct Value {
    /// Physical interpretation of the original subtree.
    descriptor: Descriptor,
    /// Converted scalar coordinates after original checks.
    coordinates: Coordinate,
    /// Explicit nonzero assumption or exact literal evidence only.
    nonzero: bool,
    /// Known bare or typed ratio constant, never general numerical evaluation.
    literal: Option<Rational>,
    /// Original symbol identity, absent after any operation.
    origin: Option<String>,
    /// Value and reference identities established by supported uncertainty operations.
    companion: Option<CompanionEvidence>,
    /// Dependencies and uncertainty operands reached before any algebraic rewriting.
    trace: Trace,
}

/// Dependency and contributor evidence collected from original expression operands.
///
/// TODO ´todo:code:track-indexed-dependencies-and-paired-uncertainty´: track indexed dependencies and paired uncertainty contributors before zero or cancellation rewrites under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:retain-declaration-snapshot-keys-for-dependencies´: retain declaration snapshot keys for dependencies used by reusable checks under rule:nrz:computation-cache-validity.
#[derive(Clone, Default)]
struct Trace {
    /// All declarations reached by this subtree, including resolved derived dependencies.
    dependencies: BTreeSet<String>,
    /// Bound uncertainty operands, indexed by their original declaration names.
    contributors: BTreeMap<String, Companion>,
    /// An uncertainty operand lacked a checked value relationship.
    unbound: bool,
}

impl Trace {
    /// Combine independently checked operands without discarding cancelled contributions.
    ///
    /// TODO ´todo:code:merge-contributor-entry-maps-without-erasing´: merge contributor entry maps without erasing shared source dependence under def:nrz:companion-shape and rule:nrz:shape-sensitivity-propagation.
    ///
    /// TODO ´todo:code:reject-merging-contributors-from-inconsistent-shared´: reject merging contributors from inconsistent shared source declaration snapshots under rule:nrz:batch-source-covariance.
    fn merge(&mut self, other: Self) {
        self.dependencies.extend(other.dependencies);
        self.contributors.extend(other.contributors);
        self.unbound |= other.unbound;
    }
}

/// Value dependencies that may supply uncertainty contributors to an explicit propagation.
///
/// TODO ´todo:code:restrict-covariance-contributors-to-the-declared´: restrict covariance contributors to the declared paired target closures and contraction axes under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:instantiate-dependency-closures-against-explicit-binding´: instantiate dependency closures against explicit binding point and slot identities under def:nrz:plan-binding.
struct PropagationScope {
    /// The primary value and, for covariance, the second value's dependency closures.
    values: BTreeSet<String>,
    /// Propagation requires a derived primary value or an explicit covariance pair.
    admitted: bool,
}

/// One bounded traversal and its original-operation evidence.
struct State<'a> {
    /// Immutable physical declarations and rules.
    context: &'a Context,
    /// Owning engine used for inspection and coordinate construction.
    cas: &'a mut Cas,
    /// Shared ledger includes walks, declaration expansion and coordinate construction.
    budget: Budget,
    /// Successful derived values belong to this check's immutable declaration snapshot.
    memo: BTreeMap<String, Rc<Memo<'a>>>,
    /// Ordered depth checks allow cached bodies to preserve deeper-use refusals exactly.
    depth_checks: Vec<DepthCheck>,
    /// Child-before-parent coordinate drafts admitted in one CAS transaction.
    coordinates: Coordinates,
    /// Original operation responsible for the most recent coordinate draft.
    coordinate_location: Location,
    /// Current original operand path.
    path: Vec<usize>,
    /// Active declaration chain used for cycle detection.
    stack: Vec<String>,
    /// Declarations whose expression or relationship is the current diagnostic source.
    definitions: Vec<String>,
    /// Expected companion used to disambiguate ratio references from scaling coefficients.
    expected: Option<Companion>,
    /// Postorder evidence retained in the successful result.
    nodes: Vec<NodeEvidence>,
    /// Owned snapshots of declarations reached by this traversal.
    declarations: BTreeMap<String, Declaration>,
}

impl<'a> State<'a> {
    /// Start a fresh ledger, declaration memo and evidence inventory for one check.
    fn new(context: &'a Context, cas: &'a mut Cas) -> Result<Self, Diagnostic> {
        let budget = Budget::new(context.limits.clone())
            .map_err(|error| Diagnostic::declaration(error.into()))?;
        Ok(Self {
            context,
            cas,
            budget,
            coordinates: Coordinates::default(),
            coordinate_location: Location {
                path: Vec::new(),
                span: None,
                definitions: Vec::new(),
            },
            path: Vec::new(),
            stack: Vec::new(),
            definitions: Vec::new(),
            expected: None,
            nodes: Vec::new(),
            declarations: BTreeMap::new(),
            memo: BTreeMap::new(),
            depth_checks: Vec::new(),
        })
    }

    /// Capture the current source location before any rewriting.
    fn location(&self, source: &Source) -> Location {
        Location {
            path: self.path.clone(),
            span: source.span,
            definitions: self.definitions.clone(),
        }
    }

    /// Make a structured refusal at the original operation.
    fn error(
        &self,
        source: &Source,
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
    fn units(&self, source: &Source, failure: UnitError, derived: Vec<Descriptor>) -> Diagnostic {
        self.error(
            source,
            failure.into(),
            Expectation::Rule(
                "compatible dimensions, explicit kinds, affine roles and count domains",
            ),
            derived,
        )
    }

    /// Stage coordinates while checking their size and depth at the original operation.
    fn build(
        &mut self,
        source: &Source,
        operator: Operator,
        operands: &[Coordinate],
    ) -> Result<Coordinate, Diagnostic> {
        let coordinate = self
            .coordinates
            .push(&source.header, operator, operands, &self.budget)
            .map_err(|error| {
                self.error(
                    source,
                    Failure::Coordinates(error),
                    Expectation::Rule("bounded exact coordinate expression"),
                    Vec::new(),
                )
            })?;
        self.coordinate_location = self.location(source);
        Ok(coordinate)
    }

    /// Admit all checked coordinate drafts in one journaled transaction on the shared ledger.
    fn publish(&mut self, coordinate: &Coordinate) -> Result<Handle, Diagnostic> {
        self.coordinates
            .publish(self.cas, coordinate, &mut self.budget)
            .map_err(|error| Diagnostic {
                location: Box::new(self.coordinate_location.clone()),
                expected: Expectation::Rule("bounded exact coordinate expression"),
                derived: Vec::new(),
                failure: Failure::Coordinates(error),
            })
    }

    /// Apply an exact coordinate map without normalizing original operations.
    ///
    /// TODO ´todo:code:apply-axis-conversions-to-values-and´: apply axis conversions to values and both covariance indices while excluding affine uncertainty offsets under def:nrz:covariance-shape.
    fn convert(
        &mut self,
        source: &Source,
        mut coordinates: Coordinate,
        conversion: &Conversion,
    ) -> Result<Coordinate, Diagnostic> {
        if conversion.scale != Rational::one() {
            let scale = self.build(source, Operator::Rational(conversion.scale.clone()), &[])?;
            coordinates = self.build(source, Operator::Mul, &[scale, coordinates])?;
        }
        if conversion.shift != Rational::zero() {
            let shift = self.build(source, Operator::Rational(conversion.shift.clone()), &[])?;
            coordinates = self.build(source, Operator::Add, &[coordinates, shift])?;
        }
        Ok(coordinates)
    }

    /// Visit all original occurrences and inspect their syntax before checking parent operations.
    fn walk(
        &mut self,
        original: &OriginalForm,
        depth: usize,
    ) -> Result<(Value, Source), Diagnostic> {
        let walk = self
            .cas
            .walk_original(&original.root, &mut self.budget)
            .map_err(|error| Diagnostic::declaration(error.fault.into()))?;
        let source_spans = spans(&walk.events);
        let base_path = self.path.clone();
        let mut values = BTreeMap::new();
        let mut root = None;
        for event in walk.events {
            let WalkEvent::Visit(visit) = event else {
                return Err(Diagnostic::declaration(unsupported().into()));
            };
            self.path.clone_from(&base_path);
            for index in &visit.path {
                self.path.push(
                    usize::try_from(*index)
                        .map_err(|_| Diagnostic::declaration(unsupported().into()))?,
                );
            }
            let source = Source::inspect(self.cas, &visit, &source_spans, &mut self.budget)
                .map_err(|error| Diagnostic::declaration(error.into()))?;
            if visit.path.is_empty() && visit.expression != original.syntax {
                return Err(Diagnostic::declaration(unsupported().into()));
            }
            let children = original_children(&visit.path, source.arguments.len(), &mut values)?;
            let value = self.operation(&source, children, depth + visit.path.len())?;
            if visit.path.is_empty() {
                root = Some(source);
            }
            values.insert(visit.path, value);
        }
        self.path = base_path;
        let value = values
            .remove(&Vec::new())
            .ok_or_else(|| Diagnostic::declaration(unsupported().into()))?;
        let source = root.ok_or_else(|| Diagnostic::declaration(unsupported().into()))?;
        Ok((value, source))
    }

    /// Retain postorder evidence after each original operator's physical rule succeeds.
    fn operation(
        &mut self,
        source: &Source,
        values: Vec<Value>,
        depth: usize,
    ) -> Result<Value, Diagnostic> {
        let (value, arguments) = self.evaluate(source, values, depth)?;
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

    /// Interpret only the scalar operator family admitted by the physical checker.
    fn evaluate(
        &mut self,
        source: &Source,
        values: Vec<Value>,
        depth: usize,
    ) -> Result<(Value, Vec<Conversion>), Diagnostic> {
        self.depth(source, depth)?;
        if let Operator::Call(name) = &source.operator {
            return self.call(source, name, values);
        }
        let mut values = values.into_iter();
        let value = match &source.operator {
            Operator::Rational(value) => Value {
                descriptor: Descriptor::ratio(),
                coordinates: Coordinate::leaf(source.expression.clone()),
                nonzero: !value.is_zero(),
                literal: Some(value.clone()),
                origin: None,
                companion: None,
                trace: Trace::default(),
            },
            Operator::Free(name) => self.symbol(source, name, depth)?,
            Operator::Neg => self.negate(source, next_value(&mut values)?)?,
            Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => {
                let operation = match source.operator {
                    Operator::Add => BinaryOp::Add,
                    Operator::Sub => BinaryOp::Subtract,
                    Operator::Mul => BinaryOp::Multiply,
                    _ => BinaryOp::Divide,
                };
                self.binary(
                    source,
                    operation,
                    next_value(&mut values)?,
                    next_value(&mut values)?,
                )?
            }
            Operator::IntegerPower(exponent) => {
                let exponent = ratio(*exponent, 1)
                    .map_err(|error| self.units(source, error.into(), Vec::new()))?;
                self.power(source, next_value(&mut values)?, &exponent)?
            }
            Operator::GeneralPower(Branch::StrictReal) => {
                let base = next_value(&mut values)?;
                let exponent = next_value(&mut values)?
                    .literal
                    .ok_or_else(|| Diagnostic::declaration(unsupported().into()))?;
                self.power(source, base, &exponent)?
            }
            _ => {
                return Err(self.error(
                    source,
                    unsupported().into(),
                    Expectation::Rule("supported scalar original operation"),
                    Vec::new(),
                ));
            }
        };
        Ok((value, Vec::new()))
    }

    /// Bound recursive declaration expansion as well as paths inside each original body.
    fn depth(&mut self, source: &Source, depth: usize) -> Result<(), Diagnostic> {
        let location = self.location(source);
        self.record_depth(location, depth)
    }

    /// Negation preserves companion provenance while refusing affine points and count domains.
    fn negate(&mut self, source: &Source, value: Value) -> Result<Value, Diagnostic> {
        let descriptor = self
            .context
            .registry
            .negate(&value.descriptor)
            .map_err(|error| self.units(source, error, vec![value.descriptor.clone()]))?;
        let literal = value
            .literal
            .as_ref()
            .map(|value| value.negated(&mut self.budget))
            .transpose()
            .map_err(|error| self.units(source, error.into(), vec![value.descriptor]))?;
        Ok(Value {
            descriptor,
            coordinates: self.build(
                source,
                Operator::Neg,
                std::slice::from_ref(&value.coordinates),
            )?,
            nonzero: false,
            literal,
            origin: None,
            companion: value.companion,
            trace: value.trace,
        })
    }

    /// Validate and expand a numerical declaration, retaining its evidence snapshot.
    fn symbol(&mut self, source: &Source, name: &str, depth: usize) -> Result<Value, Diagnostic> {
        self.depth(source, depth)?;
        self.budget
            .work(1, CasLocation::new("units declaration expansion"))
            .map_err(|error| {
                self.error(
                    source,
                    error.into(),
                    Expectation::Rule("bounded declaration expansion"),
                    Vec::new(),
                )
            })?;
        if self.stack.iter().any(|active| active == name) {
            let mut cycle = self.stack.clone();
            cycle.push(name.to_owned());
            return Err(self.error(
                source,
                Failure::Cycle(cycle),
                Expectation::Rule("acyclic declarations"),
                Vec::new(),
            ));
        }
        let declaration = self.snapshot(source, name)?;
        if let Some(memo) = self.memo.get(name).cloned()
            && std::ptr::eq(memo.snapshot, declaration)
        {
            return self.reuse(source, depth, &memo);
        }
        let checkpoint = matches!(
            declaration,
            Declaration::Derived(_) | Declaration::Uncertainty { .. }
        )
        .then(|| self.checkpoint(source, depth));
        let value = self.expand(source, name, declaration.clone(), depth)?;
        if let Some(checkpoint) = checkpoint {
            self.remember(source, name, declaration, &value, checkpoint)?;
        }
        Ok(value)
    }

    /// Resolve one uncached declaration under its original diagnostic scopes.
    fn expand(
        &mut self,
        source: &Source,
        name: &str,
        declaration: Declaration,
        depth: usize,
    ) -> Result<Value, Diagnostic> {
        self.declarations
            .insert(name.to_owned(), declaration.clone());
        let nonzero = declaration.is_nonzero();
        let descriptor = declaration
            .descriptor()
            .ok_or_else(|| {
                self.error(
                    source,
                    Failure::Metadata(name.to_owned()),
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
        self.stack.push(name.to_owned());
        if has_definition {
            self.definitions.push(name.to_owned());
        }
        let result = self.resolve(source, declaration, &descriptor, depth);
        if has_definition {
            self.definitions.pop();
        }
        self.stack.pop();
        let mut value = result?;
        value.descriptor = descriptor;
        value.origin = Some(name.to_owned());
        value.nonzero = nonzero;
        value.trace.dependencies.insert(name.to_owned());
        if let Some(evidence) = &value.companion {
            value.trace.contributors =
                BTreeMap::from([(name.to_owned(), evidence.companion().clone())]);
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
        source: &Source,
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
            Declaration::Input { .. } => self.convert(
                source,
                Coordinate::leaf(source.expression.clone()),
                &conversion,
            )?,
            Declaration::Constant { value, .. } => {
                if descriptor.kind() == &Kind::Ratio {
                    literal =
                        Some(conversion.apply(&value).map_err(|error| {
                            self.units(source, error, vec![descriptor.clone()])
                        })?);
                }
                let coordinates = self.build(source, Operator::Rational(value), &[])?;
                self.convert(source, coordinates, &conversion)?
            }
            Declaration::UncertaintyInput {
                companion: relation,
                ..
            } => {
                let location = Source {
                    span: None,
                    expression: source.expression.clone(),
                    header: source.header.clone(),
                    operator: source.operator.clone(),
                    arguments: Vec::new(),
                };
                self.companion(&location, &relation, descriptor, depth)?;
                companion = Some(CompanionEvidence::Established(relation));
                self.convert(
                    source,
                    Coordinate::leaf(source.expression.clone()),
                    &conversion,
                )?
            }
            Declaration::Derived(observable) => {
                return self.observable(&observable.expression, descriptor, None, depth);
            }
            Declaration::Uncertainty {
                observable,
                companion,
            } => {
                self.path.push(0);
                let location = self.original_source(&observable.expression)?;
                let scope = self.companion(&location, &companion, descriptor, depth);
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
        expression: &OriginalForm,
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
    ///
    /// TODO ´todo:code:separate-checked-observable-contracts-from-their´: separate checked observable contracts from their concrete point instantiations under rule:nrz:binding-validation.
    fn observable_body(
        &mut self,
        expression: &OriginalForm,
        descriptor: &Descriptor,
        companion: Option<(&Companion, &PropagationScope)>,
        depth: usize,
    ) -> Result<Value, Diagnostic> {
        let previous = std::mem::replace(
            &mut self.expected,
            companion.map(|(companion, _)| companion.clone()),
        );
        let result = self.walk(expression, depth + 1);
        self.expected = previous;
        let (mut value, source) = result?;
        value
            .descriptor
            .conversion_to(descriptor)
            .map_err(|error| {
                self.error(
                    &source,
                    error.into(),
                    Expectation::Descriptor(Box::new(descriptor.clone())),
                    vec![value.descriptor.clone()],
                )
            })?;
        if let Some((companion, scope)) = companion
            && value.companion.as_ref().map(CompanionEvidence::companion) != Some(companion)
        {
            self.propagate(&source, descriptor, companion, scope, &mut value)?;
        }
        Ok(value)
    }

    /// Bind an explicitly declared propagation only to contributors inside its value closures.
    ///
    /// TODO ´todo:code:validate-indexed-covariance-contractions-while-leaving´: validate indexed covariance contractions while leaving derivative correctness and numerical assurance separate under rule:nrz:shape-data-time-evidence.
    ///
    /// TODO ´todo:code:preserve-common-source-keys-when-instantiated´: preserve common source keys when instantiated contributors cross binding points under rule:nrz:batch-source-covariance.
    fn propagate(
        &self,
        expression: &Source,
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
    fn relationship_value(&mut self, name: &str, depth: usize) -> Result<Value, Diagnostic> {
        let header = self
            .cas
            .context_header(&self.context.semantic)
            .map_err(|error| Diagnostic::declaration(error.into()))?;
        let mut source = Source {
            expression: Reference::Existing(header.clone()),
            header,
            operator: Operator::Free(name.to_owned()),
            span: None,
            arguments: Vec::new(),
        };
        source.expression = self.build(&source, source.operator.clone(), &[])?.reference;
        self.symbol(&source, name, depth + 1)
    }

    /// Read a declaration root from a complete original occurrence walk.
    fn original_source(&mut self, original: &OriginalForm) -> Result<Source, Diagnostic> {
        Source::root(self.cas, original, &mut self.budget)
            .map_err(|error| Diagnostic::declaration(error.into()))
    }

    /// Check the companion's descriptors at its own expression source.
    ///
    /// TODO ´todo:code:resolve-companion-entry-axes-and-relative´: resolve companion entry axes and relative references before accepting relationship evidence under def:nrz:companion-shape and rule:nrz:correlation-and-relative-shapes.
    ///
    /// TODO ´todo:code:reject-relationship-instantiation-against-stale-bound´: reject relationship instantiation against stale bound target versions under rule:nrz:binding-validation.
    fn companion(
        &mut self,
        source: &Source,
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
        source: &Source,
        operation: BinaryOp,
        left: Value,
        right: Value,
    ) -> Result<Value, Diagnostic> {
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
        let operands = [left.coordinates, right.coordinates];
        let operator = match operation {
            BinaryOp::Add => Operator::Add,
            BinaryOp::Subtract => Operator::Sub,
            BinaryOp::Multiply => Operator::Mul,
            BinaryOp::Divide => Operator::Div,
        };
        Ok(Value {
            descriptor,
            coordinates: self.build(source, operator, &operands)?,
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
                return registry.power(&left.descriptor, &ratio(2, 1)?);
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
                        right.literal.as_ref().ok_or(UnitError::CountDomain)?,
                    );
                }
                if right.descriptor.kind().count_domain().is_some()
                    && left.descriptor.kind() == &Kind::Ratio
                {
                    return registry.scale_count(
                        &right.descriptor,
                        left.literal.as_ref().ok_or(UnitError::CountDomain)?,
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
    fn power(
        &mut self,
        source: &Source,
        base: Value,
        exponent: &Rational,
    ) -> Result<Value, Diagnostic> {
        let descriptor = self
            .context
            .registry
            .power(&base.descriptor, exponent)
            .map_err(|error| self.units(source, error, vec![base.descriptor.clone()]))?;
        let preserves = (exponent == &Rational::one()
            && descriptor.kind() == base.descriptor.kind())
            || (exponent
                == &ratio(2, 1).map_err(|error| self.units(source, error.into(), Vec::new()))?
                && Self::variance_pair(base.descriptor.kind(), descriptor.kind()))
            || (exponent
                == &ratio(1, 2).map_err(|error| self.units(source, error.into(), Vec::new()))?
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
            coordinates: self.power_coordinates(source, base.coordinates, exponent)?,
            nonzero: false,
            literal: None,
            origin: None,
            companion,
            trace,
        })
    }

    /// Encode an exact power using the CAS's integer or strict-real constructor.
    fn power_coordinates(
        &mut self,
        source: &Source,
        base: Coordinate,
        exponent: &Rational,
    ) -> Result<Coordinate, Diagnostic> {
        let components = exponent
            .small_ratio(&mut self.budget)
            .map_err(|error| self.units(source, error.into(), Vec::new()))?;
        if let Some((power, 1)) = components {
            return self.build(
                source,
                Operator::IntegerPower(power),
                std::slice::from_ref(&base),
            );
        }
        let exponent = self.build(source, Operator::Rational(exponent.clone()), &[])?;
        self.build(
            source,
            Operator::GeneralPower(Branch::StrictReal),
            &[base, exponent],
        )
    }

    /// Check special relationships or a fixed function signature after all argument visits.
    ///
    /// TODO ´todo:code:require-declared-shape-and-dependency-signatures´: require declared shape and dependency signatures for matrix valued functions under rule:nrz:shape-data-time-evidence.
    fn call(
        &mut self,
        source: &Source,
        name: &str,
        values: Vec<Value>,
    ) -> Result<(Value, Vec<Conversion>), Diagnostic> {
        if matches!(name, "sqrt" | "count_ratio") {
            return self.special_call(source, name, values);
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
                    diagnostic.location.span = source.arguments.get(index).copied().flatten();
                    diagnostic
                })?;
            coordinates.push(self.convert(source, value.coordinates, &conversion)?);
            conversions.push(conversion);
            trace.merge(value.trace);
        }
        let result = self.build(source, Operator::Call(name.to_owned()), &coordinates)?;
        let conversion = signature
            .result
            .conversion_to(&signature.result.coherent())
            .map_err(|error| self.units(source, error, vec![signature.result.clone()]))?;
        trace.unbound |= matches!(signature.result.kind(), Kind::Uncertainty(..));
        let value = Value {
            descriptor: signature.result,
            coordinates: self.convert(source, result, &conversion)?,
            nonzero: false,
            literal: None,
            origin: None,
            companion: None,
            trace,
        };
        Ok((value, conversions))
    }

    /// Root and explicit count-ratio calls retain their special physical relationships.
    fn special_call(
        &mut self,
        source: &Source,
        name: &str,
        mut values: Vec<Value>,
    ) -> Result<(Value, Vec<Conversion>), Diagnostic> {
        let expected = if name == "sqrt" { 1 } else { 2 };
        self.arity(source, expected, &values)?;
        let first = values.remove(0);
        if name == "sqrt" {
            let half = ratio(1, 2).map_err(|error| self.units(source, error.into(), Vec::new()))?;
            return Ok((self.power(source, first, &half)?, Vec::new()));
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
            Operator::Div,
            &[first.coordinates, second.coordinates],
        )?;
        let mut trace = first.trace;
        trace.merge(second.trace);
        Ok((
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
        ))
    }

    /// Check arity with all successfully derived argument descriptors in the diagnostic.
    fn arity(&self, source: &Source, expected: usize, values: &[Value]) -> Result<(), Diagnostic> {
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
    fn signature(&self, source: &Source, name: &str) -> Result<FunctionSignature, Diagnostic> {
        let angle = || {
            Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)
                .map_err(|error| self.units(source, error, Vec::new()))
        };
        match name {
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
                    Failure::UnknownFunction(name.to_owned()),
                    Expectation::Rule("a declared function signature"),
                    Vec::new(),
                )
            }),
        }
    }
}

/// Remove one already visited operand; malformed trees remain refusals.
fn next_value(values: &mut std::vec::IntoIter<Value>) -> Result<Value, Diagnostic> {
    values
        .next()
        .ok_or_else(|| Diagnostic::declaration(unsupported().into()))
}

/// Collect already validated operands by distinct original occurrence paths.
fn original_children(
    path: &[u32],
    count: usize,
    values: &mut BTreeMap<Vec<u32>, Value>,
) -> Result<Vec<Value>, Diagnostic> {
    let mut children = Vec::new();
    for index in 0..count {
        let mut child = path.to_vec();
        child
            .push(u32::try_from(index).map_err(|_| Diagnostic::declaration(unsupported().into()))?);
        children.push(
            values
                .remove(&child)
                .ok_or_else(|| Diagnostic::declaration(unsupported().into()))?,
        );
    }
    Ok(children)
}
