// SPDX-License-Identifier: AGPL-3.0-only
//! Extended Euclidean certificate generation and independent divisibility/Bezout checking.

use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::multiplicity::GcdCertificate;
use crate::number::Rational;
use crate::univariate::Polynomial;

/// A remainder represented as a linear combination of the two fixed original inputs.
struct Row {
    /// Current Euclidean remainder.
    remainder: Polynomial,
    /// Left-input multiplier.
    left: Polynomial,
    /// Right-input multiplier.
    right: Polynomial,
}

/// Generate Bezout evidence with a strictly decreasing nonzero remainder degree.
pub(crate) fn generate(
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<GcdCertificate, Fault> {
    left.prepay(budget)?;
    right.prepay(budget)?;
    let mut older = Row {
        remainder: left.clone(),
        left: Polynomial::one(budget)?,
        right: Polynomial::zero(),
    };
    let mut current = Row {
        remainder: right.clone(),
        left: Polynomial::zero(),
        right: Polynomial::one(budget)?,
    };
    while !current.remainder.is_zero() {
        let (quotient, remainder) = older.remainder.divide(&current.remainder, budget)?;
        let left_product = quotient.mul(&current.left, budget)?;
        let right_product = quotient.mul(&current.right, budget)?;
        let next = Row {
            remainder,
            left: older.left.add(&left_product, true, budget)?,
            right: older.right.add(&right_product, true, budget)?,
        };
        older = std::mem::replace(&mut current, next);
    }
    let inverse = Rational::one().div(older.remainder.leading()?, budget)?;
    let divisor = older.remainder.scale(&inverse, budget)?;
    let bezout_left = older.left.scale(&inverse, budget)?;
    let bezout_right = older.right.scale(&inverse, budget)?;
    let left_quotient = left.exact_divide(&divisor, budget)?;
    let right_quotient = right.exact_divide(&divisor, budget)?;
    Ok(GcdCertificate {
        gcd: divisor.coefficients,
        left_quotient: left_quotient.coefficients,
        right_quotient: right_quotient.coefficients,
        bezout_left: bezout_left.coefficients,
        bezout_right: bezout_right.coefficients,
    })
}

/// Verify G divides both inputs and A U+B V=G, without running the producer's Euclidean loop.
pub(crate) fn check(
    left: &Polynomial,
    right: &Polynomial,
    certificate: &GcdCertificate,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let divisor = Polynomial::new(&certificate.gcd, budget)?;
    if divisor.is_zero() || *divisor.leading()? != Rational::one() {
        return Err(invalid());
    }
    let left_quotient = Polynomial::new(&certificate.left_quotient, budget)?;
    let right_quotient = Polynomial::new(&certificate.right_quotient, budget)?;
    require_equal(&divisor.mul(&left_quotient, budget)?, left, budget)?;
    require_equal(&divisor.mul(&right_quotient, budget)?, right, budget)?;
    let multiplier = Polynomial::new(&certificate.bezout_left, budget)?;
    let first = multiplier.mul(left, budget)?;
    let multiplier = Polynomial::new(&certificate.bezout_right, budget)?;
    let second = multiplier.mul(right, budget)?;
    require_equal(&first.add(&second, false, budget)?, &divisor, budget)?;
    Ok(divisor)
}

/// Charge every coefficient comparison before testing a claimed exact polynomial identity.
pub(crate) fn require_equal(
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<(), Fault> {
    left.prepay(budget)?;
    right.prepay(budget)?;
    if left == right {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// A false certificate cannot become a gcd claim.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, Location::new("Euclidean certificate"))
}
