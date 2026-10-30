// SPDX-License-Identifier: AGPL-3.0-only
//! Producer proposals carry closed identities separately from their interval calculations.

use super::{
    AT, DerivativeEvidence, DerivativeRule as Rule, DerivativeStep, Input, Interval, Segment,
    invalid,
};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::model::{Branch, Operator};
use crate::{Fault, Rational};

/// Follow the original subtree without callbacks or midpoint samples.
pub(in crate::enclosure) fn produce(
    source: Segment<'_>,
    coordinate: &str,
    position: (u32, u32, u32),
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(Interval, DerivativeEvidence), Fault> {
    if source.generalized {
        super::super::clamp::source(source, coordinate, budget)?;
    }
    let (record, first, grid) = position;
    let count = crate::graph::size(source.schedule.len(), AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        count.saturating_mul(128),
        AT,
    )?;
    let mut size = crate::graph::size(coordinate.len(), AT)?.saturating_add(64);
    budget.work(size, AT)?;
    let mut evidence = DerivativeEvidence {
        record,
        first,
        coordinate: coordinate.into(),
        steps: Vec::new(),
    };
    let mut stack = Vec::new();
    evidence
        .steps
        .try_reserve(source.schedule.len())
        .map_err(|_| Fault::allocation(AT))?;
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
        let (rule, bound) = node(
            &expression.operator,
            source.generalized,
            coordinate,
            (left.map(input), right.map(input)),
            &source.values[index],
            grid,
            budget,
        )?;
        size = size
            .saturating_add(super::super::interval_bytes(&bound))
            .saturating_add(16);
        budget.check(BudgetKind::StagedPayloadBytes, size, AT)?;
        budget.work(super::super::interval_bytes(&bound).saturating_add(16), AT)?;
        evidence.steps.push(DerivativeStep { rule, bound });
        stack.push(index);
    }
    if stack.len() != 1 {
        return Err(invalid());
    }
    let result = evidence
        .steps
        .last()
        .ok_or_else(invalid)?
        .bound
        .copied(budget)?;
    Ok((result, evidence))
}

/// Each identity is fixed by the source operator and the original coordinate.
fn node(
    operator: &Operator,
    generalized: bool,
    coordinate: &str,
    inputs: (Option<Input<'_>>, Option<Input<'_>>),
    value: &Interval,
    grid: u32,
    budget: &mut Budget,
) -> Result<(Rule, Interval), Fault> {
    let zero = || Rational::zero();
    let result = match operator {
        Operator::ClampFloor => {
            if !generalized {
                return Err(invalid());
            }
            super::super::clamp::slope(
                inputs.0.ok_or_else(invalid)?.value,
                inputs.1.ok_or_else(invalid)?.value,
                budget,
            )?
        }
        Operator::Rational(_) => (Rule::Constant, Interval::singleton(&zero(), budget)?),
        Operator::Free(name) if name == coordinate => (
            Rule::Coordinate,
            Interval::singleton(&Rational::one(), budget)?,
        ),
        Operator::Free(_) => (Rule::Parameter, Interval::singleton(&zero(), budget)?),
        Operator::Neg => {
            let d = inputs.1.ok_or_else(invalid)?.derivative;
            (
                Rule::Negate,
                Interval::new(
                    d.upper().negated(budget)?,
                    d.lower().negated(budget)?,
                    budget,
                )?
                .coarsen(grid, budget)?,
            )
        }
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => binary(
            operator,
            inputs.0.ok_or_else(invalid)?,
            inputs.1.ok_or_else(invalid)?,
            grid,
            budget,
        )?,
        Operator::GeneralPower(Branch::StrictReal) | Operator::PositivePower => {
            let base = inputs.0.ok_or_else(invalid)?;
            let exponent = inputs.1.ok_or_else(invalid)?.value;
            positive(base.value, budget)?;
            let bound = value
                .mul(exponent, grid, budget)?
                .mul(&base.value.reciprocal(grid, budget)?, grid, budget)?
                .mul(base.derivative, grid, budget)?;
            (
                if matches!(operator, Operator::PositivePower) {
                    Rule::Power
                } else {
                    Rule::Root
                },
                bound,
            )
        }
        _ => analytic(operator, inputs.1.ok_or_else(invalid)?, value, grid, budget)?,
    };
    Ok(result)
}

/// Product and quotient rules retain operand order and every denominator premise.
fn binary(
    operator: &Operator,
    left: Input<'_>,
    right: Input<'_>,
    grid: u32,
    budget: &mut Budget,
) -> Result<(Rule, Interval), Fault> {
    Ok(match operator {
        Operator::Add => (
            Rule::Sum,
            left.derivative.add(right.derivative, grid, budget)?,
        ),
        Operator::Sub => (
            Rule::Difference,
            left.derivative.sub(right.derivative, grid, budget)?,
        ),
        Operator::Mul => (
            Rule::Product,
            left.derivative.mul(right.value, grid, budget)?.add(
                &left.value.mul(right.derivative, grid, budget)?,
                grid,
                budget,
            )?,
        ),
        Operator::Div => {
            let numerator = left.derivative.mul(right.value, grid, budget)?.sub(
                &left.value.mul(right.derivative, grid, budget)?,
                grid,
                budget,
            )?;
            let denominator = right.value.mul(right.value, grid, budget)?;
            (
                Rule::Quotient,
                numerator.mul(&denominator.reciprocal(grid, budget)?, grid, budget)?,
            )
        }
        _ => return Err(invalid()),
    })
}

/// Closed analytic identities use checked original values over the entire box.
fn analytic(
    operator: &Operator,
    input: Input<'_>,
    value: &Interval,
    grid: u32,
    budget: &mut Budget,
) -> Result<(Rule, Interval), Fault> {
    let one = Interval::singleton(&Rational::one(), budget)?;
    let (rule, factor) = match operator {
        Operator::Exponential => (Rule::Exponential, value.copied(budget)?),
        Operator::NaturalLog => {
            positive(input.value, budget)?;
            (Rule::Logarithm, input.value.reciprocal(grid, budget)?)
        }
        Operator::Sigmoid => (
            Rule::Sigmoid,
            value.mul(&one.sub(value, grid, budget)?, grid, budget)?,
        ),
        Operator::Tanh => (
            Rule::Tanh,
            one.sub(&value.mul(value, grid, budget)?, grid, budget)?,
        ),
        _ => return Err(invalid()),
    };
    Ok((rule, factor.mul(input.derivative, grid, budget)?))
}

/// A root value at zero does not establish a bounded smooth derivative there.
fn positive(value: &Interval, budget: &mut Budget) -> Result<(), Fault> {
    if value.lower().compare(&Rational::zero(), budget)? != std::cmp::Ordering::Greater {
        return Err(invalid());
    }
    Ok(())
}
