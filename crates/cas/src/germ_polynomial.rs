// SPDX-License-Identifier: AGPL-3.0-only
//! Exact finite Taylor congruence and independently reconstructed remainder identities.

use crate::budget::Budget;
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::number::Rational;
use crate::univariate::Polynomial;

/// Polynomial substitution p(x+centre) by Horner composition, with no infinite series assertion.
pub(crate) fn shift(
    values: &[Rational],
    centre: &Rational,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let affine = Polynomial::new(&[centre.clone(), Rational::one()], budget)?;
    let mut result = Polynomial::zero();
    for value in values.iter().rev() {
        result = result.mul(&affine, budget)?;
        if !value.is_zero() {
            result = result.add(
                &Polynomial::new(std::slice::from_ref(value), budget)?,
                false,
                budget,
            )?;
        }
    }
    Ok(result)
}

/// Reconstruct p(x) from the proposed increment coefficients and remainder in the opposite shift.
pub(crate) fn remainder(
    polynomial: &[Rational],
    centre: &Rational,
    coefficients: &[Rational],
    tail: &[Rational],
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::univariate::validate(tail, budget)?;
    let length = coefficients
        .len()
        .checked_add(tail.len())
        .ok_or_else(invalid)?;
    let mut complete = crate::univariate::zero_coefficients(length, budget)?;
    for (destination, value) in complete.iter_mut().zip(coefficients.iter().chain(tail)) {
        destination.clone_from(value);
    }
    while complete.last().is_some_and(Rational::is_zero) {
        let _removed = complete.pop();
    }
    let negative = centre.negated(budget)?;
    if shift(&complete, &negative, budget)?.coefficients != polynomial {
        return Err(invalid());
    }
    Ok(())
}
