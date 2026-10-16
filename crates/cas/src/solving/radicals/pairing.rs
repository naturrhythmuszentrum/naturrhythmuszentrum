// SPDX-License-Identifier: AGPL-3.0-only
//! Enumerate low-total-degree character products without stored pairing identities.

use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{State, size};
use crate::solving::coefficients::model::Element;
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::orbits::produce::enumerate;
use crate::solving::symmetric::algebra::Polynomial;
use crate::solving::symmetric::cursor::Seed;
use crate::solving::symmetric::reduction::{Reduction, reduce};
use crate::solving::symmetric::seeds::generate;

/// One derived invariant, retaining the exact character exponents used to find it.
#[derive(Debug)]
pub(crate) struct Pairing {
    /// Character indices follow the order of the formal cyclic characters.
    pub(crate) powers: Vec<u32>,
    /// Full symmetric expansion obligation, never an asserted pairing equation.
    pub(crate) reduction: Reduction,
}

/// Advance fixed-total exponent vectors in increasing lexicographic order.
fn advance(powers: &mut [u32], budget: &mut Budget) -> Result<bool, Fault> {
    for slot in (0..powers.len().saturating_sub(1)).rev() {
        budget.work(size(powers.len(), super::AT)?, super::AT)?;
        let suffix = powers.get(slot + 1..).ok_or_else(invalid)?;
        let total = suffix
            .iter()
            .try_fold(0_u32, |sum, power| sum.checked_add(*power))
            .ok_or_else(invalid)?;
        if total > 0 {
            let current = powers.get_mut(slot).ok_or_else(invalid)?;
            *current = current.checked_add(1).ok_or_else(invalid)?;
            powers.get_mut(slot + 1..).ok_or_else(invalid)?.fill(0);
            *powers.last_mut().ok_or_else(invalid)? = total - 1;
            return Ok(true);
        }
    }
    Ok(false)
}

/// Generate every character from the same primitive orientation and multiply only the offered powers.
fn product(
    frame: &Polynomial,
    unity: &Element,
    powers: &[u32],
    state: &State,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let mut result = frame.identity(true, state, budget)?;
    for (slot, power) in powers.iter().enumerate() {
        if *power == 0 {
            continue;
        }
        let seed = Seed::Cyclic {
            index: u32::try_from(slot + 1).map_err(|_| invalid())?,
            power: 1,
        };
        let character = generate(frame, &seed, Some(unity), state, budget)?;
        result = result.mul(&character.pow(*power, state, budget)?, state, budget)?;
    }
    Ok(result)
}

/// Search increasing total degrees, retaining only a cross-character symmetric product.
pub(crate) fn discover(
    frame: &Polynomial,
    unity: &Element,
    maximum: u32,
    state: &State,
    budget: &mut Budget,
) -> Result<Option<Pairing>, Fault> {
    let width = frame.variables.len().checked_sub(1).ok_or_else(invalid)?;
    if width < 2 {
        return Ok(None);
    }
    budget.check(BudgetKind::PolynomialDegree, u64::from(maximum), super::AT)?;
    for total in 2..=maximum {
        let mut powers = vector(width, budget)?;
        powers.resize(width, 0);
        *powers.last_mut().ok_or_else(invalid)? = total;
        loop {
            budget.work(size(width, super::AT)?, super::AT)?;
            if powers.iter().filter(|power| **power != 0).count() > 1 {
                let source = product(frame, unity, &powers, state, budget)?;
                let orbit = enumerate(&source, state, budget)?;
                if orbit.members.len() == 1 {
                    return Ok(Some(Pairing {
                        powers,
                        reduction: reduce(&source, state, budget)?,
                    }));
                }
            }
            if !advance(&mut powers, budget)? {
                break;
            }
        }
    }
    Ok(None)
}
