// SPDX-License-Identifier: AGPL-3.0-only
//! Three-valued universal box truth, with exact affine extrema and strict definedness.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::{AT, affine, bounds, number_bytes, temporary, unsupported};
use crate::budget::Budget;
use crate::decision::CaseCut;
use crate::domain_model::{AmbientDomain, RegionExpr};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId, Vertex, size};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::number::Rational;

/// Whether a total predicate holds throughout the box, nowhere, or is unresolved.
#[derive(Clone, Copy)]
pub(super) enum Truth {
    /// Every point in the box satisfies the predicate.
    All,
    /// No point in the box satisfies the predicate.
    None,
    /// Exact bounds do not establish uniform truth on the box.
    Mixed,
}

impl Truth {
    /// Total negation exchanges uniform truth and falsity.
    const fn negate(self) -> Self {
        match self {
            Self::All => Self::None,
            Self::None => Self::All,
            Self::Mixed => Self::Mixed,
        }
    }

    /// Conjunction is uniformly false if either conjunct is uniformly false.
    const fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::None, _) | (_, Self::None) => Self::None,
            (Self::All, Self::All) => Self::All,
            _ => Self::Mixed,
        }
    }

    /// Disjunction is uniformly true if one disjunct covers the complete box.
    const fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::All, _) | (_, Self::All) => Self::All,
            (Self::None, Self::None) => Self::None,
            _ => Self::Mixed,
        }
    }
}

/// A checked scalar coefficient vector or total Boolean box bound.
pub(super) enum Value {
    /// Constant term followed by coefficients in the supplied coordinate order.
    Affine(Vec<Rational>),
    /// Independently reconstructed universal truth classification.
    Predicate(Truth),
}

/// Exact local node identities retain all previously checked dependencies.
type Values = BTreeMap<LogicalId, Value>;

/// Evaluate the dependency plan before requiring both complete region inclusions.
pub(super) fn check(
    cas: &Cas,
    cut: &CaseCut,
    header: LogicalId,
    roots: &[LogicalId; 2],
    order: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let values = build(cas, cut, header, roots[0], order, budget)?;
    for root in roots {
        match truth(&values, *root)? {
            Truth::All => {}
            Truth::None => return Err(Fault::invalid(InvalidCode::DecisionOutsideRegion, AT)),
            Truth::Mixed => {
                return Err(Fault::invalid(
                    InvalidCode::DecisionBoundaryIntersection,
                    AT,
                ));
            }
        }
    }
    Ok(())
}

/// Reconstruct every original operand; successful compilation proves global totality of this grammar.
pub(super) fn build(
    cas: &Cas,
    cut: &CaseCut,
    header: LogicalId,
    domain: LogicalId,
    order: &[LogicalId],
    budget: &mut Budget,
) -> Result<Values, Fault> {
    let ambient = &cas.state.region(domain)?.ambient;
    let mut values = Values::new();
    for id in order {
        budget.condition(size(values.len(), AT)?.saturating_add(1), AT)?;
        let retained = payload(&values, budget)?;
        let bytes = retained.saturating_add(temporary(cut.bounds.len(), budget)?);
        let value = budget.with_retained_payload(bytes, AT, |budget| {
            evaluate(cas, cut, header, ambient, *id, &values, budget)
        })?;
        values.insert(*id, value);
    }
    Ok(values)
}

/// Interpret one node only after its original typed dependencies have been checked.
fn evaluate(
    cas: &Cas,
    cut: &CaseCut,
    header: LogicalId,
    ambient: &AmbientDomain,
    id: LogicalId,
    values: &Values,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    match cas.state.node(id)? {
        Vertex::Region(value) => {
            frame(value.header, header, value.scope.is_empty())?;
            if value.ambient != *ambient {
                return Err(Fault::invalid(InvalidCode::Scope, AT));
            }
            Ok(Value::Predicate(region(&value.expression, values, budget)?))
        }
        Vertex::Condition(value) => {
            frame(value.header, header, value.scope.is_empty())?;
            Ok(Value::Predicate(predicate(
                &value.kind,
                &value.arguments,
                values,
                cut,
                budget,
            )?))
        }
        Vertex::Expression(value) => {
            let closed = value.scope.is_empty() && value.binding.is_none() && value.space.is_none();
            frame(value.header, header, closed)?;
            if let Operator::Predicate(kind) = &value.operator {
                return Ok(Value::Predicate(predicate(
                    kind,
                    &value.operands,
                    values,
                    cut,
                    budget,
                )?));
            }
            let numeric = matches!(
                &value.sort,
                Sort::Value(AtomicSort::Integer | AtomicSort::Scalar(Field::Real))
            );
            if !numeric || value.domain.is_some() || value.attachment.is_some() {
                return Err(unsupported());
            }
            Ok(Value::Affine(affine::build(
                &value.operator,
                &value.operands,
                id,
                &cut.bounds,
                values,
                budget,
            )?))
        }
        _ => Err(unsupported()),
    }
}

/// Open scopes and changed headers are unsupported coordinate transports.
fn frame(actual: LogicalId, expected: LogicalId, closed: bool) -> Result<(), Fault> {
    if actual != expected || !closed {
        Err(Fault::invalid(InvalidCode::Scope, AT))
    } else {
        Ok(())
    }
}

/// Read a previously reconstructed predicate or region classification.
fn truth(values: &Values, id: LogicalId) -> Result<Truth, Fault> {
    match values.get(&id) {
        Some(Value::Predicate(value)) => Ok(*value),
        _ => Err(unsupported()),
    }
}

