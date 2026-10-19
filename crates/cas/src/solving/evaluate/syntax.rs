// SPDX-License-Identifier: AGPL-3.0-only
//! Same-sort scalar leaves and explicit local family arithmetic.

use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Handle, Vertex};
use crate::model::Operator;
use crate::solving::SolveDraft;
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::elementary::value::Value;

/// Ordinary values retain their own declarations; family arithmetic retains its explicit local body.
pub(super) fn read(
    evaluation: &Evaluation<'_>,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    let mut pending = vector(1, budget)?;
    pending.push((handle.logical_id(), false, 1_u64));
    let mut values = vector(1, budget)?;
    while let Some((id, expanded, depth)) = pending.pop() {
        budget.work(1, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        match evaluation.cas.state.node(id)? {
            Vertex::Expression(_) => {
                reserve(&mut values, 1, budget)?;
                values.push(evaluation.expression(id, true, budget)?);
            }
            Vertex::Solve(body) => {
                let SolveDraft::FamilyExpression(body) = body.as_ref() else {
                    return Err(invalid());
                };
                if body.region != *evaluation.region {
                    return Err(invalid());
                }
                if expanded {
                    let start = values
                        .len()
                        .checked_sub(body.arguments.len())
                        .ok_or_else(invalid)?;
                    let result = operation(
                        evaluation,
                        &body.operator,
                        values.get(start..).ok_or_else(invalid)?,
                        budget,
                    )?;
                    values.truncate(start);
                    reserve(&mut values, 1, budget)?;
                    values.push(result);
                } else {
                    reserve(&mut pending, body.arguments.len().saturating_add(1), budget)?;
                    pending.push((id, true, depth));
                    pending.extend(body.arguments.iter().rev().map(|argument| {
                        (argument.value.logical_id(), false, depth.saturating_add(1))
                    }));
                }
            }
            _ => return Err(invalid()),
        }
    }
    if values.len() != 1 {
        return Err(invalid());
    }
    values.pop().ok_or_else(invalid)
}

/// Each local operation retains strict denominator obligations before cancellation.
fn operation(
    evaluation: &Evaluation<'_>,
    operator: &Operator,
    values: &[Value],
    budget: &mut Budget,
) -> Result<Value, Fault> {
    let engine = &evaluation.engine;
    match (operator, values) {
        (Operator::Neg, [value]) => value.neg(engine, budget),
        (Operator::Add, [left, right]) => left.add(right, engine, budget),
        (Operator::Sub, [left, right]) => left.sub(right, engine, budget),
        (Operator::Mul, [left, right]) => left.mul(right, engine, budget),
        (Operator::Div, [left, right]) => {
            evaluation.nonzero(right, budget)?;
            left.div(right, engine, budget)
        }
        (Operator::IntegerPower(exponent), [value]) if *exponent > 0 => value.pow(
            u32::try_from(*exponent).map_err(|_| invalid())?,
            engine,
            budget,
        ),
        _ => Err(Fault::invalid(
            InvalidCode::UnsupportedFragment,
            crate::error::Location::new("finite family coefficient operator"),
        )),
    }
}
