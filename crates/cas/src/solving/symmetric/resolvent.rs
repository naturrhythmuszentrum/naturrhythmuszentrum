// SPDX-License-Identifier: AGPL-3.0-only
//! Orbit products and strictly smaller recursive targets, without a formula cache.

use super::AT;
use super::algebra::Polynomial;
use super::reduction::{Reduction, reduce};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{State, size};
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::orbits::produce::{Orbit, adjacent};

/// Coefficients in ascending powers of the new resolvent variable, including the monic leading one.
#[derive(Debug)]
pub(crate) struct Resolvent {
    /// Every coefficient has a computed symmetric expression and expansion obligation.
    pub(crate) coefficients: Vec<Reduction>,
    /// Strictly increased recursion depth, after checking both degree and depth ceilings.
    pub(crate) depth: u64,
}

/// Multiply one factor (z - member), retaining repeated specialized factors for later substitution.
fn factor(
    coefficients: &[Polynomial],
    member: &Polynomial,
    state: &State,
    budget: &mut Budget,
) -> Result<Vec<Polynomial>, Fault> {
    let length = coefficients.len().checked_add(1).ok_or_else(invalid)?;
    budget.check(BudgetKind::PolynomialDegree, size(length - 1, AT)?, AT)?;
    let mut result = vector(length, budget)?;
    for _ in 0..length {
        result.push(member.identity(false, state, budget)?);
    }
    for (degree, coefficient) in coefficients.iter().enumerate() {
        let product = coefficient.mul(member, state, budget)?.neg(state, budget)?;
        let lower = result
            .get(degree)
            .ok_or_else(invalid)?
            .add(&product, state, budget)?;
        *result.get_mut(degree).ok_or_else(invalid)? = lower;
        let upper = result
            .get(degree + 1)
            .ok_or_else(invalid)?
            .add(coefficient, state, budget)?;
        *result.get_mut(degree + 1).ok_or_else(invalid)? = upper;
    }
    Ok(result)
}

/// Equal/larger orbits are skipped before any resolvent coefficient is expanded.
pub(crate) fn smaller(
    orbit: &Orbit,
    original_degree: u32,
    depth: u64,
    state: &State,
    budget: &mut Budget,
) -> Result<Option<Resolvent>, Fault> {
    let seed = orbit.members.first().ok_or_else(invalid)?;
    let degree = size(orbit.members.len(), AT)?;
    if u64::from(original_degree) != size(seed.variables.len(), AT)? {
        return Err(invalid());
    }
    if degree >= u64::from(original_degree) {
        return Ok(None);
    }
    budget.solving(BudgetKind::ResolventDegree, degree, AT)?;
    let next_depth = depth
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::ResolventDepth, AT))?;
    budget.solving(BudgetKind::ResolventDepth, next_depth, AT)?;
    if orbit.generators != adjacent(seed.variables.len(), budget)? {
        return Err(invalid());
    }
    let mut coefficients = vector(1, budget)?;
    coefficients.push(seed.identity(true, state, budget)?);
    for member in &orbit.members {
        coefficients = factor(&coefficients, member, state, budget)?;
    }
    let mut reduced = vector(coefficients.len(), budget)?;
    for coefficient in coefficients {
        reduced.push(reduce(&coefficient, state, budget)?);
    }
    Ok(Some(Resolvent {
        coefficients: reduced,
        depth: next_depth,
    }))
}
