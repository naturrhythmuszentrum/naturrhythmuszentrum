// SPDX-License-Identifier: AGPL-3.0-only
//! Exact real odd-root and positive/nonnegative square-root hypotheses.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, ExpressionAttachment, Handle};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{Element, Value};
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::scalar::Scalar;
use crate::solving::coefficients::storage::{AT, begin, invalid, polynomial};
use crate::solving::{RootRelationData, RootSelectionData, SelectionMode};
use std::cmp::Ordering;

/// A real-root subcase names precisely a monic binomial, including a zero radicand.
pub(crate) fn binomial(
    cas: &Cas,
    relation: &RootRelationData<Handle>,
    order: u32,
    budget: &mut Budget,
) -> Result<Option<Handle>, Fault> {
    let stored = polynomial(&cas.state, &relation.polynomial)?;
    let leading = stored.terms.first().ok_or_else(invalid)?;
    if leading.exponents.as_slice() != [order] || order == 0 || stored.terms.len() > 2 {
        return Err(invalid());
    }
    let engine = Engine {
        state: &cas.state,
        domain: &stored.domain,
    };
    if !engine
        .formal_equal(
            &engine
                .read(&leading.coefficient, budget)
                .map_err(|error| fault(&error))?,
            &engine
                .constant(true, budget)
                .map_err(|error| fault(&error))?,
            budget,
        )
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    if let Some(constant) = stored.terms.get(1) {
        if constant.exponents.as_slice() != [0] {
            return Err(invalid());
        }
        Ok(Some(constant.coefficient.clone()))
    } else {
        Ok(None)
    }
}

/// Extract only manifest rational constants; symbolic or selected tower values need sign premises.
fn rational(
    value: &Element,
    budget: &mut Budget,
) -> Result<Option<crate::number::Rational>, Fault> {
    begin(budget)?;
    match &value.value {
        Value::Scalar(Scalar::Rational(value)) => Ok(Some(
            crate::solving::coefficients::scalar::rational_copy(value, budget)?,
        )),
        Value::Scalar(Scalar::Gaussian(value)) => {
            if value.imaginary(budget)?.is_zero() {
                Ok(Some(value.real(budget)?))
            } else {
                Ok(None)
            }
        }
        Value::Tower(coefficients) => match coefficients.as_slice() {
            [] => {
                budget.arithmetic(1, AT)?;
                Ok(Some(crate::number::Rational::zero()))
            }
            [constant] => rational(constant, budget),
            _ => Ok(None),
        },
        Value::Fraction(fraction) => {
            match (
                fraction.numerator.scalar(budget)?,
                fraction.denominator.scalar(budget)?,
            ) {
                (Some(Scalar::Rational(top)), Some(Scalar::Rational(bottom))) => {
                    Ok(Some(top.div(&bottom, budget)?))
                }
                _ => Ok(None),
            }
        }
    }
}

/// The positive selection predicate refers to its actual bound root slot.
pub(crate) fn positive_selection(
    cas: &Cas,
    selection: &RootSelectionData<Handle>,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    budget.work(1, AT)?;
    if selection.mode != SelectionMode::Constrained {
        return Err(invalid());
    }
    let crate::graph::Vertex::Condition(predicate) =
        cas.state.node(selection.predicate.logical_id())?
    else {
        return Err(invalid());
    };
    let strict = match predicate.kind {
        PredicateKind::Positive => true,
        PredicateKind::NonNegative => false,
        _ => return Err(invalid()),
    };
    let [argument] = predicate.arguments.as_slice() else {
        return Err(invalid());
    };
    if cas.state.expression(*argument)?.operator != (Operator::Bound { depth: 0, slot: 0 }) {
        return Err(invalid());
    }
    Ok(strict)
}

/// A supplied sign fact must refer to -c for this exact binomial's constant c and scalar map.
fn sign_premise(
    cas: &Cas,
    constant: &Handle,
    relation: &RootRelationData<Handle>,
    strict: bool,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    for premise in premises {
        budget.work(1, AT)?;
        let Statement::PredicateFact { region, predicate } = &premise.judgment.statement else {
            continue;
        };
        if *region != relation.region {
            continue;
        }
        let crate::graph::Vertex::Condition(condition) = cas.state.node(predicate.logical_id())?
        else {
            return Err(invalid());
        };
        if condition.kind != PredicateKind::Positive
            && (strict || condition.kind != PredicateKind::NonNegative)
        {
            continue;
        }
        let [argument] = condition.arguments.as_slice() else {
            continue;
        };
        let expression = cas.state.expression(*argument)?;
        if expression.operator != Operator::Neg {
            continue;
        }
        let [operand] = expression.operands.as_slice() else {
            continue;
        };
        let literal = cas.state.expression(*operand)?;
        if matches!(&literal.attachment, Some(ExpressionAttachment::ExactCoefficient { value, map }) if *value == constant.logical_id() && *map == relation.map.logical_id())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Nonnegative or strictly positive radicands are proved exactly; no principal branch is inferred.
pub(crate) fn sign(
    cas: &Cas,
    relation: &RootRelationData<Handle>,
    constant: Option<&Handle>,
    strict: bool,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Some(constant) = constant else {
        return if strict { Err(invalid()) } else { Ok(()) };
    };
    let stored = crate::solving::coefficients::storage::value(&cas.state, constant)?;
    let engine = Engine {
        state: &cas.state,
        domain: &stored.domain,
    };
    if let Some(value) = rational(
        &engine
            .read(constant, budget)
            .map_err(|error| fault(&error))?,
        budget,
    )? {
        let order = value.compare(&crate::number::Rational::zero(), budget)?;
        if order == Ordering::Less || (!strict && order == Ordering::Equal) {
            return Ok(());
        }
        return Err(invalid());
    }
    if sign_premise(cas, constant, relation, strict, premises, budget)? {
        Ok(())
    } else {
        Err(invalid())
    }
}
