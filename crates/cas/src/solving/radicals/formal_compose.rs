// SPDX-License-Identifier: AGPL-3.0-only
//! Observable generic composition skips zero monomials before evaluating their other factors.

use super::audit::Audit;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::State;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::invalid;
use crate::solving::symmetric::algebra::Polynomial;

/// Every product remains ordinary independently bounded coefficient arithmetic.
fn multiply(
    left: &Polynomial,
    right: &Polynomial,
    state: &State,
    audit: &mut Audit,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    audit.observe("compose-multiply-start", &[left, right], budget)?;
    audit.describe("compose-multiply-left", left, budget)?;
    audit.describe("compose-multiply-right", right, budget)?;
    let result = left.mul(right, state, budget)?;
    audit.observe("compose-multiply-end", &[&result], budget)?;
    Ok(result)
}

/// Repeated squaring exposes each actual operand before canonical fraction arithmetic runs.
fn power(
    image: &Polynomial,
    exponent: u32,
    state: &State,
    audit: &mut Audit,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    budget.check(BudgetKind::IntegerExponent, u64::from(exponent), super::AT)?;
    let mut result = image.identity(true, state, budget)?;
    let mut factor = image.copy(state, budget)?;
    let mut remaining = exponent;
    while remaining != 0 {
        if remaining & 1 == 1 {
            result = multiply(&result, &factor, state, audit, budget)?;
        }
        remaining >>= 1;
        if remaining != 0 {
            factor = multiply(&factor, &factor, state, audit, budget)?;
        }
    }
    Ok(result)
}

/// A zero substitution annihilates its whole monomial before any unrelated power is evaluated.
fn vanished(
    term: &crate::solving::coefficients::forms::Term,
    images: &[Polynomial],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    for (variable, exponent) in &term.powers {
        budget.work(1, super::AT)?;
        let Variable::Slot(slot) = variable else {
            return Err(invalid());
        };
        let image = images
            .get(usize::try_from(*slot).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        if *exponent != 0 && image.form.terms.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Exact ordered substitution retains all source coefficients and exposes each surviving monomial.
pub(crate) fn compose(
    expression: &Polynomial,
    images: &[Polynomial],
    state: &State,
    audit: &mut Audit,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    if expression.variables.len() != images.len() {
        return Err(invalid());
    }
    let frame = images.first().unwrap_or(expression);
    for image in images {
        budget.work(
            crate::graph::size(image.variables.len(), super::AT)?.max(1),
            super::AT,
        )?;
        if (&image.domain, &image.variables) != (&expression.domain, &frame.variables) {
            return Err(invalid());
        }
    }
    let engine = Engine {
        state,
        domain: &expression.domain,
    };
    audit.describe("compose-expression", expression, budget)?;
    let mut result = frame.identity(false, state, budget)?;
    for term in &expression.form.terms {
        if vanished(term, images, budget)? {
            continue;
        }
        let coefficient = engine
            .copy(&term.coefficient, budget)
            .map_err(|error| fault(&error))?;
        let mut product = Polynomial::constant(
            &expression.domain,
            &frame.variables,
            coefficient,
            state,
            budget,
        )?;
        for (variable, exponent) in &term.powers {
            let Variable::Slot(slot) = variable else {
                return Err(invalid());
            };
            let image = images
                .get(usize::try_from(*slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            let factor = power(image, *exponent, state, audit, budget)?;
            product = multiply(&product, &factor, state, audit, budget)?;
        }
        result = result.add(&product, state, budget)?;
    }
    Ok(result)
}
