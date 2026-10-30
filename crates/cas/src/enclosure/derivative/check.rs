// SPDX-License-Identifier: AGPL-3.0-only
//! Replay derives identities from original syntax and bounds from exact endpoint relations.

use super::{AT, DerivativeEvidence, DerivativeRule as Rule, Input, Interval, Segment, invalid};
use crate::budget::{Budget, BudgetUsage};
use crate::model::{Branch, Operator};
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Neither the producer's rule tag nor its derivative formula is trusted.
pub(in crate::enclosure) fn check(
    source: Segment<'_>,
    evidence: &DerivativeEvidence,
    proposed: &Interval,
    grid: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(), Fault> {
    if source.generalized {
        super::super::clamp::source(source, &evidence.coordinate, budget)?;
    }
    let count = source.schedule.len();
    if evidence.steps.len() != count || source.values.len() != count {
        return Err(invalid());
    }
    let mut stack = Vec::new();
    stack
        .try_reserve(source.schedule.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (index, id) in source.schedule.iter().enumerate() {
        super::super::boundary(budget, interrupt)?;
        let expression = source.cas.state.expression(*id)?;
        let right = if expression.operands.is_empty() {
            None
        } else {
            Some(stack.pop().ok_or_else(invalid)?)
        };
        let left = if expression.operands.len() == 2 {
            Some(stack.pop().ok_or_else(invalid)?)
        } else {
            None
        };
        let input = |i: usize| Input {
            value: &source.values[i],
            derivative: &evidence.steps[i].bound,
        };
        let (rule, bound) = identity(
            &expression.operator,
            source.generalized,
            &evidence.coordinate,
            (left.map(input), right.map(input)),
            &source.values[index],
            grid,
            budget,
        )?;
        if rule != evidence.steps[index].rule || bound != evidence.steps[index].bound {
            return Err(invalid());
        }
        stack.push(index);
    }
    if stack.len() != 1 || &evidence.steps.last().ok_or_else(invalid)?.bound != proposed {
        return Err(invalid());
    }
    Ok(())
}

/// Reconstruct the closed symbolic law independently of the production dispatcher.
fn identity(
    operator: &Operator,
    generalized: bool,
    coordinate: &str,
    inputs: (Option<Input<'_>>, Option<Input<'_>>),
    value: &Interval,
    grid: u32,
    budget: &mut Budget,
) -> Result<(Rule, Interval), Fault> {
    match operator {
        Operator::ClampFloor => {
            if !generalized {
                return Err(invalid());
            }
            super::super::clamp::check_slope(
                inputs.0.ok_or_else(invalid)?.value,
                inputs.1.ok_or_else(invalid)?.value,
                budget,
            )
        }
        Operator::Rational(_) => Ok((Rule::Constant, constant(0, budget)?)),
        Operator::Free(name) => Ok(if name == coordinate {
            (Rule::Coordinate, constant(1, budget)?)
        } else {
            (Rule::Parameter, constant(0, budget)?)
        }),
        Operator::Neg => {
            let input = inputs.1.ok_or_else(invalid)?.derivative;
            Ok((
                Rule::Negate,
                rounded(
                    &input.upper().negated(budget)?,
                    &input.lower().negated(budget)?,
                    grid,
                    budget,
                )?,
            ))
        }
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => binary(
            operator,
            inputs.0.ok_or_else(invalid)?,
            inputs.1.ok_or_else(invalid)?,
            grid,
            budget,
        ),
        Operator::PositivePower | Operator::GeneralPower(Branch::StrictReal) => {
            let base = inputs.0.ok_or_else(invalid)?;
            if base.value.lower().compare(&Rational::zero(), budget)? != Ordering::Greater {
                return Err(invalid());
            }
            let exponent = inputs.1.ok_or_else(invalid)?.value;
            if exponent.lower() != exponent.upper() {
                return Err(invalid());
            }
            if matches!(operator, Operator::GeneralPower(_)) && !exponent.lower().is_half() {
                return Err(invalid());
            }
            let scaled = operation(&Operator::Mul, value, exponent, grid, budget)?;
            let factor = operation(&Operator::Div, &scaled, base.value, grid, budget)?;
            Ok((
                if matches!(operator, Operator::PositivePower) {
                    Rule::Power
                } else {
                    Rule::Root
                },
                operation(&Operator::Mul, &factor, base.derivative, grid, budget)?,
            ))
        }
        _ => analytic(operator, inputs.1.ok_or_else(invalid)?, value, grid, budget),
    }
}

/// Ordered endpoint arithmetic checks both terms of every product or quotient identity.
fn binary(
    operator: &Operator,
    left: Input<'_>,
    right: Input<'_>,
    grid: u32,
    budget: &mut Budget,
) -> Result<(Rule, Interval), Fault> {
    if matches!(operator, Operator::Add | Operator::Sub) {
        return Ok((
            if matches!(operator, Operator::Add) {
                Rule::Sum
            } else {
                Rule::Difference
            },
            operation(operator, left.derivative, right.derivative, grid, budget)?,
        ));
    }
    let first = operation(&Operator::Mul, left.derivative, right.value, grid, budget)?;
    let second = operation(&Operator::Mul, left.value, right.derivative, grid, budget)?;
    match operator {
        Operator::Mul => Ok((
            Rule::Product,
            operation(&Operator::Add, &first, &second, grid, budget)?,
        )),
        Operator::Div => {
            let numerator = operation(&Operator::Sub, &first, &second, grid, budget)?;
            let denominator = operation(&Operator::Mul, right.value, right.value, grid, budget)?;
            Ok((
                Rule::Quotient,
                operation(&Operator::Div, &numerator, &denominator, grid, budget)?,
            ))
        }
        _ => Err(invalid()),
    }
}

/// Registered analytic identities have full-box value and domain premises.
fn analytic(
    operator: &Operator,
    input: Input<'_>,
    value: &Interval,
    grid: u32,
    budget: &mut Budget,
) -> Result<(Rule, Interval), Fault> {
    let one = constant(1, budget)?;
    let (rule, factor) = match operator {
        Operator::Exponential => (
            Rule::Exponential,
            Interval::new(
                value.lower().add(&Rational::zero(), budget)?,
                value.upper().add(&Rational::zero(), budget)?,
                budget,
            )?,
        ),
        Operator::NaturalLog => {
            if input.value.lower().compare(&Rational::zero(), budget)? != Ordering::Greater {
                return Err(invalid());
            }
            (
                Rule::Logarithm,
                rounded(
                    &Rational::one().div(input.value.upper(), budget)?,
                    &Rational::one().div(input.value.lower(), budget)?,
                    grid,
                    budget,
                )?,
            )
        }
        Operator::Sigmoid => {
            let complement = operation(&Operator::Sub, &one, value, grid, budget)?;
            (
                Rule::Sigmoid,
                operation(&Operator::Mul, value, &complement, grid, budget)?,
            )
        }
        Operator::Tanh => {
            let square = operation(&Operator::Mul, value, value, grid, budget)?;
            (
                Rule::Tanh,
                operation(&Operator::Sub, &one, &square, grid, budget)?,
            )
        }
        _ => return Err(invalid()),
    };
    Ok((
        rule,
        operation(&Operator::Mul, &factor, input.derivative, grid, budget)?,
    ))
}

/// Exact scalar endpoint replay, followed by the independently checked outward grid.
fn operation(
    operator: &Operator,
    a: &Interval,
    b: &Interval,
    grid: u32,
    budget: &mut Budget,
) -> Result<Interval, Fault> {
    let (lo, hi) = super::super::checker::relation(operator, a, b, grid, budget)?;
    rounded(&lo, &hi, grid, budget)
}

/// Reconstruct signed floor and ceiling without production interval arithmetic.
fn rounded(
    lo: &Rational,
    hi: &Rational,
    grid: u32,
    budget: &mut Budget,
) -> Result<Interval, Fault> {
    let (lo, hi) = super::super::checker::grid(lo, hi, grid, budget)?;
    Interval::new(lo, hi, budget)
}

/// Exact derivative identities introduce only rational singleton constants.
fn constant(n: i64, budget: &mut Budget) -> Result<Interval, Fault> {
    Interval::singleton(&Rational::from_ratio(n, 1, budget)?, budget)
}
