// SPDX-License-Identifier: AGPL-3.0-only
//! Import graph-owned ground coefficients into bounded arithmetic scratch.

use super::fraction::Fraction;
use super::polynomial::{Polynomial, Term};
use super::scalar::{Ground, Scalar, rational_copy};
use super::storage::{begin, copied, domain, invalid, polynomial, value, vector};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Handle, State};
use crate::solving::numeric::Gaussian;
use crate::solving::{CoefficientDomainData, CoefficientValueData};

/// Identify a ground field, refusing any implicit fraction field of a quotient.
pub(crate) fn ground(state: &State, handle: &Handle) -> Result<Ground, Fault> {
    match domain(state, handle)? {
        CoefficientDomainData::Rational => Ok(Ground::Rational),
        CoefficientDomainData::Gaussian => Ok(Ground::Gaussian),
        _ => Err(invalid()),
    }
}

/// Copy one ground coefficient using the numeric leaf's fallible adapters.
pub(crate) fn scalar(state: &State, handle: &Handle, budget: &mut Budget) -> Result<Scalar, Fault> {
    begin(budget)?;
    let stored = value(state, handle)?;
    match (&stored.value, ground(state, &stored.domain)?) {
        (CoefficientValueData::Rational { value }, Ground::Rational) => {
            Ok(Scalar::Rational(rational_copy(value, budget)?))
        }
        (CoefficientValueData::Gaussian { real, imaginary }, Ground::Gaussian) => Ok(
            Scalar::Gaussian(Gaussian::from_parts(real, imaginary, budget)?),
        ),
        _ => Err(invalid()),
    }
}

/// Reconstruct the fixed-width canonical ground polynomial under the active budget.
pub(crate) fn sparse(
    state: &State,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    begin(budget)?;
    let value = polynomial(state, handle)?;
    let field = ground(state, &value.domain)?;
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        terms.push(Term {
            powers: copied(&term.exponents, budget)?,
            coefficient: scalar(state, &term.coefficient, budget)?,
        });
    }
    Polynomial::new(value.variables.len(), field, terms, budget)
}

/// Admission independently normalizes an untrusted pair and compares both canonical endpoints.
pub(crate) fn canonical_fraction(
    state: &State,
    numerator: &Handle,
    denominator: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let top = sparse(state, numerator, budget)?;
    let bottom = sparse(state, denominator, budget)?;
    let normalized = Fraction::new(&top, &bottom, budget)?;
    if !top.equals(&normalized.numerator, budget)?
        || !bottom.equals(&normalized.denominator, budget)?
    {
        return Err(invalid());
    }
    Ok(())
}
