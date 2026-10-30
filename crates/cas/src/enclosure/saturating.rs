// SPDX-License-Identifier: AGPL-3.0-only
//! Sign-selected compositions retain their original arguments and exponential premises.

use super::{AT, ExpEndpoint, ExpEvidence, Interval, boundary};
use crate::budget::{Budget, BudgetUsage};
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Propose the two monotone endpoints using the stable sign-selected quotient.
fn produce_family(
    family: Family,
    input: &Interval,
    record: u32,
    plan: super::log::Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(Interval, ExpEvidence), Fault> {
    let (lower, lo) = produce_endpoint(family, input.lower(), plan, budget, interrupt)?;
    let (upper, hi) = if input.lower().compare(input.upper(), budget)? == Ordering::Equal {
        budget.work(
            super::exp::endpoint_bytes(&lower).saturating_add(super::interval_bytes(&lo)),
            AT,
        )?;
        (lower.clone(), lo.clone())
    } else {
        produce_endpoint(family, input.upper(), plan, budget, interrupt)?
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
    family: Family,
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
    let argument = if family == Family::Tanh {
        argument.mul(&Rational::from_ratio(2, 1, budget)?, budget)?
    } else {
        argument
    };
    let (proof, exponential) = super::exp::produce_endpoint(&argument, plan, budget, interrupt)?;
    let increasing = family == Family::Sigmoid && x.is_negative();
    let (lo, hi) = if increasing {
        (exponential.lower(), exponential.upper())
    } else {
        (exponential.upper(), exponential.lower())
    };
    let bound = Interval::new(
        produce_quotient(family, x.is_negative(), lo, budget)?,
        produce_quotient(family, x.is_negative(), hi, budget)?,
        budget,
    )?
    .coarsen(plan.grid, budget)?;
    let bound = if family == Family::Tanh && x.is_negative() {
        Interval::new(
            bound.upper().negated(budget)?,
            bound.lower().negated(budget)?,
            budget,
        )?
    } else {
        bound
    };
    let minimum = if family == Family::Tanh {
        Rational::from_ratio(-1, 1, budget)?
    } else {
        Rational::zero()
    };
    let range = Interval::new(minimum, Rational::one(), budget)?;
    let result = bound.intersection(&range, budget)?;
    Ok((proof, result))
}

/// Replay each exponential from the transformed original argument, never a proposed branch.
fn check_family(
    family: Family,
    input: &Interval,
    evidence: &ExpEvidence,
    proposed: &Interval,
    q: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(), Fault> {
    let lo = check_endpoint(family, input.lower(), &evidence.lower, q, budget, interrupt)?;
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
        check_endpoint(family, input.upper(), &evidence.upper, q, budget, interrupt)?
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
    family: Family,
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
    let argument = if family == Family::Tanh {
        argument.mul(&Rational::from_ratio(2, 1, budget)?, budget)?
    } else {
        argument
    };
    let exponential = super::exp::check_endpoint(&argument, proof, q, budget, interrupt)?;
    let low_image = check_quotient(family, x.is_negative(), exponential.lower(), budget)?;
    let high_image = check_quotient(family, x.is_negative(), exponential.upper(), budget)?;
    let (lo, hi) = if family == Family::Sigmoid && x.is_negative() {
        (low_image, high_image)
    } else {
        (high_image, low_image)
    };
    let (lo, hi) = super::checker::grid(&lo, &hi, q, budget)?;
    let bound = Interval::new(lo, hi, budget)?;
    let bound = if family == Family::Tanh && x.is_negative() {
        Interval::new(
            bound.upper().negated(budget)?,
            bound.lower().negated(budget)?,
            budget,
        )?
    } else {
        bound
    };
    let lower = if family == Family::Tanh {
        let minimum = Rational::from_ratio(-1, 1, budget)?;
        if bound.lower().compare(&minimum, budget)? == Ordering::Less {
            minimum
        } else {
            bound.lower().add(&Rational::zero(), budget)?
        }
    } else if bound.lower().is_negative() {
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

/// Evaluate a scalar quotient exactly before the producer rounds outward.
fn produce_quotient(
    family: Family,
    negative: bool,
    e: &Rational,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    let denominator = Rational::one().add(e, budget)?;
    if denominator.is_zero() || denominator.is_negative() {
        return Err(super::exp::invalid());
    }
    let numerator = match family {
        Family::Tanh => Rational::one().sub(e, budget)?,
        Family::Sigmoid if negative => e.add(&Rational::zero(), budget)?,
        Family::Sigmoid => Rational::one(),
    };
    numerator.div(&denominator, budget)
}

/// Independently reconstruct the exact scalar map and its positive denominator.
fn check_quotient(
    family: Family,
    negative: bool,
    e: &Rational,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    let denominator = e.add(&Rational::one(), budget)?;
    if denominator.compare(&Rational::zero(), budget)? != Ordering::Greater {
        return Err(super::exp::invalid());
    }
    if family == Family::Tanh {
        Rational::one().sub(e, budget)?.div(&denominator, budget)
    } else if negative {
        e.div(&denominator, budget)
    } else {
        Rational::one().div(&denominator, budget)
    }
}

/// Closed composition choice derived from the original operator, never from evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Family {
    /// Logistic quotient with a half value at zero.
    Sigmoid,
    /// Hyperbolic quotient with odd reflection.
    Tanh,
}

/// Propose the sigmoid family without conferring checked authority.
pub(super) fn produce(
    input: &Interval,
    record: u32,
    plan: super::log::Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(Interval, ExpEvidence), Fault> {
    produce_family(Family::Sigmoid, input, record, plan, budget, interrupt)
}

/// Independently replay the original sigmoid composition and all its premises.
pub(super) fn check(
    input: &Interval,
    evidence: &ExpEvidence,
    proposed: &Interval,
    q: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(), Fault> {
    check_family(
        Family::Sigmoid,
        input,
        evidence,
        proposed,
        q,
        budget,
        interrupt,
    )
}

/// Propose the tanh family without conferring checked authority.
pub(super) fn produce_tanh(
    input: &Interval,
    record: u32,
    plan: super::log::Plan,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(Interval, ExpEvidence), Fault> {
    produce_family(Family::Tanh, input, record, plan, budget, interrupt)
}

/// Independently replay the original tanh composition and all its premises.
pub(super) fn check_tanh(
    input: &Interval,
    evidence: &ExpEvidence,
    proposed: &Interval,
    q: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(), Fault> {
    check_family(
        Family::Tanh,
        input,
        evidence,
        proposed,
        q,
        budget,
        interrupt,
    )
}
