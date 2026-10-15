// SPDX-License-Identifier: AGPL-3.0-only
//! Exact cyclotomic construction from the divisor product, independent of any root choice.

use super::dense::Dense;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::size;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{AT, add, invalid, vector};

/// The monic binomial X^n-1, with every coefficient constructed under the same ledger.
fn binomial(engine: &Engine<'_>, order: u32, budget: &mut Budget) -> Result<Dense, Fault> {
    budget.check(BudgetKind::PolynomialDegree, u64::from(order), AT)?;
    let count = usize::try_from(add(u64::from(order), 1)?).map_err(|_| invalid())?;
    let mut coefficients = vector(count, budget)?;
    for _ in 0..count {
        coefficients.push(
            engine
                .constant(false, budget)
                .map_err(|error| fault(&error))?,
        );
    }
    *coefficients.first_mut().ok_or_else(invalid)? = engine
        .neg(
            &engine
                .constant(true, budget)
                .map_err(|error| fault(&error))?,
            budget,
        )
        .map_err(|error| fault(&error))?;
    *coefficients.last_mut().ok_or_else(invalid)? = engine
        .constant(true, budget)
        .map_err(|error| fault(&error))?;
    Dense::new(engine, coefficients, budget).map_err(|error| fault(&error))
}

/// Exact monic division uses no field inversion and rejects any nonzero remainder.
pub(crate) fn quotient(
    dividend: &Dense,
    divisor: &Dense,
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Dense, Fault> {
    if dividend.domain != *engine.domain || divisor.domain != *engine.domain {
        return Err(invalid());
    }
    let leading = divisor.coefficients.last().ok_or_else(invalid)?;
    if !engine
        .formal_equal(
            leading,
            &engine
                .constant(true, budget)
                .map_err(|error| fault(&error))?,
            budget,
        )
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    let mut remainder = dividend
        .copy(engine.state, budget)
        .map_err(|error| fault(&error))?;
    let mut result = Dense::zero(engine, budget).map_err(|error| fault(&error))?;
    while !remainder.coefficients.is_empty()
        && remainder.coefficients.len() >= divisor.coefficients.len()
    {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let before = remainder.coefficients.len();
        let shift = before
            .checked_sub(divisor.coefficients.len())
            .ok_or_else(invalid)?;
        let scalar = engine
            .copy(remainder.coefficients.last().ok_or_else(invalid)?, budget)
            .map_err(|error| fault(&error))?;
        let term = Dense::one(engine, budget)
            .map_err(|error| fault(&error))?
            .scaled(&scalar, shift, engine.state, budget)
            .map_err(|error| fault(&error))?;
        result = result
            .sum(&term, false, engine.state, budget)
            .map_err(|error| fault(&error))?;
        remainder = remainder
            .sum(
                &divisor
                    .scaled(&scalar, shift, engine.state, budget)
                    .map_err(|error| fault(&error))?,
                true,
                engine.state,
                budget,
            )
            .map_err(|error| fault(&error))?;
        if remainder.coefficients.len() >= before {
            return Err(invalid());
        }
    }
    if !remainder.coefficients.is_empty() {
        return Err(invalid());
    }
    Ok(result)
}

/// `Φ_n` is reconstructed from X^n-1 divided by every earlier proper-divisor cyclotomic factor.
pub(crate) fn polynomial(
    engine: &Engine<'_>,
    order: u32,
    budget: &mut Budget,
) -> Result<Dense, Fault> {
    if order == 0 {
        return Err(invalid());
    }
    budget.check(BudgetKind::PolynomialDegree, u64::from(order), AT)?;
    let count = usize::try_from(add(u64::from(order), 1)?).map_err(|_| invalid())?;
    let mut earlier = vector(count, budget)?;
    earlier.push(Dense::one(engine, budget).map_err(|error| fault(&error))?);
    for current in 1..=order {
        let mut value = binomial(engine, current, budget)?;
        for divisor in 1..current {
            budget.work(1, AT)?;
            if current.is_multiple_of(divisor) {
                let index = usize::try_from(divisor).map_err(|_| invalid())?;
                value = quotient(
                    &value,
                    earlier.get(index).ok_or_else(invalid)?,
                    engine,
                    budget,
                )?;
            }
        }
        budget.work(size(value.coefficients.len(), AT)?, AT)?;
        earlier.push(value);
    }
    earlier.pop().ok_or_else(invalid)
}
