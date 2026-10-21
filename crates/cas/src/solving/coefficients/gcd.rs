// SPDX-License-Identifier: AGPL-3.0-only
//! Recursive content/primitive-part polynomial gcd, with exact quotient verification.
//!
//! Multivariate coprimality is never replaced by a Bezout-to-one assertion.

use super::polynomial::{Polynomial, Term};
use super::scalar::Scalar;
use super::storage::{AT, begin, invalid, vector};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::size;
use crate::number::Rational;
use crate::solving::numeric::Gaussian;

/// An explicit zero in the same polynomial ring.
fn zero(value: &Polynomial, budget: &mut Budget) -> Result<Polynomial, Fault> {
    Polynomial::constant(
        value.variables,
        Scalar::constant(value.ground, false, budget)?,
        budget,
    )
}

/// Degree in one fixed variable; zero has no degree.
pub(crate) fn degree(
    value: &Polynomial,
    variable: usize,
    budget: &mut Budget,
) -> Result<Option<u32>, Fault> {
    begin(budget)?;
    budget.work(size(value.terms.len(), AT)?, AT)?;
    let mut result = None;
    for term in &value.terms {
        let power = *term.powers.get(variable).ok_or_else(invalid)?;
        result = Some(result.map_or(power, |earlier: u32| earlier.max(power)));
    }
    Ok(result)
}

/// Extract a coefficient in the last active variable, keeping the original frame width.
fn coefficient(
    value: &Polynomial,
    variable: usize,
    power: u32,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    begin(budget)?;
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        budget.work(1, AT)?;
        if term.powers.get(variable) == Some(&power) {
            let mut entry = term.copy(budget)?;
            *entry.powers.get_mut(variable).ok_or_else(invalid)? = 0;
            terms.push(entry);
        }
    }
    Polynomial::new(value.variables, value.ground, terms, budget)
}

/// Multiply by a formal variable power with checked exponent arithmetic.
fn shift(
    value: &Polynomial,
    variable: usize,
    power: u32,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        let mut entry = term.copy(budget)?;
        let slot = entry.powers.get_mut(variable).ok_or_else(invalid)?;
        *slot = slot
            .checked_add(power)
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        terms.push(entry);
    }
    Polynomial::new(value.variables, value.ground, terms, budget)
}

/// One multivariate long-division leading monomial, if divisibility is possible.
pub(crate) fn quotient_term(
    dividend: &Term,
    divisor: &Term,
    budget: &mut Budget,
) -> Result<Option<Term>, Fault> {
    let mut powers = vector(dividend.powers.len(), budget)?;
    for (left, right) in dividend.powers.iter().zip(&divisor.powers) {
        let Some(power) = left.checked_sub(*right) else {
            return Ok(None);
        };
        powers.push(power);
    }
    Ok(Some(Term {
        powers,
        coefficient: dividend.coefficient.div(&divisor.coefficient, budget)?,
    }))
}

/// A single monomial divides independently term by term, without rebuilding every intermediate remainder.
fn monomial_quotient(
    dividend: &Polynomial,
    divisor: &Term,
    budget: &mut Budget,
) -> Result<Option<Polynomial>, Fault> {
    if let Some(maximum) = dividend.total_degree(budget)? {
        budget.check(BudgetKind::PolynomialDegree, maximum, AT)?;
    }
    let mut terms = vector(dividend.terms.len(), budget)?;
    for current in &dividend.terms {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let Some(term) = quotient_term(current, divisor, budget)? else {
            return Ok(None);
        };
        terms.push(term);
    }
    Polynomial::new(dividend.variables, dividend.ground, terms, budget).map(Some)
}

