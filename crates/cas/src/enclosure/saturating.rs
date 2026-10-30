// SPDX-License-Identifier: AGPL-3.0-only
//! Sign-selected compositions retain their original arguments and exponential premises.

use super::{AT, ExpEndpoint, ExpEvidence, Interval, boundary};
use crate::budget::{Budget, BudgetUsage};
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Propose the two monotone endpoints using the stable sign-selected quotient.
pub(super) fn produce(
    input: &Interval,
    record: u32,
    plan: super::log::Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(Interval, ExpEvidence), Fault> {
    let (lower, lo) = produce_endpoint(input.lower(), plan, budget, interrupt)?;
    let (upper, hi) = if input.lower().compare(input.upper(), budget)? == Ordering::Equal {
        budget.work(
            super::exp::endpoint_bytes(&lower).saturating_add(super::interval_bytes(&lo)),
            AT,
        )?;
        (lower.clone(), lo.clone())
    } else {
        produce_endpoint(input.upper(), plan, budget, interrupt)?
    };
    let result = Interval::new(
        lo.lower().add(&Rational::zero(), budget)?,
        hi.upper().add(&Rational::zero(), budget)?,
        budget,
    )?;
    Ok((
        result,
        ExpEvidence {
            record,
            lower,
            upper,
        },
    ))
}

/// The proposal uses interval operations; only independent replay grants authority.
fn produce_endpoint(
    x: &Rational,
    plan: super::log::Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(ExpEndpoint, Interval), Fault> {
    boundary(budget, interrupt)?;
    let argument = if x.is_negative() {
        x.add(&Rational::zero(), budget)?
    } else {
        x.negated(budget)?
    };
    let (proof, exponential) = super::exp::produce_endpoint(&argument, plan, budget, interrupt)?;
    let one = Interval::singleton(&Rational::one(), budget)?;
    let denominator = one.add(&exponential, plan.grid, budget)?;
    if denominator.lower().is_zero() || denominator.lower().is_negative() {
        return Err(super::exp::invalid());
    }
    let numerator = if x.is_negative() { &exponential } else { &one };
    let bound = numerator.mul(
        &denominator.reciprocal(plan.grid, budget)?,
        plan.grid,
        budget,
    )?;
    let range = Interval::new(Rational::zero(), Rational::one(), budget)?;
    let result = bound.intersection(&range, budget)?;
    Ok((proof, result))
}

/// Replay each exponential from the transformed original argument, never a proposed branch.
pub(super) fn check(
    input: &Interval,
    evidence: &ExpEvidence,
    proposed: &Interval,
    q: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(), Fault> {
    let lo = check_endpoint(input.lower(), &evidence.lower, q, budget, interrupt)?;
    budget.work(
        super::exp::endpoint_bytes(&evidence.lower)
            .saturating_add(super::exp::endpoint_bytes(&evidence.upper)),
        AT,
    )?;
    let hi = if input.lower().compare(input.upper(), budget)? == Ordering::Equal
        && evidence.lower == evidence.upper
    {
        budget.work(super::interval_bytes(&lo), AT)?;
        lo.clone()
    } else {
        check_endpoint(input.upper(), &evidence.upper, q, budget, interrupt)?
    };
    if proposed.lower().compare(lo.lower(), budget)? != Ordering::Equal
        || proposed.upper().compare(hi.upper(), budget)? != Ordering::Equal
    {
        return Err(super::exp::invalid());
    }
    Ok(())
}

/// Exact endpoint relations independently verify composition, separation and range.
fn check_endpoint(
    x: &Rational,
    proof: &ExpEndpoint,
    q: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Interval, Fault> {
    boundary(budget, interrupt)?;
    let argument = if x.is_negative() {
        x.add(&Rational::zero(), budget)?
    } else {
        x.negated(budget)?
    };
    let exponential = super::exp::check_endpoint(&argument, proof, q, budget, interrupt)?;
    let one = Interval::new(Rational::one(), Rational::one(), budget)?;
    let denominator = relation(&crate::model::Operator::Add, &one, &exponential, q, budget)?;
    if denominator.lower().compare(&Rational::zero(), budget)? != Ordering::Greater {
        return Err(super::exp::invalid());
    }
    let numerator = if x.is_negative() { &exponential } else { &one };
    let bound = relation(
        &crate::model::Operator::Div,
        numerator,
        &denominator,
        q,
        budget,
    )?;
    let lower = if bound.lower().is_negative() {
        Rational::zero()
    } else {
        bound.lower().add(&Rational::zero(), budget)?
    };
    let upper = if bound.upper().compare(&Rational::one(), budget)? == Ordering::Greater {
        Rational::one()
    } else {
        bound.upper().add(&Rational::zero(), budget)?
    };
    Interval::new(lower, upper, budget)
}

/// Use only the exact core relation checker, not production interval arithmetic.
fn relation(
    operator: &crate::model::Operator,
    a: &Interval,
    b: &Interval,
    q: u32,
    budget: &mut Budget,
) -> Result<Interval, Fault> {
    let (lo, hi) = super::checker::relation(operator, a, b, q, budget)?;
    let (lo, hi) = super::checker::grid(&lo, &hi, q, budget)?;
    Interval::new(lo, hi, budget)
}
