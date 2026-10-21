// SPDX-License-Identifier: AGPL-3.0-only
//! Extended Euclidean arithmetic, propagating parent splits before child calculations.

use super::dense::Dense;
use crate::budget::{Budget, BudgetKind};
use crate::graph::State;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{
    CoefficientFailure, Element, EvaluationScope, GuardedObligation, UnitDecision,
};
use crate::solving::coefficients::storage::{AT, invalid};

/// A parent obstruction must be resolved before continuing a child Euclidean algorithm.
#[derive(Debug)]
pub(crate) enum Pending {
    /// Every divisor coefficient is zero on the exact evaluation region.
    ZeroDivisor,
    /// An earlier level needs a component split.
    Split(Box<super::SplitCertificate>),
    /// Regional nonvanishing has not been established.
    Guard(GuardedObligation),
}

/// A complete independently replayable polynomial gcd identity.
#[derive(Debug)]
pub(crate) struct Bezout {
    /// Monic gcd of the ordered inputs.
    pub(crate) gcd: Dense,
    /// Multiplier for the first input.
    pub(crate) first: Dense,
    /// Multiplier for the second input.
    pub(crate) second: Dense,
}

/// Convert a parent unit decision without treating a nonzero vector as invertible.
fn inverse(
    engine: &Engine<'_>,
    value: &Element,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Result<Element, Pending>, CoefficientFailure> {
    Ok(match engine.inverse(value, scope, budget)? {
        UnitDecision::Unit { inverse, .. } => Ok(inverse),
        UnitDecision::Split(certificate) => Err(Pending::Split(certificate)),
        UnitDecision::Undetermined(obligation) => Err(Pending::Guard(obligation)),
        UnitDecision::Zero(_) => Err(Pending::ZeroDivisor),
    })
}

/// Decide the effective degree before any cancellation; zero coefficients never supply a reciprocal.
fn leading(
    engine: &Engine<'_>,
    polynomial: &Dense,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Result<(usize, Element), Pending>, CoefficientFailure> {
    for (index, coefficient) in polynomial.coefficients.iter().enumerate().rev() {
        match inverse(engine, coefficient, scope, budget)? {
            Ok(reciprocal) => return Ok(Ok((index + 1, reciprocal))),
            Err(Pending::ZeroDivisor) => {}
            Err(obstruction) => return Ok(Err(obstruction)),
        }
    }
    Ok(Err(Pending::ZeroDivisor))
}

/// A regional inverse may cancel a leading term only on the current cell, so replay that degree fall.
fn cancelled(
    polynomial: &Dense,
    before: usize,
    engine: &Engine<'_>,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Result<Dense, Pending>, CoefficientFailure> {
    use crate::solving::coefficients::model::ZeroDecision;
    let mut count = polynomial.coefficients.len();
    while count >= before {
        let coefficient = polynomial.coefficients.get(count - 1).ok_or_else(invalid)?;
        match engine.zero_test(coefficient, scope, budget)? {
            ZeroDecision::Zero(_) => count -= 1,
            ZeroDecision::Split(certificate) => return Ok(Err(Pending::Split(certificate))),
            ZeroDecision::Undetermined(obligation) => return Ok(Err(Pending::Guard(obligation))),
            ZeroDecision::Nonzero(_) => return Err(invalid().into()),
        }
    }
    Ok(Ok(polynomial
        .shortened(count, engine.state, budget)?
        .ok_or_else(invalid)?))
}

/// Long division charges every leading cancellation and obtains its inverse from the parent.
pub(crate) fn divide(
    dividend: &Dense,
    divisor: &Dense,
    state: &State,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Result<(Dense, Dense), Pending>, CoefficientFailure> {
    if dividend.domain != divisor.domain {
        return Err(invalid().into());
    }
    let parent = Engine {
        state,
        domain: &dividend.domain,
    };
    let (count, reciprocal) = match leading(&parent, divisor, scope, budget)? {
        Ok(value) => value,
        Err(pending) => return Ok(Err(pending)),
    };
    let shortened = divisor.shortened(count, state, budget)?;
    let divisor = shortened.as_ref().unwrap_or(divisor);
    let mut remainder = dividend.copy(state, budget)?;
    let mut quotient = Dense::zero(&parent, budget)?;
    while !remainder.coefficients.is_empty()
        && remainder.coefficients.len() >= divisor.coefficients.len()
    {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let before = remainder.coefficients.len();
        let shift = before
            .checked_sub(divisor.coefficients.len())
            .ok_or_else(invalid)?;
        let scalar = parent.mul(
            remainder.coefficients.last().ok_or_else(invalid)?,
            &reciprocal,
            budget,
        )?;
        let term = Dense::one(&parent, budget)?.scaled(&scalar, shift, state, budget)?;
        quotient = quotient.sum(&term, false, state, budget)?;
        remainder = remainder.sum(
            &divisor.scaled(&scalar, shift, state, budget)?,
            true,
            state,
            budget,
        )?;
        if remainder.coefficients.len() >= before {
            remainder = match cancelled(&remainder, before, &parent, scope, budget)? {
                Ok(value) => value,
                Err(pending) => return Ok(Err(pending)),
            };
        }
    }
    Ok(Ok((quotient, remainder)))
}

/// Extended gcd computes each coefficient in the parent algebra before updating the child remainder.
pub(crate) fn gcd(
    first: &Dense,
    second: &Dense,
    state: &State,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Result<Bezout, Pending>, CoefficientFailure> {
    let parent = Engine {
        state,
        domain: &first.domain,
    };
    if first.domain != second.domain {
        return Err(invalid().into());
    }
    let mut left = first.copy(state, budget)?;
    let mut right = second.copy(state, budget)?;
    let mut first_left = Dense::one(&parent, budget)?;
    let mut first_right = Dense::zero(&parent, budget)?;
    let mut second_left = Dense::zero(&parent, budget)?;
    let mut second_right = Dense::one(&parent, budget)?;
    while !right.coefficients.is_empty() {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let (quotient, remainder) = match divide(&left, &right, state, scope, budget)? {
            Ok(result) => result,
            Err(Pending::ZeroDivisor) => break,
            Err(pending) => return Ok(Err(pending)),
        };
        let next_first = first_left.sum(
            &quotient.mul(&first_right, state, budget)?,
            true,
            state,
            budget,
        )?;
        let next_second = second_left.sum(
            &quotient.mul(&second_right, state, budget)?,
            true,
            state,
            budget,
        )?;
        left = right;
        right = remainder;
        first_left = first_right;
        first_right = next_first;
        second_left = second_right;
        second_right = next_second;
    }
    let (count, reciprocal) = match leading(&parent, &left, scope, budget)? {
        Ok(value) => value,
        Err(Pending::ZeroDivisor) => {
            return Ok(Ok(Bezout {
                gcd: Dense::zero(&parent, budget)?,
                first: first_left,
                second: second_left,
            }));
        }
        Err(pending) => return Ok(Err(pending)),
    };
    let shortened = left.shortened(count, state, budget)?;
    let left = shortened.as_ref().unwrap_or(&left);
    Ok(Ok(Bezout {
        gcd: left.scaled(&reciprocal, 0, state, budget)?,
        first: first_left.scaled(&reciprocal, 0, state, budget)?,
        second: second_left.scaled(&reciprocal, 0, state, budget)?,
    }))
}
