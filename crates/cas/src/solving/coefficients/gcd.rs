// SPDX-License-Identifier: AGPL-3.0-only
//! Recursive content/primitive-part polynomial gcd, with exact quotient verification.
//!
//! Multivariate coprimality is never replaced by a Bezout-to-one assertion.

use super::polynomial::{Polynomial, Term};
use super::scalar::Scalar;
use super::storage::{AT, begin, invalid, vector};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::size;

/// An explicit zero in the same polynomial ring.
fn zero(value: &Polynomial, budget: &mut Budget) -> Result<Polynomial, Fault> {
    Polynomial::constant(
        value.variables,
        Scalar::constant(value.ground, false, budget)?,
        budget,
    )
}

/// Degree in one fixed variable; zero has no degree.
pub(crate) fn degree(
    value: &Polynomial,
    variable: usize,
    budget: &mut Budget,
) -> Result<Option<u32>, Fault> {
    begin(budget)?;
    budget.work(size(value.terms.len(), AT)?, AT)?;
    let mut result = None;
    for term in &value.terms {
        let power = *term.powers.get(variable).ok_or_else(invalid)?;
        result = Some(result.map_or(power, |earlier: u32| earlier.max(power)));
    }
    Ok(result)
}

/// Extract a coefficient in the last active variable, keeping the original frame width.
fn coefficient(
    value: &Polynomial,
    variable: usize,
    power: u32,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    begin(budget)?;
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        budget.work(1, AT)?;
        if term.powers.get(variable) == Some(&power) {
            let mut entry = term.copy(budget)?;
            *entry.powers.get_mut(variable).ok_or_else(invalid)? = 0;
            terms.push(entry);
        }
    }
    Polynomial::new(value.variables, value.ground, terms, budget)
}

/// Multiply by a formal variable power with checked exponent arithmetic.
fn shift(
    value: &Polynomial,
    variable: usize,
    power: u32,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        let mut entry = term.copy(budget)?;
        let slot = entry.powers.get_mut(variable).ok_or_else(invalid)?;
        *slot = slot
            .checked_add(power)
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        terms.push(entry);
    }
    Polynomial::new(value.variables, value.ground, terms, budget)
}

/// One multivariate long-division leading monomial, if divisibility is possible.
fn quotient_term(
    dividend: &Term,
    divisor: &Term,
    budget: &mut Budget,
) -> Result<Option<Term>, Fault> {
    let mut powers = vector(dividend.powers.len(), budget)?;
    for (left, right) in dividend.powers.iter().zip(&divisor.powers) {
        let Some(power) = left.checked_sub(*right) else {
            return Ok(None);
        };
        powers.push(power);
    }
    Ok(Some(Term {
        powers,
        coefficient: dividend.coefficient.div(&divisor.coefficient, budget)?,
    }))
}

/// Exact polynomial division; a nonzero remainder is an ordinary failed divisibility check.
pub(crate) fn exact(
    dividend: &Polynomial,
    divisor: &Polynomial,
    budget: &mut Budget,
) -> Result<Option<Polynomial>, Fault> {
    begin(budget)?;
    if dividend.variables != divisor.variables || dividend.ground != divisor.ground {
        return Err(invalid());
    }
    let leading = divisor.terms.first().ok_or_else(invalid)?;
    let mut remainder = dividend.copy(budget)?;
    let mut result = zero(dividend, budget)?;
    while let Some(current) = remainder.terms.first() {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let Some(term) = quotient_term(current, leading, budget)? else {
            return Ok(None);
        };
        let mut terms = vector(1, budget)?;
        terms.push(term);
        let step = Polynomial::new(dividend.variables, dividend.ground, terms, budget)?;
        remainder = remainder.sub(&step.mul(divisor, budget)?, budget)?;
        result = result.add(&step, budget)?;
    }
    Ok(Some(result))
}

/// Divide only after exact divisibility has been established by the metered loop.
fn quotient(
    dividend: &Polynomial,
    divisor: &Polynomial,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    exact(dividend, divisor, budget)?.ok_or_else(invalid)
}

/// Recursive content is computed in the ring of preceding variables.
fn content(value: &Polynomial, active: usize, budget: &mut Budget) -> Result<Polynomial, Fault> {
    let variable = active.checked_sub(1).ok_or_else(invalid)?;
    let Some(top) = degree(value, variable, budget)? else {
        return zero(value, budget);
    };
    let mut result = zero(value, budget)?;
    for power in 0..=top {
        let entry = coefficient(value, variable, power, budget)?;
        result = recursive(&result, &entry, variable, budget)?;
        if let Some(constant) = result.scalar(budget)?
            && !constant.zero(budget)?
        {
            break;
        }
    }
    Ok(result)
}

/// Strip all recursively computed content without assuming its regional nonvanishing.
fn primitive(value: &Polynomial, active: usize, budget: &mut Budget) -> Result<Polynomial, Fault> {
    if value.zero(budget)? {
        return value.copy(budget);
    }
    quotient(value, &content(value, active, budget)?, budget)
}

/// Pseudo-division avoids inversion of a polynomial in the preceding variables.
fn pseudo_remainder(
    dividend: &Polynomial,
    divisor: &Polynomial,
    variable: usize,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let divisor_degree = degree(divisor, variable, budget)?.ok_or_else(invalid)?;
    let leading = coefficient(divisor, variable, divisor_degree, budget)?;
    let mut remainder = dividend.copy(budget)?;
    while let Some(current_degree) = degree(&remainder, variable, budget)? {
        let Some(distance) = current_degree.checked_sub(divisor_degree) else {
            break;
        };
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let current = coefficient(&remainder, variable, current_degree, budget)?;
        let cancellation = shift(&current.mul(divisor, budget)?, variable, distance, budget)?;
        remainder = leading
            .mul(&remainder, budget)?
            .sub(&cancellation, budget)?;
        if degree(&remainder, variable, budget)?.is_some_and(|next| next >= current_degree) {
            return Err(invalid());
        }
    }
    Ok(remainder)
}

/// The active variable count decreases at each recursive content computation.
fn recursive(
    left: &Polynomial,
    right: &Polynomial,
    active: usize,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    begin(budget)?;
    budget.check(BudgetKind::Depth, size(active, AT)?, AT)?;
    if left.zero(budget)? {
        return right.monic(budget);
    }
    if right.zero(budget)? {
        return left.monic(budget);
    }
    if active == 0 {
        return left.one(budget);
    }
    let common = recursive(
        &content(left, active, budget)?,
        &content(right, active, budget)?,
        active - 1,
        budget,
    )?;
    let mut first = primitive(left, active, budget)?;
    let mut second = primitive(right, active, budget)?;
    while !second.zero(budget)? {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let remainder = primitive(
            &pseudo_remainder(&first, &second, active - 1, budget)?,
            active,
            budget,
        )?;
        first = second;
        second = remainder;
    }
    common.mul(&first.monic(budget)?, budget)?.monic(budget)
}

/// Compute a canonical gcd and independently verify both exact factor reconstructions.
pub(crate) fn gcd(
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    if left.variables != right.variables || left.ground != right.ground {
        return Err(invalid());
    }
    let result = recursive(left, right, left.variables, budget)?;
    if result.zero(budget)? {
        return Ok(result);
    }
    for input in [left, right] {
        let factor = quotient(input, &result, budget)?;
        if !factor.mul(&result, budget)?.equals(input, budget)? {
            return Err(invalid());
        }
    }
    Ok(result)
}