/// Exact polynomial division; a nonzero remainder is an ordinary failed divisibility check.
pub(crate) fn exact(
    dividend: &Polynomial,
    divisor: &Polynomial,
    budget: &mut Budget,
) -> Result<Option<Polynomial>, Fault> {
    begin(budget)?;
    if dividend.variables != divisor.variables || dividend.ground != divisor.ground {
        return Err(invalid());
    }
    if dividend.variables > 0
        && let [single] = divisor.terms.as_slice()
    {
        return monomial_quotient(dividend, single, budget);
    }
    let leading = divisor.terms.first().ok_or_else(invalid)?;
    let mut remainder = dividend.copy(budget)?;
    let mut result = zero(dividend, budget)?;
    while let Some(current) = remainder.terms.first() {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let Some(term) = quotient_term(current, leading, budget)? else {
            return Ok(None);
        };
        let mut terms = vector(1, budget)?;
        terms.push(term);
        let step = Polynomial::new(dividend.variables, dividend.ground, terms, budget)?;
        remainder = remainder.sub(&step.mul(divisor, budget)?, budget)?;
        result = result.add(&step, budget)?;
    }
    Ok(Some(result))
}

/// Multivariate division records every cancellation; the remainder differs by a divisor multiple.
pub(crate) fn division(
    dividend: &Polynomial,
    divisor: &Polynomial,
    budget: &mut Budget,
) -> Result<(Polynomial, Polynomial), Fault> {
    begin(budget)?;
    if dividend.variables != divisor.variables || dividend.ground != divisor.ground {
        return Err(invalid());
    }
    let leading = divisor.terms.first().ok_or_else(invalid)?;
    let mut pending = dividend.copy(budget)?;
    let mut result = zero(dividend, budget)?;
    let mut quotient = zero(dividend, budget)?;
    while let Some(current) = pending.terms.first() {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let step = if let Some(term) = quotient_term(current, leading, budget)? {
            let mut terms = vector(1, budget)?;
            terms.push(term);
            let multiple = Polynomial::new(dividend.variables, dividend.ground, terms, budget)?;
            quotient = quotient.add(&multiple, budget)?;
            multiple.mul(divisor, budget)?
        } else {
            let mut terms = vector(1, budget)?;
            terms.push(current.copy(budget)?);
            let term = Polynomial::new(dividend.variables, dividend.ground, terms, budget)?;
            result = result.add(&term, budget)?;
            term
        };
        pending = pending.sub(&step, budget)?;
    }
    Ok((quotient, result))
}

/// Divide only after exact divisibility has been established by the metered loop.
fn quotient(
    dividend: &Polynomial,
    divisor: &Polynomial,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    exact(dividend, divisor, budget)?.ok_or_else(invalid)
}

/// Recursive content is computed in the ring of preceding variables.
fn content(value: &Polynomial, active: usize, budget: &mut Budget) -> Result<Polynomial, Fault> {
    let variable = active.checked_sub(1).ok_or_else(invalid)?;
    let Some(top) = degree(value, variable, budget)? else {
        return zero(value, budget);
    };
    let mut result = zero(value, budget)?;
    for power in 0..=top {
        let entry = coefficient(value, variable, power, budget)?;
        result = recursive(&result, &entry, variable, budget)?;
        if let Some(constant) = result.scalar(budget)?
            && !constant.zero(budget)?
        {
            break;
        }
    }
    Ok(result)
}

/// Clear all ground denominators and integer content together, without monic rescaling.
/// For Gaussian coefficients the rational content includes both exact components.
fn integral(value: &Polynomial, budget: &mut Budget) -> Result<Polynomial, Fault> {
    if value.zero(budget)? {
        return value.copy(budget);
    }
    let mut content = Rational::from_ratio(0, 1, budget)?;
    for term in &value.terms {
        begin(budget)?;
        content = match &term.coefficient {
            Scalar::Rational(part) => content.content_gcd(part, budget)?,
            Scalar::Gaussian(part) => {
                for component in [part.real(budget)?, part.imaginary(budget)?] {
                    if !component.is_zero() {
                        content = content.content_gcd(&component, budget)?;
                    }
                }
                content
            }
        };
    }
    if content.is_zero() {
        return Err(invalid());
    }
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        terms.push(Term {
            powers: super::storage::copied(&term.powers, budget)?,
            coefficient: integral_coefficient(&term.coefficient, &content, budget)?,
        });
    }
    Polynomial::new(value.variables, value.ground, terms, budget)
}

