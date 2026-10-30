// SPDX-License-Identifier: AGPL-3.0-only
//! Reconstruct finite log instances without reduction or series producer calls.

use super::{AT, Interval, LogEndpoint, LogEvidence, LogSeries, invalid};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::model::Operator;
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Monotonicity joins two independently checked strictly positive endpoint instances.
pub(in crate::enclosure) fn check(
    input: &Interval,
    evidence: &LogEvidence,
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

/// Exact equality and range checks authenticate the proposed binary reduction.
fn endpoint(
    x: &Rational,
    proof: &LogEndpoint,
    grid: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Interval, Fault> {
    if x.is_negative() || x.is_zero() {
        return Err(invalid());
    }
    budget.check(
        BudgetKind::IntegerExponent,
        proof.exponent.unsigned_abs(),
        AT,
    )?;
    let two = Rational::from_ratio(2, 1, budget)?;
    if proof.mantissa.compare(&Rational::one(), budget)? == Ordering::Less
        || proof.mantissa.compare(&two, budget)? != Ordering::Less
        || two
            .pow(proof.exponent, budget)?
            .mul(&proof.mantissa, budget)?
            .compare(x, budget)?
            != Ordering::Equal
    {
        return Err(invalid());
    }
    let numerator = proof.mantissa.sub(&Rational::one(), budget)?;
    let denominator = proof.mantissa.add(&Rational::one(), budget)?;
    if proof
        .argument
        .mul(&denominator, budget)?
        .compare(&numerator, budget)?
        != Ordering::Equal
    {
        return Err(invalid());
    }
    let reduced = series(&proof.argument, &proof.reduced, grid, budget, interrupt)?;
    let u_two = if proof.exponent == 0 {
        Rational::zero()
    } else {
        Rational::from_ratio(1, 3, budget)?
    };
    let logarithm_two = series(&u_two, &proof.two, grid, budget, interrupt)?;
    let factor = point(&Rational::from_ratio(proof.exponent, 1, budget)?, budget)?;
    let scaled = op(&Operator::Mul, &logarithm_two, &factor, grid, budget)?;
    op(&Operator::Add, &reduced, &scaled, grid, budget)
}

/// Check the analytic law's finite recurrence with separately traversed exact relations.
fn series(
    u: &Rational,
    proof: &LogSeries,
    q: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Interval, Fault> {
    if u.is_negative()
        || u.compare(&Rational::from_ratio(1, 3, budget)?, budget)? == Ordering::Greater
    {
        return Err(invalid());
    }
    let zero = point(&Rational::zero(), budget)?;
    if u.is_zero() {
        if proof.terms != 0 || proof.sum != zero || proof.power != zero || proof.tail != zero {
            return Err(invalid());
        }
        return Ok(zero);
    }
    if proof.terms == 0 {
        return Err(invalid());
    }
    budget.check(BudgetKind::ProofNodes, u64::from(proof.terms), AT)?;
    let mut power = point(u, budget)?;
    let squared = op(&Operator::Mul, &power, &power, q, budget)?;
    let mut sum = zero;
    for j in 0..proof.terms {
        super::super::boundary(budget, interrupt)?;
        let coefficient = point(
            &Rational::from_ratio(2, 2 * u64::from(j) + 1, budget)?,
            budget,
        )?;
        let term = op(&Operator::Mul, &power, &coefficient, q, budget)?;
        sum = op(&Operator::Add, &sum, &term, q, budget)?;
        power = op(&Operator::Mul, &power, &squared, q, budget)?;
    }
    let one = point(&Rational::one(), budget)?;
    let remaining = op(&Operator::Sub, &one, &squared, q, budget)?;
    if remaining.lower().is_negative() || remaining.lower().is_zero() {
        return Err(invalid());
    }
    let odd = point(
        &Rational::from_ratio(2 * i64::from(proof.terms) + 1, 1, budget)?,
        budget,
    )?;
    let denominator = op(&Operator::Mul, &remaining, &odd, q, budget)?;
    let two = point(&Rational::from_ratio(2, 1, budget)?, budget)?;
    let numerator = op(&Operator::Mul, &power, &two, q, budget)?;
    let tail = op(&Operator::Div, &numerator, &denominator, q, budget)?;
    if proof.sum != sum || proof.power != power || proof.tail != tail {
        return Err(invalid());
    }
    let upper = sum.upper().add(tail.upper(), budget)?;
    let (lower, upper) = super::super::checker::grid(sum.lower(), &upper, q, budget)?;
    Interval::new(lower, upper, budget)
}

/// Reuse only the independent rational relation checker, never producer interval operations.
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

/// Make an exact point by metered rational copies.
fn point(value: &Rational, budget: &mut Budget) -> Result<Interval, Fault> {
    Interval::new(
        value.add(&Rational::zero(), budget)?,
        value.add(&Rational::zero(), budget)?,
        budget,
    )
}
