// SPDX-License-Identifier: AGPL-3.0-only
//! The registered edge acts on components while every input keeps its own scalar sort.

use super::{AT, body, invalid, region};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::LogicalId;
use crate::model::Operator;
use crate::solving::elementary::complex::{Components, parts::Parts};
use crate::solving::elementary::value::Value;

/// Interpret only a checked family body or its native case associations.
pub(crate) fn read(
    components: &mut Components<'_, '_>,
    root: LogicalId,
    bound: &[Parts],
    budget: &mut Budget,
) -> Result<Parts, Fault> {
    let evaluation = components.evaluation;
    if let Some(value) = body(evaluation.cas, root)? {
        if region::family(evaluation.cas, evaluation.region.logical_id(), budget)?
            != value.family.logical_id()
        {
            return Err(invalid());
        }
        if !region::subset(
            evaluation.cas,
            evaluation.region.logical_id(),
            value.region.logical_id(),
            budget,
        )? {
            return Err(invalid());
        }
        region::projection(evaluation.cas, &value.region, evaluation.premises, budget)?;
        let mut values =
            crate::solving::coefficients::storage::vector(value.arguments.len(), budget)?;
        for argument in &value.arguments {
            budget.work(1, AT)?;
            values.push(components.expression(argument.value.logical_id(), bound, budget)?);
        }
        return operation(components, &value.operator, &values, budget);
    }
    cases(components, root, bound, budget)
}

/// Every reached association must agree; an undecided overlap never silently selects an arm.
fn cases(
    components: &mut Components<'_, '_>,
    root: LogicalId,
    bound: &[Parts],
    budget: &mut Budget,
) -> Result<Parts, Fault> {
    let evaluation = components.evaluation;
    let expression = evaluation.cas.state.expression(root)?;
    let mut result: Option<Parts> = None;
    for id in &expression.arms {
        budget.work(1, AT)?;
        let arm = evaluation.cas.state.arm(*id)?;
        if body(evaluation.cas, arm.value)?.is_none()
            || !region::subset(
                evaluation.cas,
                evaluation.region.logical_id(),
                arm.region,
                budget,
            )?
        {
            return Err(invalid());
        }
        let value = components.expression(arm.value, bound, budget)?;
        if let Some(previous) = &result {
            let difference = previous.sum(&value, true, evaluation, budget)?;
            components.zero(&difference.real, budget)?;
            components.zero(&difference.imaginary, budget)?;
        } else {
            result = Some(value);
        }
    }
    result.ok_or_else(invalid)
}

/// These are precisely the zero, additive, multiplicative, reciprocal and component agreements.
fn operation(
    components: &Components<'_, '_>,
    operator: &Operator,
    values: &[Parts],
    budget: &mut Budget,
) -> Result<Parts, Fault> {
    let evaluation = components.evaluation;
    match (operator, values) {
        (Operator::Add, [a, b]) => a.sum(b, false, evaluation, budget),
        (Operator::Sub, [a, b]) => a.sum(b, true, evaluation, budget),
        (Operator::Mul, [a, b]) => a.mul(b, evaluation, budget),
        (Operator::Div, [a, b]) => components.divide(a, b, budget),
        (Operator::Neg, [value]) => Ok(Parts {
            real: value.real.neg(&evaluation.engine, budget)?,
            imaginary: value.imaginary.neg(&evaluation.engine, budget)?,
        }),
        (Operator::Conjugate, [value]) => value.conjugate(evaluation, budget),
        (Operator::RealPart, [value]) => Ok(Parts {
            real: value.real.copy(&evaluation.engine, budget)?,
            imaginary: Value::integer(0, &evaluation.engine, budget)?,
        }),
        (Operator::ImagPart, [value]) => Ok(Parts {
            real: value.imaginary.copy(&evaluation.engine, budget)?,
            imaginary: Value::integer(0, &evaluation.engine, budget)?,
        }),
        (Operator::IntegerPower(power), [value]) if *power > 0 => {
            value.power(*power, evaluation, budget)
        }
        _ => Err(invalid()),
    }
}
