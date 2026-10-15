// SPDX-License-Identifier: AGPL-3.0-only
//! Exact regional nonzero evidence for canonical parameter polynomials.

use super::polynomial::Polynomial;
use super::scalar::{Ground, Scalar};
use super::storage::{AT, domain, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, Handle, State, Vertex};
use crate::model::{Field, Operator, PredicateKind};
use crate::proof_model::Statement;
use crate::solving::{
    CoefficientDomainData, CoefficientValueData, MapMode, MapTarget, SolveDraft, SolveStatement,
};

/// Only the canonical parameter interpretation is shared by regional fraction unit statements.
fn canonical_map(
    state: &State,
    source: &Handle,
    map: crate::graph::LogicalId,
    region: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let SolveDraft::CoefficientMap(mapping) = crate::solving::graph::object(state, map)? else {
        return Err(invalid());
    };
    if mapping.source != *source
        || mapping.region != *region
        || mapping.mode != MapMode::Specialization
        || !mapping.denominators.is_empty()
    {
        return Ok(false);
    }
    let CoefficientDomainData::Fraction { base, coordinates } = domain(state, source)? else {
        return Ok(false);
    };
    let SolveDraft::CoordinateSpace(product) =
        crate::solving::graph::object(state, coordinates.logical_id())?
    else {
        return Err(invalid());
    };
    let MapTarget::ScalarAmbient { header, field } = &mapping.target else {
        return Ok(false);
    };
    if !matches!(field, Field::Rational | Field::Real | Field::Complex) {
        return Ok(false);
    }
    if *header != product.header || mapping.images.len() != product.coordinates.len() {
        return Ok(false);
    }
    if super::read::ground(state, base)? == Ground::Gaussian && *field != Field::Complex {
        return Ok(false);
    }
    for (image, coordinate) in mapping.images.iter().zip(&product.coordinates) {
        budget.work(crate::graph::size(coordinate.key.len(), AT)?, AT)?;
        let expression = state.expression(image.logical_id())?;
        if expression.header != product.header.logical_id()
            || !matches!(&expression.operator, Operator::Free(key) if key == &coordinate.key)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A nonzero defined fraction has both nonzero numerator and nonzero denominator.
fn fraction_contains(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    coefficient: crate::graph::LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let SolveDraft::CoefficientValue(stored) = crate::solving::graph::object(state, coefficient)?
    else {
        return Err(invalid());
    };
    if stored.domain != *source {
        return Ok(false);
    }
    let CoefficientValueData::Fraction {
        numerator,
        denominator,
    } = &stored.value
    else {
        return Ok(false);
    };
    for handle in [numerator, denominator] {
        let candidate = super::read::sparse(state, handle, budget)?;
        if polynomial.equals(&candidate, budget)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Recognize a single independent parameter in the source's exact coordinate order.
fn coordinate(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    key: &str,
    header: crate::graph::LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let CoefficientDomainData::Fraction { base, coordinates } = domain(state, source)? else {
        return Ok(false);
    };
    let SolveDraft::CoordinateSpace(product) =
        crate::solving::graph::object(state, coordinates.logical_id())?
    else {
        return Err(invalid());
    };
    if product.header.logical_id() != header {
        return Ok(false);
    }
    if polynomial.ground != super::read::ground(state, base)?
        || polynomial.variables != product.coordinates.len()
    {
        return Ok(false);
    }
    let [term] = polynomial.terms.as_slice() else {
        return Ok(false);
    };
    let mut index = None;
    for (position, exponent) in term.powers.iter().enumerate() {
        budget.work(1, AT)?;
        if *exponent == 1 && index.is_none() {
            index = Some(position);
        } else if *exponent != 0 {
            return Ok(false);
        }
    }
    let Some(index) = index else {
        return Ok(false);
    };
    let Some(entry) = product.coordinates.get(index) else {
        return Ok(false);
    };
    let scalar = &term.coefficient;
    Ok(entry.key == key
        && scalar.equals(&Scalar::constant(scalar.ground(), true, budget)?, budget)?)
}

/// Every accepted premise is tied to the same domain, ordered parameters and coefficient region.
fn at_source(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    region: &Handle,
    statement: &Statement,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    budget.work(1, AT)?;
    match statement {
        Statement::Solve(statement) => {
            let SolveStatement::Unit(unit) = statement.as_ref() else {
                return Ok(false);
            };
            Ok(unit.domain == *source
                && unit.region == *region
                && fraction_contains(state, source, polynomial, unit.value.logical_id(), budget)?)
        }
        Statement::PredicateFact {
            region: actual,
            predicate,
        } if actual == region => {
            let Vertex::Condition(condition) = state.node(predicate.logical_id())? else {
                return Err(invalid());
            };
            if !matches!(
                condition.kind,
                PredicateKind::NonZero | PredicateKind::Positive
            ) {
                return Ok(false);
            }
            let [argument] = condition.arguments.as_slice() else {
                return Ok(false);
            };
            let expression = state.expression(*argument)?;
            match (&expression.operator, &expression.attachment) {
                (Operator::Free(key), None) => {
                    coordinate(state, source, polynomial, key, expression.header, budget)
                }
                (
                    Operator::ExactCoefficient,
                    Some(ExpressionAttachment::ExactCoefficient { value, map }),
                ) => Ok(canonical_map(state, source, *map, region, budget)?
                    && fraction_contains(state, source, polynomial, *value, budget)?),
                _ => Ok(false),
            }
        }
        _ => Ok(false),
    }
}

/// Graph-owned polynomials use the same independently metered comparison as scratch values.
pub(crate) fn proves(
    state: &State,
    source: &Handle,
    polynomial: &Handle,
    region: &Handle,
    statement: &Statement,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let value = super::read::sparse(state, polynomial, budget)?;
    proves_polynomial(state, source, &value, region, statement, budget)
}

/// Localization polynomials retain their parameter meaning through canonical tower-parent inclusions.
pub(crate) fn proves_polynomial(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    region: &Handle,
    statement: &Statement,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut current = source;
    loop {
        budget.work(1, AT)?;
        if at_source(state, current, polynomial, region, statement, budget)? {
            return Ok(true);
        }
        let CoefficientDomainData::Quotient { level } = domain(state, current)? else {
            return Ok(false);
        };
        let SolveDraft::ExtensionLevel(extension) =
            crate::solving::graph::object(state, level.logical_id())?
        else {
            return Err(invalid());
        };
        current = &extension.parent;
    }
}
