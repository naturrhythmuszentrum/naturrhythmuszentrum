// SPDX-License-Identifier: AGPL-3.0-only
//! Independent finite relations instantiate the exponential law, never the producer.

use super::{AT, ExpEndpoint, ExpEvidence, ExpSeries, Interval, invalid};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::model::Operator;
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Check both original endpoints and their monotone join.
pub(in crate::enclosure) fn check(
    input: &Interval,
    evidence: &ExpEvidence,
    proposed: &Interval,
    grid: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(), Fault> {
    let lo = endpoint(input.lower(), &evidence.lower, grid, budget, interrupt)?;
    budget.work(
        super::endpoint_bytes(&evidence.lower)
            .saturating_add(super::endpoint_bytes(&evidence.upper)),
        AT,
    )?;
    let hi = if input.lower().compare(input.upper(), budget)? == Ordering::Equal
        && evidence.lower == evidence.upper
    {
        budget.work(super::super::interval_bytes(&lo), AT)?;
        lo.clone()
    } else {
        endpoint(input.upper(), &evidence.upper, grid, budget, interrupt)?
    };
    if proposed.lower().compare(lo.lower(), budget)? != Ordering::Equal
        || proposed.upper().compare(hi.upper(), budget)? != Ordering::Equal
    {
        return Err(invalid());
    }
    Ok(())
}

/// Authenticate the reduction from the original signed rational and check every square.
pub(in crate::enclosure) fn endpoint(
    x: &Rational,
    proof: &ExpEndpoint,
    q: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Interval, Fault> {
    budget.check(BudgetKind::IntegerExponent, u64::from(proof.squarings), AT)?;
    let a = &proof.argument;
    let half = Rational::from_ratio(1, 2, budget)?;
    let quarter = Rational::from_ratio(1, 4, budget)?;
    if a.is_negative()
        || a.compare(&half, budget)? == Ordering::Greater
        || (proof.squarings != 0 && a.compare(&quarter, budget)? != Ordering::Greater)
    {
        return Err(invalid());
    }
    let magnitude = if x.is_negative() {
        x.negated(budget)?
    } else {
        x.add(&Rational::zero(), budget)?
    };
    let scale = Rational::from_ratio(2, 1, budget)?.pow(i64::from(proof.squarings), budget)?;
    if a.mul(&scale, budget)?.compare(&magnitude, budget)? != Ordering::Equal {
        return Err(invalid());
    }
    let mut result = series(a, &proof.series, q, budget, interrupt)?;
    for _ in 0..proof.squarings {
        super::super::boundary(budget, interrupt)?;
        result = op(&Operator::Mul, &result, &result, q, budget)?;
    }
    if x.is_negative() {
        result = op(
            &Operator::Div,
            &point(&Rational::one(), budget)?,
            &result,
            q,
            budget,
        )?;
    }
    Ok(result)
}

/// Rebuild finite sums, omitted terms and the strictly positive tail denominator.
fn series(
    a: &Rational,
    proof: &ExpSeries,
    q: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Interval, Fault> {
    let one = point(&Rational::one(), budget)?;
    if a.is_zero() {
        let zero = point(&Rational::zero(), budget)?;
        if proof.degree != 0 || proof.sum != one || proof.omitted != zero || proof.tail != zero {
            return Err(invalid());
        }
        return Ok(one);
    }
    if proof.degree == 0 {
        return Err(invalid());
    }
    budget.check(BudgetKind::ProofNodes, u64::from(proof.degree) + 1, AT)?;
    let argument = point(a, budget)?;
    let mut term = point(&Rational::one(), budget)?;
    let mut sum = point(&Rational::one(), budget)?;
    for j in 1..=proof.degree {
        super::super::boundary(budget, interrupt)?;
        let inverse = point(&Rational::from_ratio(1, u64::from(j), budget)?, budget)?;
        term = op(
            &Operator::Mul,
            &op(&Operator::Mul, &term, &argument, q, budget)?,
            &inverse,
            q,
            budget,
        )?;
        sum = op(&Operator::Add, &sum, &term, q, budget)?;
    }
    let inverse = point(
        &Rational::from_ratio(1, u64::from(proof.degree) + 1, budget)?,
        budget,
    )?;
    let omitted = op(
        &Operator::Mul,
        &op(&Operator::Mul, &term, &argument, q, budget)?,
        &inverse,
        q,
        budget,
    )?;
    let divisor = point(
        &Rational::from_ratio(1, u64::from(proof.degree) + 2, budget)?,
        budget,
    )?;
    let ratio = op(&Operator::Mul, &argument, &divisor, q, budget)?;
    let denominator = op(&Operator::Sub, &one, &ratio, q, budget)?;
    if denominator.lower().is_negative() || denominator.lower().is_zero() {
        return Err(invalid());
    }
    let tail = op(&Operator::Div, &omitted, &denominator, q, budget)?;
    if proof.sum != sum || proof.omitted != omitted || proof.tail != tail {
        return Err(invalid());
    }
    let upper = sum.upper().add(tail.upper(), budget)?;
    let (lower, upper) = super::super::checker::grid(sum.lower(), &upper, q, budget)?;
    Interval::new(lower, upper, budget)
}

/// Exact rational relations and outward grid checks are shared with the core checker only.
fn op(
    operator: &Operator,
    a: &Interval,
    b: &Interval,
    q: u32,
    budget: &mut Budget,
) -> Result<Interval, Fault> {
    let (lo, hi) = super::super::checker::relation(operator, a, b, q, budget)?;
    let (lo, hi) = super::super::checker::grid(&lo, &hi, q, budget)?;
    Interval::new(lo, hi, budget)
}

/// Metered exact point copies do not invoke producer interval operations.
fn point(value: &Rational, budget: &mut Budget) -> Result<Interval, Fault> {
    Interval::new(
        value.add(&Rational::zero(), budget)?,
        value.add(&Rational::zero(), budget)?,
        budget,
    )
}
