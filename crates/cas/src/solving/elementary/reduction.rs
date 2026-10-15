// SPDX-License-Identifier: AGPL-3.0-only
//! Metered descending pseudo-reduction by proved zero polynomials.

use super::proof::{AT, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::{Form, Term};
use crate::solving::coefficients::model::Value;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::vector;

/// Remove one leading multiple, using only a manifestly nonzero ground leading coefficient.
pub(crate) fn reduce(
    dividend: &Form,
    divisor: &Form,
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Option<Form>, Fault> {
    budget.work(1, AT)?;
    let Some(leading) = dividend.terms.first() else {
        return Ok(None);
    };
    let Some(factor) = divisor.terms.first() else {
        return Ok(None);
    };
    let Value::Scalar(coefficient) = &factor.coefficient.value else {
        return Ok(None);
    };
    if coefficient.zero(budget)? {
        return Err(invalid());
    }
    let mut powers = crate::solving::coefficients::storage::copied(&leading.powers, budget)?;
    for (variable, exponent) in &factor.powers {
        budget.work(crate::graph::size(powers.len(), AT)?, AT)?;
        let Ok(index) = powers.binary_search_by_key(variable, |entry| entry.0) else {
            return Ok(None);
        };
        let power = powers.get_mut(index).ok_or_else(invalid)?;
        let Some(remainder) = power.1.checked_sub(*exponent) else {
            return Ok(None);
        };
        power.1 = remainder;
    }
    powers.retain(|(_, exponent)| *exponent > 0);
    let mut terms = vector(1, budget)?;
    terms.push(Term {
        powers,
        coefficient: engine
            .copy(&leading.coefficient, budget)
            .map_err(|e| fault(&e))?,
    });
    let multiple = Form { terms }
        .mul(divisor, engine, budget)
        .map_err(|e| fault(&e))?;
    let scalar = Form::constant(
        engine
            .copy(&factor.coefficient, budget)
            .map_err(|e| fault(&e))?,
        engine,
        budget,
    )
    .map_err(|e| fault(&e))?;
    let scaled = dividend
        .mul(&scalar, engine, budget)
        .map_err(|e| fault(&e))?;
    Ok(Some(
        scaled
            .add(
                &multiple.neg(engine, budget).map_err(|e| fault(&e))?,
                engine,
                budget,
            )
            .map_err(|e| fault(&e))?,
    ))
}
