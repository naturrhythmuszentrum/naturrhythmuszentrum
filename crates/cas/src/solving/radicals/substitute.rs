// SPDX-License-Identifier: AGPL-3.0-only
//! Vieta substitution from the actual translated coefficients, without stored invariants.

use crate::budget::Budget;
use crate::graph::State;
use crate::solving::SolveReason;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{CoefficientFailure, EvaluationScope, UnitDecision};
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::symmetric::algebra::Polynomial;
use crate::solving::symmetric::resolvent::Resolvent;
use crate::solving::towers::dense::Dense;

/// Coefficient normalization requires the exact regional leading unit before applying Vieta.
pub(crate) fn images(
    source: &Dense,
    frame: &Polynomial,
    scope: &EvaluationScope<'_>,
    state: &State,
    budget: &mut Budget,
) -> Result<Vec<Polynomial>, CoefficientFailure> {
    let parent = Engine {
        state,
        domain: &source.domain,
    };
    let engine = Engine {
        state,
        domain: &frame.domain,
    };
    let leading = source.coefficients.last().ok_or_else(invalid)?;
    let reciprocal = match parent.inverse(leading, scope, budget)? {
        UnitDecision::Unit { inverse, .. } => inverse,
        UnitDecision::Zero(_) => return Err(invalid().into()),
        UnitDecision::Split(_) => {
            return Err(CoefficientFailure::Unsupported(
                SolveReason::ComponentChoicePending,
            ));
        }
        UnitDecision::Undetermined(_) => {
            return Err(CoefficientFailure::Unsupported(
                SolveReason::MissingUnitProof,
            ));
        }
    };
    let degree = source
        .coefficients
        .len()
        .checked_sub(1)
        .ok_or_else(invalid)?;
    if degree != frame.variables.len() {
        return Err(invalid().into());
    }
    let mut result = vector(degree, budget)?;
    for index in 1..=degree {
        let coefficient = source
            .coefficients
            .get(degree - index)
            .ok_or_else(invalid)?;
        let normalized = parent.mul(coefficient, &reciprocal, budget)?;
        let signed = if index.is_multiple_of(2) {
            normalized
        } else {
            parent.neg(&normalized, budget)?
        };
        let value = engine.include_parent(&signed, budget)?;
        result.push(Polynomial::constant(
            &frame.domain,
            &[],
            value,
            state,
            budget,
        )?);
    }
    Ok(result)
}

/// Substitute into every computed resolvent coefficient and retain the full monic polynomial.
pub(crate) fn resolvent(
    source: &Resolvent,
    images: &[Polynomial],
    state: &State,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let frame = images.first().ok_or_else(invalid)?;
    let engine = Engine {
        state,
        domain: &frame.domain,
    };
    let mut coefficients = vector(source.coefficients.len(), budget)?;
    for reduction in &source.coefficients {
        let evaluated = reduction.expression.compose(images, state, budget)?;
        let coefficient = match evaluated.form.terms.as_slice() {
            [] => engine.constant(false, budget)?,
            [term] if term.powers.is_empty() => engine.copy(&term.coefficient, budget)?,
            _ => return Err(invalid().into()),
        };
        coefficients.push(coefficient);
    }
    Dense::new(&engine, coefficients, budget)
}
