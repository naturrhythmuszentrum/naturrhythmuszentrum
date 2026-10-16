// SPDX-License-Identifier: AGPL-3.0-only
//! Exact scoped equality evidence with an explicit, local interpretation of polynomial slots.

use super::forms::{Form, Variable};
use super::normalize::Interpretation;
use super::proof::fault;
use super::storage::{AT, invalid, polynomial};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Handle, Vertex};
use crate::model::{AtomicSort, PredicateKind, Sort};
use crate::proof_model::Statement;
use crate::solving::{RelationMultiple, ScopedCoefficientRelation};

/// Compare independently evaluated forms without identifying scalar atoms with formal slots.
pub(crate) fn equal(
    interpreter: &Interpretation<'_>,
    left: &Form,
    right: &Form,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let negative = right
        .neg(&interpreter.engine, budget)
        .map_err(|error| fault(&error))?;
    let difference = left
        .add(&negative, &interpreter.engine, budget)
        .map_err(|error| fault(&error))?;
    Ok(difference.terms.is_empty())
}

/// Substitute only the explicitly supplied root terms into this polynomial's own formal slots.
pub(crate) fn interpreted(
    interpreter: &Interpretation<'_>,
    handle: &Handle,
    roots: &[Handle],
    budget: &mut Budget,
) -> Result<Form, Fault> {
    let stored = polynomial(&interpreter.cas.state, handle)?;
    if stored.variables.len() != roots.len() {
        return Err(invalid());
    }
    let mut values = super::storage::vector(roots.len(), budget)?;
    for (root, field) in roots.iter().zip(&stored.variables) {
        let expression = interpreter
            .cas
            .state
            .expression(interpreter.cas.checked(root)?)?;
        if expression.sort != Sort::Value(AtomicSort::Scalar(field.clone())) {
            return Err(invalid());
        }
        values.push(
            interpreter
                .endpoint(root, budget)
                .map_err(|error| fault(&error))?,
        );
    }
    let raw = interpreter
        .endpoint(handle, budget)
        .map_err(|error| fault(&error))?;
    let engine = &interpreter.engine;
    let mut result = Form { terms: Vec::new() };
    for term in raw.terms {
        let mut product =
            Form::constant(term.coefficient, engine, budget).map_err(|error| fault(&error))?;
        for (variable, exponent) in term.powers {
            let Variable::Slot(slot) = variable else {
                return Err(invalid());
            };
            let value = values
                .get(usize::try_from(slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            let power = value
                .pow(exponent, engine, budget)
                .map_err(|error| fault(&error))?;
            product = product
                .mul(&power, engine, budget)
                .map_err(|error| fault(&error))?;
        }
        result = result
            .add(&product, engine, budget)
            .map_err(|error| fault(&error))?;
    }
    Ok(result)
}

/// The selected fact must retain the exact qualified hypothesis that introduced its predicate.
fn endpoints(
    interpreter: &Interpretation<'_>,
    context: &Handle,
    evidence: &ScopedCoefficientRelation<Handle>,
    budget: &mut Budget,
) -> Result<[Handle; 2], Fault> {
    let premise = interpreter
        .premises
        .get(usize::try_from(evidence.premise).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    let Statement::PredicateFact { region, predicate } = &premise.judgment.statement else {
        return Err(invalid());
    };
    if (&premise.judgment.context, region) != (context, interpreter.region) {
        return Err(invalid());
    }
    let mut exact = false;
    for reference in &premise.conditions.hypotheses {
        budget.work(1, AT)?;
        if reference.context == *context
            && crate::context_check::predicate(interpreter.cas, reference)? == *predicate
        {
            exact = true;
        }
    }
    if !exact {
        return Err(invalid());
    }
    let Vertex::Condition(condition) = interpreter
        .cas
        .state
        .node(interpreter.cas.checked(predicate)?)?
    else {
        return Err(invalid());
    };
    let [left, right] = condition.arguments.as_slice() else {
        return Err(invalid());
    };
    if condition.kind != PredicateKind::Equal {
        return Err(invalid());
    }
    Ok([
        interpreter.cas.handle(*left),
        interpreter.cas.handle(*right),
    ])
}

/// Independently match each endpoint and the supplied formal difference, preserving provenance.
pub(crate) fn relation(
    interpreter: &Interpretation<'_>,
    context: &Handle,
    multiple: &RelationMultiple<Handle>,
    evidence: &ScopedCoefficientRelation<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let predicates = endpoints(interpreter, context, evidence, budget)?;
    let frame = polynomial(&interpreter.cas.state, &multiple.relation)?;
    for (handle, predicate) in [&evidence.left, &evidence.right]
        .into_iter()
        .zip(&predicates)
    {
        let endpoint = polynomial(&interpreter.cas.state, handle)?;
        budget.work(crate::graph::size(endpoint.variables.len(), AT)?, AT)?;
        if (&endpoint.domain, &endpoint.variables) != (&frame.domain, &frame.variables) {
            return Err(invalid());
        }
        let actual = interpreted(interpreter, handle, &evidence.roots, budget)?;
        let expected = interpreter
            .endpoint(predicate, budget)
            .map_err(|error| fault(&error))?;
        if !equal(interpreter, &actual, &expected, budget)? {
            return Err(invalid());
        }
    }
    difference(interpreter, multiple, evidence, budget)
}

/// The relation is the actual difference of the two displayed polynomials, checked separately.
fn difference(
    interpreter: &Interpretation<'_>,
    multiple: &RelationMultiple<Handle>,
    evidence: &ScopedCoefficientRelation<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let left = interpreter
        .endpoint(&evidence.left, budget)
        .map_err(|error| fault(&error))?;
    let right = interpreter
        .endpoint(&evidence.right, budget)
        .map_err(|error| fault(&error))?;
    let difference = left
        .add(
            &right
                .neg(&interpreter.engine, budget)
                .map_err(|error| fault(&error))?,
            &interpreter.engine,
            budget,
        )
        .map_err(|error| fault(&error))?;
    let actual = interpreter
        .endpoint(&multiple.relation, budget)
        .map_err(|error| fault(&error))?;
    if !equal(interpreter, &actual, &difference, budget)? {
        return Err(invalid());
    }
    Ok(())
}
