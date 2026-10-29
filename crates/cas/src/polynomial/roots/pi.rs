// SPDX-License-Identifier: AGPL-3.0-only
//! The bounded radicand extension is q*Pi^k for positive rational q and integral k.

use super::{
    AT, Atom, Budget, BudgetKind, Fault, Fraction, Monomial, Polynomial, Rational, Root, fragment,
    size,
};

/// Recover one exact rational coefficient and its sole permitted named constant exponent.
fn monomial(value: &Polynomial, budget: &mut Budget) -> Result<(Rational, i64), Fault> {
    value.prepay(budget)?;
    let Some((monomial, coefficient)) = value.terms.first_key_value() else {
        return Err(fragment());
    };
    if value.terms.len() != 1 {
        return Err(fragment());
    }
    let exponent = match monomial.as_slice() {
        [] => 0,
        [(Atom::Constant(0), exponent)] => i64::try_from(*exponent).map_err(|_| fragment())?,
        _ => return Err(fragment()),
    };
    Ok((coefficient.clone(), exponent))
}

/// Positive Pi permits even powers to leave the root; no sum or other atom is accepted.
pub(super) fn convert(base: &Fraction, budget: &mut Budget) -> Result<Fraction, Fault> {
    let (numerator, numerator_power) = monomial(&base.numerator, budget)?;
    let (denominator, denominator_power) = monomial(&base.denominator, budget)?;
    let value = numerator.div(&denominator, budget)?;
    if value.is_zero() || value.is_negative() {
        return Err(Fault::invalid(super::InvalidCode::RefutedDomain, AT));
    }
    let power = numerator_power
        .checked_sub(denominator_power)
        .ok_or_else(fragment)?;
    let odd = power.rem_euclid(2) != 0;
    let root = if odd {
        Polynomial::atom(
            Atom::Root(Box::new(Root::new(&value, true, budget)?)),
            budget,
        )?
    } else if let Some(root) = value.square_root_exact(budget)? {
        Polynomial::constant(&root, budget)?
    } else {
        Polynomial::atom(
            Atom::Root(Box::new(Root::new(&value, false, budget)?)),
            budget,
        )?
    };
    let power = power.div_euclid(2);
    if power == 0 {
        return Fraction::polynomial(root, budget);
    }
    let pi = Polynomial::atom(Atom::Constant(0), budget)?.pow(power.unsigned_abs(), budget)?;
    if power < 0 {
        Ok(Fraction {
            numerator: root,
            denominator: pi,
        })
    } else {
        Fraction::polynomial(root.mul(&pi, budget)?, budget)
    }
}

/// Insert the Pi powers released by positive roots while preserving complete monomial order.
pub(super) fn merge(monomial: &mut Monomial, power: u64, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(
        size(monomial.len(), AT)?
            .saturating_add(1)
            .saturating_mul(budget.limits().name_bytes),
        AT,
    )?;
    let atom = Atom::Constant(0);
    match monomial.binary_search_by(|(candidate, _)| candidate.cmp(&atom)) {
        Ok(index) => {
            let exponent = monomial[index].1.checked_add(power).ok_or_else(fragment)?;
            budget.check(BudgetKind::PolynomialDegree, exponent, AT)?;
            monomial[index].1 = exponent;
        }
        Err(index) => {
            budget.check(BudgetKind::PolynomialDegree, power, AT)?;
            monomial.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            monomial.insert(index, (atom, power));
        }
    }
    Ok(())
}