/// Divide Gaussian components separately by a positive rational, avoiding an unnecessary norm square.
fn integral_coefficient(
    value: &Scalar,
    content: &Rational,
    budget: &mut Budget,
) -> Result<Scalar, Fault> {
    begin(budget)?;
    Ok(match value {
        Scalar::Rational(part) => Scalar::Rational(part.div(content, budget)?),
        Scalar::Gaussian(part) => Scalar::Gaussian(Gaussian::from_parts(
            &part.real(budget)?.div(content, budget)?,
            &part.imaginary(budget)?.div(content, budget)?,
            budget,
        )?),
    })
}

/// Strip all recursively computed content without assuming its regional nonvanishing.
fn primitive(value: &Polynomial, active: usize, budget: &mut Budget) -> Result<Polynomial, Fault> {
    if value.zero(budget)? {
        return value.copy(budget);
    }
    integral(
        &quotient(value, &content(value, active, budget)?, budget)?,
        budget,
    )
}

/// Pseudo-division strips content after each cancellation, preserving the primitive gcd over the coefficient field.
fn pseudo_remainder(
    dividend: &Polynomial,
    divisor: &Polynomial,
    variable: usize,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let divisor_degree = degree(divisor, variable, budget)?.ok_or_else(invalid)?;
    let leading = coefficient(divisor, variable, divisor_degree, budget)?;
    let mut remainder = integral(dividend, budget)?;
    while let Some(current_degree) = degree(&remainder, variable, budget)? {
        let Some(distance) = current_degree.checked_sub(divisor_degree) else {
            break;
        };
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let current = coefficient(&remainder, variable, current_degree, budget)?;
        let cancellation = shift(&current.mul(divisor, budget)?, variable, distance, budget)?;
        let reduced = leading
            .mul(&remainder, budget)?
            .sub(&cancellation, budget)?;
        // Primitive rescaling in the coefficient fraction field preserves the primitive gcd.
        remainder = primitive(
            &reduced,
            variable.checked_add(1).ok_or_else(invalid)?,
            budget,
        )?;
        if degree(&remainder, variable, budget)?.is_some_and(|next| next >= current_degree) {
            return Err(invalid());
        }
    }
    Ok(remainder)
}

/// The active variable count decreases at each recursive content computation.
fn recursive(
    left: &Polynomial,
    right: &Polynomial,
    active: usize,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    begin(budget)?;
    budget.check(BudgetKind::Depth, size(active, AT)?, AT)?;
    if left.zero(budget)? {
        return integral(right, budget);
    }
    if right.zero(budget)? {
        return integral(left, budget);
    }
    if active == 0 {
        return left.one(budget);
    }
    if let Some(result) = monomial(left, right, budget)? {
        return Ok(result);
    }
    let common = recursive(
        &content(left, active, budget)?,
        &content(right, active, budget)?,
        active - 1,
        budget,
    )?;
    let mut first = primitive(left, active, budget)?;
    let mut second = primitive(right, active, budget)?;
    while !second.zero(budget)? {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        let remainder = pseudo_remainder(&first, &second, active - 1, budget)?;
        first = second;
        second = remainder;
    }
    integral(&common.mul(&first, budget)?, budget)
}

