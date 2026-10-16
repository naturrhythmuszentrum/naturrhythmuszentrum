// SPDX-License-Identifier: AGPL-3.0-only
//! Homogeneous components and strictly decreasing leading-lex symmetric elimination.

use super::AT;
use super::algebra::Polynomial;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{State, size};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::orbits::produce::adjacent;

/// An unsealed identity `F(y) = Q(e_1(y), ..., e_n(y))`.
#[derive(Debug)]
pub(crate) struct Reduction {
    /// The original symmetric polynomial, in formal root slots.
    pub(crate) source: Polynomial,
    /// Computed expression in elementary-symmetric slots, in that same fixed order.
    pub(crate) expression: Polynomial,
    /// Independently constructible elementary symmetric polynomials, excluding `e_0`.
    pub(crate) basis: Vec<Polynomial>,
}

/// Compute elementary symmetric functions by choosing or omitting each root from a factor.
pub(crate) fn elementary(
    frame: &Polynomial,
    state: &State,
    budget: &mut Budget,
) -> Result<Vec<Polynomial>, Fault> {
    let length = frame.variables.len().checked_add(1).ok_or_else(invalid)?;
    let mut result = vector(length, budget)?;
    result.push(frame.identity(true, state, budget)?);
    for _ in 0..frame.variables.len() {
        result.push(frame.identity(false, state, budget)?);
    }
    for slot in 0..frame.variables.len() {
        let variable =
            frame.variable(u32::try_from(slot).map_err(|_| invalid())?, state, budget)?;
        for degree in (1..=slot + 1).rev() {
            let product = result
                .get(degree - 1)
                .ok_or_else(invalid)?
                .mul(&variable, state, budget)?;
            let next = result
                .get(degree)
                .ok_or_else(invalid)?
                .add(&product, state, budget)?;
            *result.get_mut(degree).ok_or_else(invalid)? = next;
        }
    }
    budget.work(size(result.len(), AT)?, AT)?;
    let _constant = result.remove(0);
    Ok(result)
}

/// Full adjacent-transposition invariance is checked before claiming symmetric reduction.
fn invariant(source: &Polynomial, state: &State, budget: &mut Budget) -> Result<(), Fault> {
    for generator in adjacent(source.variables.len(), budget)? {
        if !source.equals(&source.permute(&generator, state, budget)?, state, budget)? {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Graded order separates homogeneous degrees; lex order controls descent inside one degree.
fn key(source: &Polynomial, budget: &mut Budget) -> Result<Option<(u64, Vec<u32>)>, Fault> {
    let Some(term) = source.form.terms.first() else {
        return Ok(None);
    };
    let exponents = source.exponents(term, budget)?;
    let degree = exponents
        .iter()
        .try_fold(0_u64, |sum, exponent| sum.checked_add(u64::from(*exponent)))
        .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
    Ok(Some((degree, exponents)))
}

/// A dominant leading root monomial determines one monomial in elementary symmetric functions.
fn elementary_powers(exponents: &[u32], budget: &mut Budget) -> Result<Vec<u32>, Fault> {
    let mut result = vector(exponents.len(), budget)?;
    for (index, exponent) in exponents.iter().enumerate() {
        result.push(
            exponent
                .checked_sub(exponents.get(index + 1).copied().unwrap_or(0))
                .ok_or_else(invalid)?,
        );
    }
    Ok(result)
}

/// Eliminate one leading monomial and explicitly verify descent before continuing.
fn subtract(
    remainder: &Polynomial,
    basis: &[Polynomial],
    state: &State,
    budget: &mut Budget,
) -> Result<(Polynomial, Polynomial), Fault> {
    budget.solving(BudgetKind::SymmetricSteps, 1, AT)?;
    let previous = key(remainder, budget)?.ok_or_else(invalid)?;
    let powers = elementary_powers(&previous.1, budget)?;
    let coefficient = &remainder
        .form
        .terms
        .first()
        .ok_or_else(invalid)?
        .coefficient;
    let engine = Engine {
        state,
        domain: &remainder.domain,
    };
    let term = remainder.monomial(
        &powers,
        engine
            .copy(coefficient, budget)
            .map_err(|error| fault(&error))?,
        state,
        budget,
    )?;
    let expanded = term.compose(basis, state, budget)?;
    let next = remainder.add(&expanded.neg(state, budget)?, state, budget)?;
    budget.work(size(remainder.variables.len(), AT)?.max(1), AT)?;
    if key(&next, budget)?.is_some_and(|current| current >= previous) {
        return Err(invalid());
    }
    Ok((next, term))
}

/// No degree-specific invariant is supplied: every coefficient comes from leading-lex subtraction.
pub(crate) fn reduce(
    source: &Polynomial,
    state: &State,
    budget: &mut Budget,
) -> Result<Reduction, Fault> {
    reduce_observed(source, state, budget, |_, _, _| Ok(()))
}

/// Generic callbacks observe existing arithmetic boundaries and allocate nothing on the no-op path.
pub(crate) fn reduce_observed(
    source: &Polynomial,
    state: &State,
    budget: &mut Budget,
    mut observe: impl FnMut(&'static str, usize, &mut Budget) -> Result<(), Fault>,
) -> Result<Reduction, Fault> {
    observe("symmetric-invariance-start", 0, budget)?;
    invariant(source, state, budget)?;
    observe("symmetric-invariance-end", 0, budget)?;
    observe("elementary-basis-start", 0, budget)?;
    let basis = elementary(source, state, budget)?;
    observe("elementary-basis-end", 0, budget)?;
    let mut remainder = source.copy(state, budget)?;
    let mut expression = source.identity(false, state, budget)?;
    let mut index = 0;
    while !remainder.form.terms.is_empty() {
        observe("leading-subtraction-start", index, budget)?;
        let (next, term) = subtract(&remainder, &basis, state, budget)?;
        expression = expression.add(&term, state, budget)?;
        remainder = next;
        observe("leading-subtraction-end", index, budget)?;
        index = index.checked_add(1).ok_or_else(invalid)?;
    }
    Ok(Reduction {
        source: source.copy(state, budget)?,
        expression,
        basis,
    })
}

#[cfg(test)]
mod tests {
    use super::elementary_powers;
    use crate::budget::Budget;
    use crate::limits::Limits;

    /// Leading exponents give the unique elementary monomial; a nondominant key is rejected.
    /// ´test:unit:symmetric-leading-lex-powers´
    #[test]
    fn symmetric_leading_lex_powers() -> Result<(), crate::error::Fault> {
        let mut budget = Budget::new(Limits::default())?;
        assert_eq!(elementary_powers(&[5, 3, 3, 1], &mut budget)?, [2, 0, 2, 1]);
        assert!(elementary_powers(&[1, 2], &mut budget).is_err());
        assert_eq!(elementary_powers(&[], &mut budget)?, []);
        Ok(())
    }
}
