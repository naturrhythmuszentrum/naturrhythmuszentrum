// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded proposals; no result from this module is checked evidence.

use super::{AT, Interval, LogEndpoint, LogEvidence, LogSeries, Plan};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::{Fault, InvalidCode, Rational};
use std::cmp::Ordering;

/// Monotone endpoint extension; computed zero boundaries are refinable.
pub(in crate::enclosure) fn produce(
    input: &Interval,
    record: u32,
    plan: Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(Interval, LogEvidence), Fault> {
    if input.upper().is_zero() || input.upper().is_negative() {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    if input.lower().is_zero() || input.lower().is_negative() {
        return Err(Fault::invalid(InvalidCode::Policy, super::UNSEPARATED));
    }
    let (lower, left) = endpoint(input.lower(), plan, budget, interrupt)?;
    let (upper, right) = if input.lower().compare(input.upper(), budget)? == Ordering::Equal {
        budget.work(
            super::endpoint_bytes(&lower).saturating_add(super::super::interval_bytes(&left)),
            AT,
        )?;
        (lower.clone(), left.clone())
    } else {
        endpoint(input.upper(), plan, budget, interrupt)?
    };
    let bound = Interval::new(left.lower().clone(), right.upper().clone(), budget)?;
    Ok((
        bound,
        LogEvidence {
            record,
            lower,
            upper,
        },
    ))
}

/// Reduce by a bit-length estimate followed by exact comparisons.
fn endpoint(
    x: &Rational,
    plan: Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(LogEndpoint, Interval), Fault> {
    let (n, d) = x.component_bits();
    let mut exponent = i64::try_from(n).map_err(|_| super::invalid())?
        - i64::try_from(d).map_err(|_| super::invalid())?;
    let two = Rational::from_ratio(2, 1, budget)?;
    budget.check(BudgetKind::IntegerExponent, exponent.unsigned_abs(), AT)?;
    let mut mantissa = x.mul(&two.pow(-exponent, budget)?, budget)?;
    if mantissa.compare(&Rational::one(), budget)? == Ordering::Less {
        exponent -= 1;
        mantissa = mantissa.mul(&two, budget)?;
    }
    if exponent.unsigned_abs() > u64::from(plan.reduction) {
        return Err(Fault::invalid(InvalidCode::Policy, AT));
    }
    let argument = mantissa
        .sub(&Rational::one(), budget)?
        .div(&mantissa.add(&Rational::one(), budget)?, budget)?;
    let reduced = series(&argument, plan, budget, interrupt)?;
    let two_series = series(
        &if exponent == 0 {
            Rational::zero()
        } else {
            Rational::from_ratio(1, 3, budget)?
        },
        plan,
        budget,
        interrupt,
    )?;
    let reduced_bound = bound(&reduced, plan.grid, budget)?;
    let two_bound = bound(&two_series, plan.grid, budget)?;
    let factor = Interval::singleton(&Rational::from_ratio(exponent, 1, budget)?, budget)?;
    let result = reduced_bound.add(
        &two_bound.mul(&factor, plan.grid, budget)?,
        plan.grid,
        budget,
    )?;
    Ok((
        LogEndpoint {
            exponent,
            mantissa,
            argument,
            reduced,
            two: two_series,
        },
        result,
    ))
}

/// Ascending odd terms with outward rounding after every scheduled operation.
fn series(
    u: &Rational,
    plan: Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<LogSeries, Fault> {
    let mut power = Interval::singleton(u, budget)?;
    let zero = Interval::singleton(&Rational::zero(), budget)?;
    if u.is_zero() {
        return Ok(LogSeries {
            terms: 0,
            sum: zero.clone(),
            power,
            tail: zero,
        });
    }
    budget.check(BudgetKind::ProofNodes, u64::from(plan.terms), AT)?;
    let square = power.mul(&power, plan.grid, budget)?;
    let mut sum = zero;
    for j in 0..plan.terms {
        super::super::boundary(budget, interrupt)?;
        let factor = Interval::singleton(
            &Rational::from_ratio(2, 2 * u64::from(j) + 1, budget)?,
            budget,
        )?;
        sum = sum.add(&power.mul(&factor, plan.grid, budget)?, plan.grid, budget)?;
        power = power.mul(&square, plan.grid, budget)?;
    }
    let one = Interval::singleton(&Rational::one(), budget)?;
    let odd = Interval::singleton(
        &Rational::from_ratio(i64::from(plan.terms) * 2 + 1, 1, budget)?,
        budget,
    )?;
    let denominator = one
        .sub(&square, plan.grid, budget)?
        .mul(&odd, plan.grid, budget)?;
    let two = Interval::singleton(&Rational::from_ratio(2, 1, budget)?, budget)?;
    let tail = power.mul(&two, plan.grid, budget)?.mul(
        &denominator.reciprocal(plan.grid, budget)?,
        plan.grid,
        budget,
    )?;
    Ok(LogSeries {
        terms: plan.terms,
        sum,
        power,
        tail,
    })
}

/// Tail is added only upwards to the already outward partial sum.
fn bound(series: &LogSeries, grid: u32, budget: &mut Budget) -> Result<Interval, Fault> {
    Interval::new(
        series.sum.lower().clone(),
        series.sum.upper().add(series.tail.upper(), budget)?,
        budget,
    )?
    .coarsen(grid, budget)
}