/// A monomial's only nonunit factors are coordinate variables, so its gcd has exact exponent minima.
fn monomial(
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<Option<Polynomial>, Fault> {
    let (single, other) = match (left.terms.as_slice(), right.terms.as_slice()) {
        ([term], _) => (term, right),
        (_, [term]) => (term, left),
        _ => return Ok(None),
    };
    let mut powers = super::storage::copied(&single.powers, budget)?;
    for term in &other.terms {
        for (minimum, exponent) in powers.iter_mut().zip(&term.powers) {
            budget.work(1, AT)?;
            *minimum = (*minimum).min(*exponent);
        }
    }
    let mut terms = vector(1, budget)?;
    terms.push(Term {
        powers,
        coefficient: Scalar::constant(left.ground, true, budget)?,
    });
    Ok(Some(Polynomial::new(
        left.variables,
        left.ground,
        terms,
        budget,
    )?))
}

/// If one operand omits a variable, its gcd with the other is the gcd with all coefficients in that variable.
fn coefficient_gcd(
    fixed: &Polynomial,
    varying: &Polynomial,
    variable: usize,
    maximum: u32,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let mut result = fixed.monic(budget)?;
    for power in (0..=maximum).rev() {
        let entry = coefficient(varying, variable, power, budget)?;
        if entry.zero(budget)? {
            continue;
        }
        result = gcd(&result, &entry, budget)?;
        if let Some(scalar) = result.scalar(budget)?
            && !scalar.zero(budget)?
        {
            break;
        }
    }
    Ok(result)
}

/// Every nested coefficient gcd omits another variable; the original ordered exponent frame is unchanged.
fn independent(
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<Option<Polynomial>, Fault> {
    if left.zero(budget)? || right.zero(budget)? {
        return Ok(None);
    }
    for variable in 0..left.variables {
        let first = degree(left, variable, budget)?.ok_or_else(invalid)?;
        let second = degree(right, variable, budget)?.ok_or_else(invalid)?;
        if first == 0 && second > 0 {
            return coefficient_gcd(left, right, variable, second, budget).map(Some);
        }
        if second == 0 && first > 0 {
            return coefficient_gcd(right, left, variable, first, budget).map(Some);
        }
    }
    Ok(None)
}

/// Use the smallest available main degree first; all coordinate permutations stay internal to the gcd.
fn elimination_order(
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<Vec<usize>, Fault> {
    let mut ranked = vector(left.variables, budget)?;
    for variable in 0..left.variables {
        let maximum = degree(left, variable, budget)?
            .unwrap_or(0)
            .max(degree(right, variable, budget)?.unwrap_or(0));
        ranked.push((maximum, variable));
        let mut position = ranked.len() - 1;
        while position > 0 {
            budget.work(1, AT)?;
            if ranked.get(position - 1) >= ranked.get(position) {
                break;
            }
            ranked.swap(position - 1, position);
            position -= 1;
        }
    }
    let mut order = vector(left.variables, budget)?;
    for (_, variable) in ranked {
        order.push(variable);
    }
    Ok(order)
}

/// Reindex scratch exponents bijectively, then reestablish canonical monomial order.
fn permuted(value: &Polynomial, order: &[usize], budget: &mut Budget) -> Result<Polynomial, Fault> {
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        let mut powers = vector(order.len(), budget)?;
        for variable in order {
            budget.work(1, AT)?;
            powers.push(*term.powers.get(*variable).ok_or_else(invalid)?);
        }
        terms.push(Term {
            powers,
            coefficient: term.coefficient.copy(budget)?,
        });
    }
    Polynomial::new(value.variables, value.ground, terms, budget)
}

/// Restore the original coordinate frame before canonicalization and both factor reconstructions.
fn ordered_recursive(
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let order = elimination_order(left, right, budget)?;
    let mut inverse = vector(order.len(), budget)?;
    inverse.resize(order.len(), 0);
    for (position, variable) in order.iter().enumerate() {
        budget.work(1, AT)?;
        *inverse.get_mut(*variable).ok_or_else(invalid)? = position;
    }
    let result = recursive(
        &permuted(left, &order, budget)?,
        &permuted(right, &order, budget)?,
        left.variables,
        budget,
    )?;
    permuted(&result, &inverse, budget)?.monic(budget)
}

/// Compute a canonical gcd and independently verify both exact factor reconstructions.
pub(crate) fn gcd(
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    if left.variables != right.variables || left.ground != right.ground {
        return Err(invalid());
    }
    let result = if let Some(result) = monomial(left, right, budget)? {
        result
    } else if let Some(result) = independent(left, right, budget)? {
        result
    } else {
        ordered_recursive(left, right, budget)?
    };
    if result.zero(budget)? {
        return Ok(result);
    }
    for input in [left, right] {
        let factor = quotient(input, &result, budget)?;
        if !factor.mul(&result, budget)?.equals(input, budget)? {
            return Err(invalid());
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{Polynomial, Scalar, Term, gcd};
    use crate::budget::Budget;
    use crate::number::Rational;
    use crate::solving::coefficients::scalar::Ground;

    fn scaled(
        powers: &[u32],
        denominator: u64,
        budget: &mut Budget,
    ) -> Result<Polynomial, crate::error::Fault> {
        let mut terms = Vec::new();
        for power in powers {
            terms.push(Term {
                powers: vec![*power],
                coefficient: Scalar::Rational(Rational::from_ratio(1, denominator, budget)?),
            });
        }
        Polynomial::new(1, Ground::Rational, terms, budget)
    }

    /// Ground rescaling leaves the gcd ideal unchanged without squaring large rational denominators.
    /// ´test:unit:primitive-remainder-ground-scaling´
    #[test]
    fn primitive_remainder_ground_scaling() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let left = scaled(&[2, 0], 1_200_757_082_375_992_968, &mut budget)?;
        let right = scaled(&[1, 0], 61_004_779_879_896, &mut budget)?;
        let common = gcd(&left, &right, &mut budget)?;
        assert!(common.equals(&left.one(&mut budget)?, &mut budget)?);
        Ok(())
    }
    /// Intermediate content removal keeps a two-variable gcd below degree 64 instead of expanding degree 90.
    /// ´test:unit:primitive-remainder-content-bound´
    #[test]
    fn primitive_remainder_content_bound() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let polynomial = |terms: &[([u32; 2], i64)], ledger: &mut Budget| {
            let mut entries = Vec::new();
            for (powers, value) in terms {
                entries.push(Term {
                    powers: powers.to_vec(),
                    coefficient: Scalar::Rational(Rational::from_ratio(*value, 1, ledger)?),
                });
            }
            Polynomial::new(2, Ground::Rational, entries, ledger)
        };
        // P=y²+y+x³⁰-1 and Q=x³⁰y+1 are each primitive in y.
        let first = polynomial(
            &[([0, 2], 1), ([0, 1], 1), ([30, 0], 1), ([0, 0], -1)],
            &mut budget,
        )?;
        let second = polynomial(&[([30, 1], 1), ([0, 0], 1)], &mut budget)?;
        // The first elimination gives (x³⁰-1)(y+x³⁰); remove that content before the next elimination.
        let expanded = polynomial(
            &[([30, 1], 1), ([0, 1], -1), ([60, 0], 1), ([30, 0], -1)],
            &mut budget,
        )?;
        let leading = polynomial(&[([30, 0], 1)], &mut budget)?;
        assert!(
            leading
                .mul(
                    &expanded,
                    &mut Budget::new(crate::limits::Limits::blind_cubic())?
                )
                .is_err()
        );
        let common = gcd(&first, &second, &mut budget)?;
        assert!(common.equals(&first.one(&mut budget)?, &mut budget)?);
        let factor = polynomial(
            &[([1, 1], 1), ([1, 0], 1), ([0, 1], 1), ([0, 0], 1)],
            &mut budget,
        )?;
        let common = gcd(
            &first.mul(&factor, &mut budget)?,
            &second.mul(&factor, &mut budget)?,
            &mut budget,
        )?;
        assert!(common.equals(&factor, &mut budget)?);
        Ok(())
    }

    /// The measured small fraction endpoints are coprime without the overflowing main-variable remainder sequence.
    /// ´test:unit:independent-variable-gcd-bound´
    #[test]
    fn independent_variable_gcd_bound() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let polynomial = |terms: &[([u32; 4], i64, u64)], ledger: &mut Budget| {
            let mut entries = Vec::new();
            for (powers, numerator, denominator) in terms {
                entries.push(Term {
                    powers: powers.to_vec(),
                    coefficient: Scalar::Rational(Rational::from_ratio(
                        *numerator,
                        *denominator,
                        ledger,
                    )?),
                });
            }
            Polynomial::new(4, Ground::Rational, entries, ledger)
        };
        let numerator = polynomial(
            &[
                ([0, 2, 0, 5], 4, 729),
                ([0, 1, 2, 4], -8, 2187),
                ([0, 0, 4, 3], 4, 6561),
            ],
            &mut budget,
        )?;
        let denominator = polynomial(
            &[
                ([2, 0, 0, 4], 1, 1),
                ([1, 1, 1, 3], -2, 3),
                ([1, 0, 3, 2], 4, 27),
                ([0, 2, 2, 2], 1, 9),
                ([0, 1, 4, 1], -4, 81),
                ([0, 0, 6, 0], 4, 729),
            ],
            &mut budget,
        )?;
        // The numerator omits v0. The leading denominator coefficient is v3^4; its constant coefficient has a nonzero v2^6 term.
        let common = gcd(&numerator, &denominator, &mut budget)?;
        assert!(common.equals(&numerator.one(&mut budget)?, &mut budget)?);
        let factor = polynomial(&[([0, 0, 0, 1], 1, 1)], &mut budget)?;
        let common = gcd(
            &numerator.mul(&factor, &mut budget)?,
            &denominator.mul(&factor, &mut budget)?,
            &mut budget,
        )?;
        assert!(common.equals(&factor, &mut budget)?);
        Ok(())
    }

    fn measured_polynomial(
        terms: &[([u32; 4], i64, u64)],
        budget: &mut Budget,
    ) -> Result<Polynomial, crate::error::Fault> {
        let mut entries = Vec::new();
        for (powers, numerator, denominator) in terms {
            entries.push(Term {
                powers: powers.to_vec(),
                coefficient: Scalar::Rational(Rational::from_ratio(
                    *numerator,
                    *denominator,
                    budget,
                )?),
            });
        }
        Polynomial::new(4, Ground::Rational, entries, budget)
    }

    /// The independently factored checkpoint pair has gcd q, and a shared coordinate factor survives.
    /// ´test:unit:integer-primitive-gcd-bound´
    #[test]
    fn integer_primitive_gcd_bound() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let numerator = measured_polynomial(
            &[
                ([2, 1, 0, 7], 4, 243),
                ([2, 0, 2, 6], -4, 729),
                ([1, 2, 1, 6], -28, 2187),
                ([1, 1, 3, 5], 16, 2187),
                ([1, 0, 5, 4], -20, 19683),
                ([0, 3, 2, 5], 16, 6561),
                ([0, 2, 4, 4], -116, 59049),
                ([0, 1, 6, 3], 92, 177_147),
                ([0, 0, 8, 2], -8, 177_147),
            ],
            &mut budget,
        )?;
        let denominator = measured_polynomial(
            &[
                ([3, 0, 0, 6], 1, 1),
                ([2, 1, 1, 5], -1, 1),
                ([2, 0, 3, 4], 2, 9),
                ([1, 2, 2, 4], 1, 3),
                ([1, 1, 4, 3], -4, 27),
                ([1, 0, 6, 2], 4, 243),
                ([0, 3, 3, 3], -1, 27),
                ([0, 2, 5, 2], 2, 81),
                ([0, 1, 7, 1], -4, 729),
                ([0, 0, 9, 0], 8, 19683),
            ],
            &mut budget,
        )?;
        // N=(4/243)v3²*p*q*r, D=q³, with primitive irreducible q linear in v0.
        // p=v1*v3-v2²/3 and r=v0*v3²-4*v1*v2*v3/9+v2³/9 are not divisible by q.
        let expected = measured_polynomial(
            &[
                ([1, 0, 0, 2], 1, 1),
                ([0, 1, 1, 1], -1, 3),
                ([0, 0, 3, 0], 2, 27),
            ],
            &mut budget,
        )?;
        let common = gcd(&numerator, &denominator, &mut budget)?;
        assert!(common.equals(&expected, &mut budget)?);
        let coordinate = measured_polynomial(&[([0, 0, 0, 1], 1, 1)], &mut budget)?;
        let common = gcd(
            &numerator.mul(&coordinate, &mut budget)?,
            &denominator.mul(&coordinate, &mut budget)?,
            &mut budget,
        )?;
        assert!(common.equals(&expected.mul(&coordinate, &mut budget)?, &mut budget)?);
        Ok(())
    }

    /// Rational content uses canonical components, ignores signs and specifies zero operands.
    /// ´test:unit:rational-content-gcd-contract´
    #[test]
    fn rational_content_gcd_contract() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let first = Rational::from_ratio(2, 3, &mut budget)?;
        let second = Rational::from_ratio(-4, 9, &mut budget)?;
        let zero = Rational::from_ratio(0, 1, &mut budget)?;
        assert_eq!(
            first.content_gcd(&second, &mut budget)?,
            Rational::from_ratio(2, 9, &mut budget)?
        );
        assert_eq!(
            zero.content_gcd(&second, &mut budget)?,
            Rational::from_ratio(4, 9, &mut budget)?
        );
        assert_eq!(first.content_gcd(&zero, &mut budget)?, first);
        assert_eq!(zero.content_gcd(&zero, &mut budget)?, zero);
        Ok(())
    }

    /// Integer normalization retains both Gaussian components and never divides by zero content.
    /// ´test:unit:integer-primitive-gaussian-components´
    #[test]
    fn integer_primitive_gaussian_components() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let gaussian = |real, imaginary, denominator, ledger: &mut Budget| {
            crate::solving::numeric::Gaussian::from_parts(
                &Rational::from_ratio(real, denominator, ledger)?,
                &Rational::from_ratio(imaginary, denominator, ledger)?,
                ledger,
            )
        };
        let original = Polynomial::new(
            1,
            Ground::Gaussian,
            vec![
                Term {
                    powers: vec![1],
                    coefficient: Scalar::Gaussian(gaussian(-6, 4, 9, &mut budget)?),
                },
                Term {
                    powers: vec![0],
                    coefficient: Scalar::Gaussian(gaussian(10, 0, 21, &mut budget)?),
                },
            ],
            &mut budget,
        )?;
        let expected = Polynomial::new(
            1,
            Ground::Gaussian,
            vec![
                Term {
                    powers: vec![1],
                    coefficient: Scalar::Gaussian(gaussian(-21, 14, 1, &mut budget)?),
                },
                Term {
                    powers: vec![0],
                    coefficient: Scalar::Gaussian(gaussian(15, 0, 1, &mut budget)?),
                },
            ],
            &mut budget,
        )?;
        assert!(super::integral(&original, &mut budget)?.equals(&expected, &mut budget)?);
        let zero = super::zero(&original, &mut budget)?;
        assert!(super::integral(&zero, &mut budget)?.zero(&mut budget)?);
        let common = gcd(&original, &expected, &mut budget)?;
        assert!(common.equals(&expected.monic(&mut budget)?, &mut budget)?);
        Ok(())
    }

    /// Monomial division checks every term, preserves scalar coefficients, and refuses a missing factor.
    /// ´test:unit:monomial-quotient-reconstruction´
    #[test]
    fn monomial_quotient_reconstruction() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(crate::limits::Limits::blind_cubic())?;
        let numerator =
            measured_polynomial(&[([3, 2, 0, 0], 6, 1), ([1, 4, 0, 0], -9, 1)], &mut budget)?;
        let divisor = measured_polynomial(&[([1, 1, 0, 0], 3, 1)], &mut budget)?;
        let expected =
            measured_polynomial(&[([2, 1, 0, 0], 2, 1), ([0, 3, 0, 0], -3, 1)], &mut budget)?;
        let quotient =
            super::exact(&numerator, &divisor, &mut budget)?.ok_or("missing exact quotient")?;
        assert!(quotient.equals(&expected, &mut budget)?);
        assert!(
            quotient
                .mul(&divisor, &mut budget)?
                .equals(&numerator, &mut budget)?
        );
        let nondivisible =
            measured_polynomial(&[([3, 2, 0, 0], 6, 1), ([0, 4, 0, 0], -9, 1)], &mut budget)?;
        assert!(super::exact(&nondivisible, &divisor, &mut budget)?.is_none());
        let zero = super::zero(&numerator, &mut budget)?;
        assert!(
            super::exact(&zero, &divisor, &mut budget)?
                .ok_or("missing zero quotient")?
                .zero(&mut budget)?
        );
        assert!(super::exact(&numerator, &zero, &mut budget).is_err());
        Ok(())
    }
}
