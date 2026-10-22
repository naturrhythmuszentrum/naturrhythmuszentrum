// SPDX-License-Identifier: AGPL-3.0-only
//! The placement action in scalar polynomial proofs, with exact domain evidence.

use super::{AT, body, invalid, region};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::LogicalId;
use crate::model::Operator;
use crate::solving::elementary::{evaluate::Evaluation, value::Value};

/// Interpret only native family syntax under its original projected support.
pub(crate) fn read(
    evaluation: &Evaluation<'_>,
    root: LogicalId,
    checked: bool,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    if let Some(value) = body(evaluation.cas, root)? {
        if region::family(evaluation.cas, evaluation.region.logical_id(), budget)?
            != value.family.logical_id()
            || !region::subset(
                evaluation.cas,
                evaluation.region.logical_id(),
                value.region.logical_id(),
                budget,
            )?
        {
            return Err(invalid());
        }
        region::projection(evaluation.cas, &value.region, evaluation.premises, budget)?;
        let mut values =
            crate::solving::coefficients::storage::vector(value.arguments.len(), budget)?;
        for argument in &value.arguments {
            budget.work(1, AT)?;
            values.push(evaluation.expression(argument.value.logical_id(), checked, budget)?);
        }
        return operation(evaluation, &value.operator, &values, checked, budget);
    }
    let expression = evaluation.cas.state.expression(root)?;
    let [id] = expression.arms.as_slice() else {
        return Err(invalid());
    };
    let arm = evaluation.cas.state.arm(*id)?;
    if !region::subset(
        evaluation.cas,
        evaluation.region.logical_id(),
        arm.region,
        budget,
    )? {
        return Err(invalid());
    }
    read(evaluation, arm.value, checked, budget)
}

/// Scalar addition and multiplication are exactly the component agreements of the registered edge.
fn operation(
    evaluation: &Evaluation<'_>,
    operator: &Operator,
    values: &[Value],
    checked: bool,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    match (operator, values) {
        (Operator::Neg, [value]) => value.neg(&evaluation.engine, budget),
        (Operator::Add, [a, b]) => a.add(b, &evaluation.engine, budget),
        (Operator::Sub, [a, b]) => a.sub(b, &evaluation.engine, budget),
        (Operator::Mul, [a, b]) => a.mul(b, &evaluation.engine, budget),
        (Operator::Div, [a, b]) => {
            if checked {
                evaluation.nonzero(b, budget)?;
            }
            a.div(b, &evaluation.engine, budget)
        }
        (Operator::IntegerPower(power), [value]) if *power > 0 => value.pow(
            u32::try_from(*power).map_err(|_| invalid())?,
            &evaluation.engine,
            budget,
        ),
        _ => Err(invalid()),
    }
}
