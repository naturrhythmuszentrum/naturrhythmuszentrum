// SPDX-License-Identifier: AGPL-3.0-only
//! Replay reconstructs the route and all exact relations without the power producer.

use super::{AT, Interval, PowerEvidence, PowerRoute, invalid};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Component evidence shares one transcript-local checked log-two cache.
type Analytic<'a, 'b> = (
    Option<&'a super::super::LogEvidence>,
    Option<&'a super::super::ExpEvidence>,
    &'b mut Vec<(&'a super::super::LogSeries, Interval)>,
);

/// Authenticate the original exponent and component premises before accepting the result.
pub(in crate::enclosure) fn check(
    inputs: (&Interval, &Rational),
    evidence: &PowerEvidence,
    analytic: &mut Analytic<'_, '_>,
    proposed: &Interval,
    grid: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<(), Fault> {
    super::super::boundary(budget, interrupt)?;
    let (base, exponent) = inputs;
    if base.lower().compare(&Rational::zero(), budget)? != Ordering::Greater {
        return Err(invalid());
    }
    let (p, d) = exponent.small_ratio(budget)?.ok_or_else(invalid)?;
    budget.check(BudgetKind::IntegerExponent, p.unsigned_abs(), AT)?;
    let root_count = if d.is_power_of_two() {
        d.trailing_zeros()
    } else {
        0
    };
    budget.check(BudgetKind::IntegerExponent, u64::from(root_count), AT)?;
    let expected = match d {
        1 => PowerRoute::Integer,
        _ if d.is_power_of_two() => PowerRoute::Dyadic,
        _ => PowerRoute::LogExp,
    };
    if evidence.route != expected
        || evidence.roots.len() != usize::try_from(root_count).map_err(|_| invalid())?
    {
        return Err(invalid());
    }
    let bound = if expected == PowerRoute::LogExp {
        analytic_bound(base, exponent, evidence, analytic, grid, budget, interrupt)?
    } else {
        if analytic.0.is_some() || analytic.1.is_some() || evidence.logarithm.is_some() {
            return Err(invalid());
        }
        let mut input = base;
        for root in &evidence.roots {
            super::super::boundary(budget, interrupt)?;
            super::super::roots::check(input, root, grid, budget)?;
            input = root;
        }
        let magnitude = i64::try_from(p.unsigned_abs()).map_err(|_| invalid())?;
        let lo = input.lower().pow(magnitude, budget)?;
        let hi = input.upper().pow(magnitude, budget)?;
        rounded(&lo, &hi, grid, budget)?
    };
    let bound = if p < 0 {
        if bound.lower().compare(&Rational::zero(), budget)? != Ordering::Greater {
            return Err(invalid());
        }
        let lo = Rational::one().div(bound.upper(), budget)?;
        let hi = Rational::one().div(bound.lower(), budget)?;
        rounded(&lo, &hi, grid, budget)?
    } else {
        bound
    };
    if &bound != proposed {
        return Err(invalid());
    }
    Ok(())
}

/// Reconstruct log scaling and require both component schemas at the same original node.
fn analytic_bound(
    base: &Interval,
    exponent: &Rational,
    evidence: &PowerEvidence,
    analytic: &mut Analytic<'_, '_>,
    grid: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Interval, Fault> {
    let log = analytic.0.ok_or_else(invalid)?;
    let exp = analytic.1.ok_or_else(invalid)?;
    if log.record != evidence.record || exp.record != evidence.record {
        return Err(invalid());
    }
    let logarithm = evidence.logarithm.as_ref().ok_or_else(invalid)?;
    super::super::log::check(base, log, logarithm, grid, analytic.2, budget, interrupt)?;
    let magnitude = if exponent.is_negative() {
        exponent.negated(budget)?
    } else {
        exponent.add(&Rational::zero(), budget)?
    };
    let lo = logarithm.lower().mul(&magnitude, budget)?;
    let hi = logarithm.upper().mul(&magnitude, budget)?;
    let argument = rounded(&lo, &hi, grid, budget)?;
    let lo =
        super::super::exp::check_endpoint(argument.lower(), &exp.lower, grid, budget, interrupt)?;
    let hi =
        super::super::exp::check_endpoint(argument.upper(), &exp.upper, grid, budget, interrupt)?;
    Interval::new(
        lo.lower().add(&Rational::zero(), budget)?,
        hi.upper().add(&Rational::zero(), budget)?,
        budget,
    )
}

/// Independent exact outward relations, never production interval coarsening.
fn rounded(
    lo: &Rational,
    hi: &Rational,
    grid: u32,
    budget: &mut Budget,
) -> Result<Interval, Fault> {
    let (lo, hi) = super::super::checker::grid(lo, hi, grid, budget)?;
    Interval::new(lo, hi, budget)
}
