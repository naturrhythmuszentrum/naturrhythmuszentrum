// SPDX-License-Identifier: AGPL-3.0-only
//! Closed clamp joins preserve values without granting a classical derivative.

use super::{AT, DerivativeRule, Interval};
use crate::budget::{Budget, BudgetKind};
use crate::graph::{Expression, LogicalId, State};
use crate::model::Operator;
use crate::{Fault, InvalidCode, Rational};
use std::cmp::Ordering;

/// A floor is an original free coordinate and a fixed rational or independent parameter.
pub(super) fn shape(state: &State, operands: &[LogicalId]) -> Result<bool, Fault> {
    if operands.len() != 2 {
        return Ok(false);
    }
    let Operator::Free(x) = &state.expression(operands[0])?.operator else {
        return Ok(false);
    };
    Ok(match &state.expression(operands[1])?.operator {
        Operator::Rational(_) => true,
        Operator::Free(wall) => wall != x,
        _ => false,
    })
}

/// Replay the maximum by exact endpoint selection, independently of interval production.
pub(super) fn check_value(
    state: &State,
    expression: &Expression,
    x: &Interval,
    wall: &Interval,
    grid: u32,
    budget: &mut Budget,
) -> Result<(Rational, Rational), Fault> {
    if !shape(state, &expression.operands)? {
        return Err(invalid());
    }
    let lower = if x.lower().compare(wall.lower(), budget)? == Ordering::Greater {
        x.lower()
    } else {
        wall.lower()
    };
    let upper = if x.upper().compare(wall.upper(), budget)? == Ordering::Greater {
        x.upper()
    } else {
        wall.upper()
    };
    super::checker::grid(lower, upper, grid, budget)
}

/// Authenticate one original g(max(x,a)) composition; no callback supplies regularity.
pub(super) fn source(
    source: super::derivative::Segment<'_>,
    coordinate: &str,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = crate::graph::size(source.schedule.len(), AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, count.saturating_mul(32), AT)?;
    let mut pending = Vec::new();
    pending
        .try_reserve(source.schedule.len())
        .map_err(|_| Fault::allocation(AT))?;
    pending.push(*source.schedule.last().ok_or_else(invalid)?);
    let mut floor = None;
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let expression = source.cas.state.expression(id)?;
        match &expression.operator {
            Operator::ClampFloor => {
                if !shape(&source.cas.state, &expression.operands)?
                    || !matches!(&source.cas.state.expression(expression.operands[0])?.operator, Operator::Free(name) if name == coordinate)
                {
                    return Err(invalid());
                }
                if floor.is_some_and(|previous| previous != id) {
                    return Err(invalid());
                }
                floor = Some(id);
            }
            Operator::Free(name) if name == coordinate => return Err(invalid()),
            Operator::Derivative(_) | Operator::GeneralizedDerivative(_) => return Err(invalid()),
            _ => pending.extend(expression.operands.iter().copied()),
        }
    }
    if floor.is_none() {
        return Err(invalid());
    }
    Ok(())
}

/// Produce a floor slope hull with the complete uncertain-wall region included.
pub(super) fn slope(
    x: &Interval,
    wall: &Interval,
    budget: &mut Budget,
) -> Result<(DerivativeRule, Interval), Fault> {
    if x.upper().compare(wall.lower(), budget)? == Ordering::Less {
        Ok((
            DerivativeRule::ClampConstant,
            Interval::singleton(&Rational::zero(), budget)?,
        ))
    } else if x.lower().compare(wall.upper(), budget)? == Ordering::Greater {
        Ok((
            DerivativeRule::ClampResponsive,
            Interval::singleton(&Rational::one(), budget)?,
        ))
    } else {
        Ok((
            DerivativeRule::ClampJoin,
            Interval::new(Rational::zero(), Rational::one(), budget)?,
        ))
    }
}

/// Reconstruct branch coverage independently; equality at a wall retains both slopes.
pub(super) fn check_slope(
    x: &Interval,
    wall: &Interval,
    budget: &mut Budget,
) -> Result<(DerivativeRule, Interval), Fault> {
    let constant = wall.lower().compare(x.upper(), budget)? == Ordering::Greater;
    let responsive = wall.upper().compare(x.lower(), budget)? == Ordering::Less;
    let lower = Rational::from_ratio(i64::from(responsive), 1, budget)?;
    let upper = Rational::from_ratio(i64::from(!constant), 1, budget)?;
    let identity = if constant {
        DerivativeRule::ClampConstant
    } else if responsive {
        DerivativeRule::ClampResponsive
    } else {
        DerivativeRule::ClampJoin
    };
    Ok((identity, Interval::new(lower, upper, budget)?))
}

/// A malformed clamp cannot grant a regularity or source premise.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
