// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded power proposals compose the existing producers without granting authority.

use super::{AT, Interval, PowerEvidence, PowerRoute, Produced, invalid};
use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::{Fault, InvalidCode, Rational};

/// Select the mandatory route only after checking the original positive domain.
pub(in crate::enclosure) fn produce(
    base: &Interval,
    exponent: &Rational,
    schedule: (u32, u32, super::super::planning::Plans),
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Produced, Fault> {
    super::super::boundary(budget, interrupt)?;
    if base.upper().is_zero() || base.upper().is_negative() {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    if base.lower().is_zero() || base.lower().is_negative() {
        return Err(Fault::invalid(
            InvalidCode::RefutedDomain,
            super::super::log::UNSEPARATED,
        ));
    }
    let (p, d) = super::exponent(exponent, budget)?;
    let (record, grid, plans) = schedule;
    let route = if d == 1 {
        PowerRoute::Integer
    } else if d.is_power_of_two() {
        PowerRoute::Dyadic
    } else {
        PowerRoute::LogExp
    };
    let mut evidence = PowerEvidence {
        record,
        route,
        roots: Vec::new(),
        logarithm: None,
    };
    let mut log = None;
    let mut exp = None;
    let bound = if route == PowerRoute::LogExp {
        let (logarithm, proof) = super::super::log::produce(
            base,
            record,
            plans.log.ok_or_else(invalid)?,
            budget,
            interrupt,
        )?;
        let magnitude = if exponent.is_negative() {
            exponent.negated(budget)?
        } else {
            exponent.add(&Rational::zero(), budget)?
        };
        let argument = logarithm.mul(&Interval::singleton(&magnitude, budget)?, grid, budget)?;
        let (result, exponential) = super::super::exp::produce(
            &argument,
            record,
            plans.exp.ok_or_else(invalid)?,
            budget,
            interrupt,
        )?;
        evidence.logarithm = Some(logarithm);
        log = Some(proof);
        exp = Some(exponential);
        result
    } else {
        finite(base, (p, d), &mut evidence, grid, budget, interrupt)?
    };
    let bound = if p < 0 {
        if bound.lower().is_zero() {
            return Err(Fault::invalid(
                InvalidCode::RefutedDomain,
                super::super::log::UNSEPARATED,
            ));
        }
        bound.reciprocal(grid, budget)?
    } else {
        bound
    };
    Ok(Produced {
        bound,
        evidence,
        log,
        exp,
    })
}

/// Root repetition and integral squaring have bounded exact endpoint operations.
fn finite(
    base: &Interval,
    exponent: (i64, u64),
    evidence: &mut PowerEvidence,
    grid: u32,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<Interval, Fault> {
    let (p, d) = exponent;
    let mut input = base.copied(budget)?;
    for _ in 0..d.trailing_zeros() {
        super::super::boundary(budget, interrupt)?;
        input = super::super::roots::produce(&input, grid, budget)?;
        let size = super::bytes(std::slice::from_ref(evidence))
            .saturating_add(super::super::interval_bytes(&input));
        budget.check(BudgetKind::StagedPayloadBytes, size, AT)?;
        budget.work(super::super::interval_bytes(&input), AT)?;
        evidence
            .roots
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        evidence.roots.push(input.clone());
    }
    let magnitude = i64::try_from(p.unsigned_abs()).map_err(|_| invalid())?;
    Interval::new(
        input.lower().pow(magnitude, budget)?,
        input.upper().pow(magnitude, budget)?,
        budget,
    )?
    .coarsen(grid, budget)
}
