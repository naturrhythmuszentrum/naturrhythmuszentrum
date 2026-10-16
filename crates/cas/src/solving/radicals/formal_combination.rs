// SPDX-License-Identifier: AGPL-3.0-only
//! Degree-generic telescoping certificates for simultaneous elementary substitution.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::State;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::invalid;
use crate::solving::symmetric::algebra::Polynomial;

/// The finite geometric sum is polynomial syntax and requires no division or nonzero premise.
fn divided(
    left: &Polynomial,
    right: &Polynomial,
    exponent: u32,
    state: &State,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let mut result = left.identity(false, state, budget)?;
    for power in 0..exponent {
        let first = left.pow(exponent - power - 1, state, budget)?;
        let second = right.pow(power, state, budget)?;
        result = result.add(&first.mul(&second, state, budget)?, state, budget)?;
    }
    Ok(result)
}

/// One row of the telescoping difference multiplies exactly its own Vieta generator.
pub(crate) fn multiplier(
    expression: &Polynomial,
    basis: &[Polynomial],
    images: &[Polynomial],
    selected: usize,
    state: &State,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let frame = basis.first().ok_or_else(invalid)?;
    let mut result = frame.identity(false, state, budget)?;
    let engine = Engine {
        state,
        domain: &frame.domain,
    };
    for term in &expression.form.terms {
        let selected_slot = u32::try_from(selected).map_err(|_| invalid())?;
        if !term
            .powers
            .iter()
            .any(|(variable, _)| *variable == Variable::Slot(selected_slot))
        {
            continue;
        }
        let coefficient = engine
            .copy(&term.coefficient, budget)
            .map_err(|error| fault(&error))?;
        let mut product =
            Polynomial::constant(&frame.domain, &frame.variables, coefficient, state, budget)?;
        for (variable, exponent) in &term.powers {
            let Variable::Slot(slot) = variable else {
                return Err(invalid());
            };
            let position = usize::try_from(*slot).map_err(|_| invalid())?;
            let original = basis.get(position).ok_or_else(invalid)?;
            let image = images.get(position).ok_or_else(invalid)?;
            let factor = match position.cmp(&selected) {
                std::cmp::Ordering::Equal => divided(original, image, *exponent, state, budget)?,
                std::cmp::Ordering::Less => image.pow(*exponent, state, budget)?,
                std::cmp::Ordering::Greater => original.pow(*exponent, state, budget)?,
            };
            product = product.mul(&factor, state, budget)?;
        }
        result = result.add(&product, state, budget)?;
    }
    Ok(result)
}
