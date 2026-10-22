// SPDX-License-Identifier: AGPL-3.0-only
//! Exact regional nonzero evidence for canonical parameter polynomials.

use super::scalar::Ground;
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
pub(crate) fn canonical_map(
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
    if matches!(mapping.target, MapTarget::Family { .. }) {
        crate::solving::family::mapping::parameters_state(state, mapping, coordinates, budget)?;
        return Ok(true);
    }
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

/// A proved nonzero defined fraction supplies both of its exact polynomial factors.
fn fraction_factors(
    state: &State,
    source: &Handle,
    coefficient: crate::graph::LogicalId,
    result: &mut super::nonzero::Factors,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let SolveDraft::CoefficientValue(stored) = crate::solving::graph::object(state, coefficient)?
    else {
        return Err(invalid());
    };
    if stored.domain != *source {
        return Ok(());
    }
    if let CoefficientValueData::Fraction {
        numerator,
        denominator,
    } = &stored.value
    {
        for handle in [numerator, denominator] {
            result.insert(super::read::sparse(state, handle, budget)?, budget)?;
        }
    }
    Ok(())
}

/// A predicate premise retains its exact region, semantic header and binder frame.
fn predicate_factors(
    state: &State,
    source: &Handle,
    region: &Handle,
    predicate: &Handle,
    result: &mut super::nonzero::Factors,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Vertex::Condition(condition) = state.node(predicate.logical_id())? else {
        return Err(invalid());
    };
    let frame = state.region(region.logical_id())?;
    if (condition.header, &condition.scope) != (frame.header, &frame.scope)
        || !matches!(
            condition.kind,
            PredicateKind::NonZero | PredicateKind::Positive
        )
    {
        return Ok(());
    }
    let [argument] = condition.arguments.as_slice() else {
        return Ok(());
    };
    let expression = state.expression(*argument)?;
    if let (Operator::ExactCoefficient, Some(ExpressionAttachment::ExactCoefficient { value, map })) =
        (&expression.operator, &expression.attachment)
        && canonical_map(state, source, *map, region, budget)?
    {
        return fraction_factors(state, source, *value, result, budget);
    }
    if let Some(value) =
        super::guard_polynomial::read(state, source, region.logical_id(), *argument, budget)?
    {
        result.insert(value, budget)?;
    }
    Ok(())
}

/// Every accepted premise names the identical source domain and coefficient region.
fn at_source(
    state: &State,
    source: &Handle,
    region: &Handle,
    statement: &Statement,
    result: &mut super::nonzero::Factors,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    match statement {
        Statement::Solve(statement) => {
            if let SolveStatement::Unit(unit) = statement.as_ref()
                && (&unit.domain, &unit.region) == (source, region)
            {
                fraction_factors(state, source, unit.value.logical_id(), result, budget)?;
            }
        }
        Statement::PredicateFact {
            region: actual,
            predicate,
        } if actual == region => {
            predicate_factors(state, source, region, predicate, result, budget)?;
        }
        _ => {}
    }
    Ok(())
}

/// Parameter meaning survives only canonical tower-parent inclusions.
pub(crate) fn factors(
    state: &State,
    source: &Handle,
    region: &Handle,
    statement: &Statement,
    result: &mut super::nonzero::Factors,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut current = source;
    loop {
        at_source(state, current, region, statement, result, budget)?;
        let CoefficientDomainData::Quotient { level } = domain(state, current)? else {
            return Ok(());
        };
        let SolveDraft::ExtensionLevel(extension) =
            crate::solving::graph::object(state, level.logical_id())?
        else {
            return Err(invalid());
        };
        current = &extension.parent;
    }
}
