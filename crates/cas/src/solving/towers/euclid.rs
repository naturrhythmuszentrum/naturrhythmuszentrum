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
        UnitDecision::Zero(_) => return Err(invalid().into()),
    })
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
    let leading = divisor.coefficients.last().ok_or_else(invalid)?;
    let reciprocal = match inverse(&parent, leading, scope, budget)? {
        Ok(value) => value,
        Err(pending) => return Ok(Err(pending)),
    };
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
            return Err(invalid().into());
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
    let reciprocal = match inverse(
        &parent,
        left.coefficients.last().ok_or_else(invalid)?,
        scope,
        budget,
    )? {
        Ok(value) => value,
        Err(pending) => return Ok(Err(pending)),
    };
    Ok(Ok(Bezout {
        gcd: left.scaled(&reciprocal, 0, state, budget)?,
        first: first_left.scaled(&reciprocal, 0, state, budget)?,
        second: second_left.scaled(&reciprocal, 0, state, budget)?,
    }))
}
