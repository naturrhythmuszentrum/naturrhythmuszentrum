// SPDX-License-Identifier: AGPL-3.0-only
//! Proposals use integer bit trials; replay checks only exact squared endpoint relations.

use super::{AT, Interval};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::number::Rational;
use std::cmp::Ordering;

/// Authenticate the original exponent itself, independently of admission metadata.
pub(super) fn exponent(operator: &crate::model::Operator) -> Result<(), Fault> {
    if matches!(operator, crate::model::Operator::Rational(value) if value.is_half()) {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Monotonicity lifts the two independently bounded rational endpoint roots.
pub(super) fn produce(value: &Interval, grid: u32, budget: &mut Budget) -> Result<Interval, Fault> {
    super::interval::scratch(budget)?;
    let (lower, _) = value.lower().square_root_grid(grid, budget)?;
    let (_, upper) = value.upper().square_root_grid(grid, budget)?;
    Interval::new(lower, upper, budget)
}

/// Reconstruct the grid law without calling the root producer or integer root algorithm.
pub(super) fn check(
    input: &Interval,
    proposed: &Interval,
    grid: u32,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.check(BudgetKind::IntegerExponent, u64::from(grid) * 2, AT)?;
    budget.check(BudgetKind::ComponentBits, u64::from(grid) + 1, AT)?;
    if input.lower().is_negative() {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    let unit = Rational::from_ratio(2, 1, budget)?.pow(-i64::from(grid), budget)?;
    endpoint(input.lower(), proposed.lower(), &unit, false, budget)?;
    endpoint(input.upper(), proposed.upper(), &unit, true, budget)
}

/// Squaring grid neighbors proves the unique floor or ceiling; exact roots are singletons.
fn endpoint(
    radicand: &Rational,
    candidate: &Rational,
    unit: &Rational,
    upper: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if candidate.is_negative() || !candidate.div(unit, budget)?.is_integer() {
        return Err(invalid());
    }
    let square = candidate.mul(candidate, budget)?;
    let order = square.compare(radicand, budget)?;
    if order == Ordering::Equal {
        return Ok(());
    }
    if upper {
        if order != Ordering::Greater || candidate.compare(unit, budget)? == Ordering::Less {
            return Err(invalid());
        }
        let previous = candidate.sub(unit, budget)?;
        if previous.mul(&previous, budget)?.compare(radicand, budget)? != Ordering::Less {
            return Err(invalid());
        }
    } else {
        if order != Ordering::Less {
            return Err(invalid());
        }
        let next = candidate.add(unit, budget)?;
        if next.mul(&next, budget)?.compare(radicand, budget)? != Ordering::Greater {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A wrong endpoint, sign or rounding direction is invalid evidence, never indecision.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
