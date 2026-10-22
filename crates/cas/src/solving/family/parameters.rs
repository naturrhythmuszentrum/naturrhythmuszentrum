// SPDX-License-Identifier: AGPL-3.0-only
//! Exact formal base and fibre parameters with their own declared fields.

use super::invalid;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Handle, LogicalId};
use crate::solving::coefficients::{forms::Form, model::Element, proof::fault};
use crate::solving::elementary::{evaluate::Evaluation, value::Value};

/// An original coordinate is read without requiring all coordinates to have its field.
pub(crate) fn read(
    evaluation: &Evaluation<'_>,
    id: LogicalId,
    budget: &mut Budget,
) -> Result<Option<Value>, Fault> {
    let Some(numerator) = crate::solving::coefficients::guard_polynomial::read(
        &evaluation.cas.state,
        evaluation.engine.domain,
        evaluation.region.logical_id(),
        id,
        budget,
    )?
    else {
        return Ok(None);
    };
    let denominator = numerator.one(budget)?;
    let coefficient =
        evaluation
            .engine
            .element(crate::solving::coefficients::model::Value::Fraction(
                crate::solving::coefficients::fraction::Fraction {
                    numerator,
                    denominator,
                },
            ));
    let form =
        Form::constant(coefficient, &evaluation.engine, budget).map_err(|error| fault(&error))?;
    Ok(Some(Value::polynomial(form, &evaluation.engine, budget)?))
}

/// Exact regional equations constrain the mixed formal algebra only on their original piece.
pub(crate) fn zero(
    evaluation: &Evaluation<'_>,
    coefficient: &Element,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let crate::solving::coefficients::model::Value::Fraction(value) = &coefficient.value else {
        return Ok(false);
    };
    if !crate::solving::coefficients::nonzero::proves(
        &evaluation.cas.state,
        evaluation.engine.domain,
        &value.denominator,
        evaluation.region,
        evaluation
            .premises
            .iter()
            .map(|premise| &premise.judgment.statement),
        budget,
    )? {
        return Ok(false);
    }
    if evaluation
        .engine
        .zero_on(coefficient, evaluation.region, budget)
        .map_err(|error| fault(&error))?
    {
        return Ok(true);
    }
    let relations = crate::solving::coefficients::regional::zeros(
        &evaluation.cas.state,
        evaluation.engine.domain,
        &value.numerator,
        evaluation.region,
        budget,
    )?;
    let mut remainder = value.numerator.copy(budget)?;
    for relation in &relations {
        let (_, reduced) =
            crate::solving::coefficients::gcd::division(&remainder, relation, budget)?;
        remainder = reduced;
    }
    remainder.zero(budget)
}

/// Canonical fraction reuse follows checked family images and every original strict denominator.
pub(crate) fn fraction(
    evaluation: &Evaluation<'_>,
    handle: &Handle,
    map: &crate::solving::CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<Option<Value>, Fault> {
    if map.source != *evaluation.engine.domain {
        return Ok(None);
    }
    let crate::solving::CoefficientDomainData::Fraction { coordinates, .. } =
        crate::solving::coefficients::storage::domain(&evaluation.cas.state, &map.source)?
    else {
        return Err(invalid());
    };
    super::mapping::parameters(evaluation.cas, map, coordinates, budget)?;
    for image in &map.images {
        let crate::model::Operator::Free(key) = &evaluation
            .cas
            .state
            .expression(image.logical_id())?
            .operator
        else {
            return Err(invalid());
        };
        if evaluation.free.iter().any(|(name, _)| *name == key) {
            return Ok(None);
        }
    }
    let coefficient = evaluation
        .engine
        .read(handle, budget)
        .map_err(|error| fault(&error))?;
    let form =
        Form::constant(coefficient, &evaluation.engine, budget).map_err(|error| fault(&error))?;
    Ok(Some(Value::polynomial(form, &evaluation.engine, budget)?))
}
