// SPDX-License-Identifier: AGPL-3.0-only
//! Character sums and sums of partition block products in the original formal root frame.

use super::AT;
use super::algebra::Polynomial;
use super::cursor::Seed;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::State;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::Element;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{invalid, vector};

/// A cyclic sum is computed from successive powers, independently of its eventual orbit size.
fn cyclic(
    frame: &Polynomial,
    unity: &Element,
    index: u32,
    power: u32,
    state: &State,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    if index == 0
        || usize::try_from(index).map_err(|_| invalid())? >= frame.variables.len()
        || power == 0
    {
        return Err(invalid());
    }
    let degree = u32::try_from(frame.variables.len()).map_err(|_| invalid())?;
    if power > super::cursor::order(degree, index, budget)? {
        return Err(invalid());
    }
    let engine = Engine {
        state,
        domain: &frame.domain,
    };
    let multiplier = engine
        .pow(unity, index, budget)
        .map_err(|error| fault(&error))?;
    let mut coefficient = engine
        .constant(true, budget)
        .map_err(|error| fault(&error))?;
    let mut result = frame.identity(false, state, budget)?;
    for slot in 0..frame.variables.len() {
        let constant = Polynomial::constant(
            &frame.domain,
            &frame.variables,
            engine
                .copy(&coefficient, budget)
                .map_err(|error| fault(&error))?,
            state,
            budget,
        )?;
        let variable =
            frame.variable(u32::try_from(slot).map_err(|_| invalid())?, state, budget)?;
        result = result.add(&constant.mul(&variable, state, budget)?, state, budget)?;
        coefficient = engine
            .mul(&coefficient, &multiplier, budget)
            .map_err(|error| fault(&error))?;
    }
    result.pow(power, state, budget)
}

/// Validate the canonical partition and multiply its blocks without a stored quartic recipe.
fn partition(
    frame: &Polynomial,
    blocks: &[u32],
    state: &State,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    if blocks.len() != frame.variables.len() || blocks.first() != Some(&0) {
        return Err(invalid());
    }
    let mut products = vector(blocks.len(), budget)?;
    products.push(frame.identity(true, state, budget)?);
    let mut maximum = 0_u32;
    for (slot, block) in blocks.iter().enumerate() {
        budget.work(1, AT)?;
        if *block > maximum.checked_add(1).ok_or_else(invalid)? {
            return Err(invalid());
        }
        if *block > maximum {
            products.push(frame.identity(true, state, budget)?);
            maximum = *block;
        }
        let product = products
            .get_mut(usize::try_from(*block).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        *product = product.mul(
            &frame.variable(u32::try_from(slot).map_err(|_| invalid())?, state, budget)?,
            state,
            budget,
        )?;
    }
    let mut result = frame.identity(false, state, budget)?;
    for product in products {
        result = result.add(&product, state, budget)?;
    }
    Ok(result)
}

/// No identity is trusted here: the generated polynomial is an unsealed orbit candidate.
pub(crate) fn generate(
    frame: &Polynomial,
    seed: &Seed,
    unity: Option<&Element>,
    state: &State,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    budget.check(
        BudgetKind::PolynomialVariables,
        crate::graph::size(frame.variables.len(), AT)?,
        AT,
    )?;
    match seed {
        Seed::Cyclic { index, power } => cyclic(
            frame,
            unity.ok_or_else(invalid)?,
            *index,
            *power,
            state,
            budget,
        ),
        Seed::Partition(blocks) => partition(frame, blocks, state, budget),
    }
}
