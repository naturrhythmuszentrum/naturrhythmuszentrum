// SPDX-License-Identifier: AGPL-3.0-only
//! Outward Taylor proposals, with no authority until independently replayed.

use super::super::log::Plan;
use super::{AT, ExpEndpoint, ExpEvidence, ExpSeries, Interval};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::{Fault, InvalidCode, Rational};
use std::cmp::Ordering;

/// Evaluate both endpoints monotonically, retaining finite recurrence proposals.
pub(in crate::enclosure) fn produce(
    input: &Interval,
    record: u32,
    plan: Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(Interval, ExpEvidence), Fault> {
    let (lower, lo) = endpoint(input.lower(), plan, budget, interrupt)?;
    let (upper, hi) = if input.lower().compare(input.upper(), budget)? == Ordering::Equal {
        budget.work(
            super::endpoint_bytes(&lower).saturating_add(super::super::interval_bytes(&lo)),
            AT,
        )?;
        (lower.clone(), lo.clone())
    } else {
        endpoint(input.upper(), plan, budget, interrupt)?
    };
    let result = Interval::new(lo.lower().clone(), hi.upper().clone(), budget)?;
    Ok((
        result,
        ExpEvidence {
            record,
            lower,
            upper,
        },
    ))
}

/// Select the least binary reduction and reconstruct by exactly that many squarings.
fn endpoint(
    x: &Rational,
    plan: Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(ExpEndpoint, Interval), Fault> {
    let mut argument = if x.is_negative() {
        x.negated(budget)?
    } else {
        x.add(&Rational::zero(), budget)?
    };
    let half = Rational::from_ratio(1, 2, budget)?;
    let mut squarings = 0;
    while argument.compare(&half, budget)? == Ordering::Greater {
        super::super::boundary(budget, interrupt)?;
        if squarings == plan.reduction {
            return Err(Fault::invalid(InvalidCode::Policy, AT));
        }
        argument = argument.mul(&half, budget)?;
        squarings += 1;
    }
    let series = series(&argument, plan, budget, interrupt)?;
    let mut result = Interval::new(
        series.sum.lower().clone(),
        series.sum.upper().add(series.tail.upper(), budget)?,
        budget,
    )?
    .coarsen(plan.grid, budget)?;
    for _ in 0..squarings {
        super::super::boundary(budget, interrupt)?;
        result = result.mul(&result, plan.grid, budget)?;
    }
    if x.is_negative() {
        result = result.reciprocal(plan.grid, budget)?;
    }
    Ok((
        ExpEndpoint {
            squarings,
            argument,
            series,
        },
        result,
    ))
}

/// Include degrees zero through N and enclose the first omitted term separately.
fn series(
    a: &Rational,
    plan: Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<ExpSeries, Fault> {
    let one = Interval::singleton(&Rational::one(), budget)?;
    if a.is_zero() {
        let zero = Interval::singleton(&Rational::zero(), budget)?;
        return Ok(ExpSeries {
            degree: 0,
            sum: one,
            omitted: zero.clone(),
            tail: zero,
        });
    }
    budget.check(BudgetKind::ProofNodes, u64::from(plan.terms) + 1, AT)?;
    let argument = Interval::singleton(a, budget)?;
    let mut term = one.clone();
    let mut sum = one.clone();
    for j in 1..=plan.terms {
        super::super::boundary(budget, interrupt)?;
        let inverse = Interval::singleton(&Rational::from_ratio(1, u64::from(j), budget)?, budget)?;
        term = term
            .mul(&argument, plan.grid, budget)?
            .mul(&inverse, plan.grid, budget)?;
        sum = sum.add(&term, plan.grid, budget)?;
    }
    let inverse = Interval::singleton(
        &Rational::from_ratio(1, u64::from(plan.terms) + 1, budget)?,
        budget,
    )?;
    let omitted = term
        .mul(&argument, plan.grid, budget)?
        .mul(&inverse, plan.grid, budget)?;
    let divisor = Interval::singleton(
        &Rational::from_ratio(1, u64::from(plan.terms) + 2, budget)?,
        budget,
    )?;
    let denominator = one.sub(
        &argument.mul(&divisor, plan.grid, budget)?,
        plan.grid,
        budget,
    )?;
    let tail = omitted.mul(
        &denominator.reciprocal(plan.grid, budget)?,
        plan.grid,
        budget,
    )?;
    Ok(ExpSeries {
        degree: plan.terms,
        sum,
        omitted,
        tail,
    })
}