/// Reconstruct native set constructors using total predicate truth only.
fn region(
    value: &RegionExpr<LogicalId>,
    values: &Values,
    budget: &mut Budget,
) -> Result<Truth, Fault> {
    match value {
        RegionExpr::Ambient => Ok(Truth::All),
        RegionExpr::Empty => Ok(Truth::None),
        RegionExpr::GuardTrue { base, predicate } => {
            Ok(truth(values, *base)?.and(truth(values, *predicate)?))
        }
        RegionExpr::GuardFalse { base, predicate } => {
            Ok(truth(values, *base)?.and(truth(values, *predicate)?.negate()))
        }
        RegionExpr::Meet(members) => boolean(members, values, true, budget),
        RegionExpr::Union(members) => boolean(members, values, false, budget),
        RegionExpr::Difference { base, excluded } => {
            Ok(truth(values, *base)?.and(truth(values, *excluded)?.negate()))
        }
    }
}

/// Fold every retained Boolean operand without discarding unsupported syntax.
fn boolean(
    arguments: &[LogicalId],
    values: &Values,
    conjunction: bool,
    budget: &mut Budget,
) -> Result<Truth, Fault> {
    let mut result = if conjunction { Truth::All } else { Truth::None };
    for id in arguments {
        budget.work(size(values.len(), AT)?.saturating_add(1), AT)?;
        let operand = truth(values, *id)?;
        result = if conjunction {
            result.and(operand)
        } else {
            result.or(operand)
        };
    }
    Ok(result)
}

/// Evaluate only admitted total affine comparisons and Boolean constructors.
fn predicate(
    kind: &PredicateKind,
    arguments: &[LogicalId],
    values: &Values,
    cut: &CaseCut,
    budget: &mut Budget,
) -> Result<Truth, Fault> {
    match kind {
        PredicateKind::And => boolean(arguments, values, true, budget),
        PredicateKind::Or => boolean(arguments, values, false, budget),
        PredicateKind::Not => {
            Ok(truth(values, *arguments.first().ok_or_else(unsupported)?)?.negate())
        }
        PredicateKind::Defined => Ok(Truth::All),
        PredicateKind::IntegerRange => integer_range(arguments, values, budget),
        PredicateKind::Positive
        | PredicateKind::NonNegative
        | PredicateKind::NonZero
        | PredicateKind::Equal => {
            let first = affine::get(values, *arguments.first().ok_or_else(unsupported)?)?;
            let extrema = if *kind == PredicateKind::Equal {
                let other = affine::get(values, *arguments.get(1).ok_or_else(unsupported)?)?;
                let difference = affine::combine(first, other, true, budget)?;
                bounds(&difference, &cut.bounds, budget)?
            } else {
                bounds(first, &cut.bounds, budget)?
            };
            comparison(kind, &extrema.0, &extrema.1, budget)
        }
        _ => Err(unsupported()),
    }
}

/// Strict endpoint comparisons preserve equality and boundary ownership exactly.
fn comparison(
    kind: &PredicateKind,
    lower: &Rational,
    upper: &Rational,
    budget: &mut Budget,
) -> Result<Truth, Fault> {
    let bottom = lower.compare(&Rational::zero(), budget)?;
    let top = upper.compare(&Rational::zero(), budget)?;
    let positive = bottom == Ordering::Greater;
    let negative = top == Ordering::Less;
    let zero = bottom == Ordering::Equal && top == Ordering::Equal;
    let (all, none) = match kind {
        PredicateKind::Positive => (positive, top != Ordering::Greater),
        PredicateKind::NonNegative => (bottom != Ordering::Less, negative),
        PredicateKind::Equal => (zero, positive || negative),
        PredicateKind::NonZero => (positive || negative, zero),
        _ => return Err(unsupported()),
    };
    Ok(if all {
        Truth::All
    } else if none {
        Truth::None
    } else {
        Truth::Mixed
    })
}

/// Constant integer predicates use exact half-open endpoint comparisons.
fn integer_range(
    arguments: &[LogicalId],
    values: &Values,
    budget: &mut Budget,
) -> Result<Truth, Fault> {
    let subject = affine::get(values, *arguments.first().ok_or_else(unsupported)?)?;
    let lower = affine::get(values, *arguments.get(1).ok_or_else(unsupported)?)?;
    let upper = affine::get(values, *arguments.get(2).ok_or_else(unsupported)?)?;
    for value in [subject, lower, upper] {
        if !affine::is_constant(value, budget)?
            || !value.first().ok_or_else(unsupported)?.is_integer()
        {
            return Err(unsupported());
        }
    }
    let subject = subject.first().ok_or_else(unsupported)?;
    let low = subject.compare(lower.first().ok_or_else(unsupported)?, budget)? != Ordering::Less;
    let high = subject.compare(upper.first().ok_or_else(unsupported)?, budget)? == Ordering::Less;
    Ok(if low && high { Truth::All } else { Truth::None })
}

/// Retain actual coefficient sizes and conservative tree/vector overhead between node evaluations.
pub(super) fn payload(values: &Values, budget: &mut Budget) -> Result<u64, Fault> {
    let mut bytes = 1_024_u64;
    for value in values.values() {
        budget.work(1, AT)?;
        bytes = bytes.saturating_add(256);
        if let Value::Affine(coefficients) = value {
            for coefficient in coefficients {
                budget.work(1, AT)?;
                bytes = bytes.saturating_add(number_bytes(coefficient));
            }
        }
    }
    Ok(bytes)
}
