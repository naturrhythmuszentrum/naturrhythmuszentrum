// SPDX-License-Identifier: AGPL-3.0-only
//! Positive rational and Pi monomial square roots in the closed constant fragment.

mod pi;

use std::cmp::Ordering;

use super::{AT, Atom, Fraction, Monomial, Polynomial, cancel_monomial, entry_bytes, fragment};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{Branch, Operator, Sort};
use crate::number::Rational;

/// A Pi tag and complete canonical rational bytes order one strictly positive radicand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Root {
    /// Positive exact radicand, privately checked at construction.
    value: Rational,
    /// Whether the positive radicand includes one additional factor of Pi.
    pi: bool,
    /// Collision-free canonical component encoding, never a digest or display string.
    key: Vec<u8>,
}

impl Ord for Root {
    fn cmp(&self, other: &Self) -> Ordering {
        self.pi
            .cmp(&other.pi)
            .then_with(|| self.key.cmp(&other.key))
    }
}

impl PartialOrd for Root {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Root {
    /// Key only a finite rational with an independently established strict positive sign.
    fn new(value: &Rational, pi: bool, budget: &mut Budget) -> Result<Self, Fault> {
        value.check(budget, AT)?;
        if value.is_zero() || value.is_negative() {
            return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
        }
        let key = value.structural_bytes(budget)?;
        Ok(Self {
            value: value.clone(),
            pi,
            key,
        })
    }

    /// Include both complete key bytes and copied rational components.
    pub(super) fn bytes(&self) -> Result<u64, Fault> {
        Ok(size(self.key.len(), AT)?
            .saturating_mul(4)
            .saturating_add(128))
    }

    /// Materialize exactly the strict-real half power, with no new wire constructor.
    pub(super) fn materialize(
        &self,
        cas: &mut Cas,
        header: LogicalId,
        sort: &Sort,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let mut base = cas.generated(
            header,
            Operator::Rational(self.value.clone()),
            sort.clone(),
            &[],
            budget,
        )?;
        if self.pi {
            let pi = cas.generated(
                header,
                Operator::Constant(crate::model::Constant::Pi),
                sort.clone(),
                &[],
                budget,
            )?;
            base = if self.value == Rational::one() {
                pi
            } else {
                cas.generated(header, Operator::Mul, sort.clone(), &[base, pi], budget)?
            };
        }
        let half = Rational::from_ratio(1, 2, budget)?;
        let exponent =
            cas.generated(header, Operator::Rational(half), sort.clone(), &[], budget)?;
        cas.generated(
            header,
            Operator::GeneralPower(Branch::StrictReal),
            sort.clone(),
            &[base, exponent],
            budget,
        )
    }
}

/// Extract a rational only after exact common-monomial cancellation of its formal fraction.
fn rational(value: &Fraction, budget: &mut Budget) -> Result<Rational, Fault> {
    value.numerator.prepay(budget)?;
    value.denominator.prepay(budget)?;
    let mut value = value.clone();
    cancel_monomial(&mut value, budget)?;
    let Some((denominator_monomial, denominator)) = value.denominator.terms.first_key_value()
    else {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    };
    if value.denominator.terms.len() != 1 || !denominator_monomial.is_empty() {
        return Err(fragment());
    }
    let Some((monomial, numerator)) = value.numerator.terms.first_key_value() else {
        return Ok(Rational::zero());
    };
    if value.numerator.terms.len() != 1 || !monomial.is_empty() {
        return Err(fragment());
    }
    numerator.div(denominator, budget)
}

/// Admit a rational first; only the explicit Pi monomial extension handles non-rational bases.
pub(super) fn convert(
    base: &Fraction,
    exponent: &Fraction,
    budget: &mut Budget,
) -> Result<Fraction, Fault> {
    if rational(exponent, budget)? != Rational::from_ratio(1, 2, budget)? {
        return Err(fragment());
    }
    let value = match rational(base, budget) {
        Ok(value) => value,
        Err(fault) if fault == fragment() => return pi::convert(base, budget),
        Err(fault) => return Err(fault),
    };
    if value.is_zero() || value.is_negative() {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    let result = if let Some(root) = value.square_root_exact(budget)? {
        Polynomial::constant(&root, budget)?
    } else {
        Polynomial::atom(
            Atom::Root(Box::new(Root::new(&value, false, budget)?)),
            budget,
        )?
    };
    Fraction::polynomial(result, budget)
}

/// Combine only strictly positive root atoms; even powers use their exact defining squares.
pub(super) fn reduce(
    monomial: Monomial,
    coefficient: &Rational,
    budget: &mut Budget,
) -> Result<(Monomial, Rational), Fault> {
    let bytes = entry_bytes(&monomial, coefficient)?.saturating_mul(8);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(monomial.len())
        .map_err(|_| Fault::allocation(AT))?;
    let mut scale = Rational::one();
    let mut radicand = Rational::one();
    let mut pi_power = 0_u64;
    for (atom, exponent) in monomial {
        if let Atom::Root(root) = atom {
            if root.pi {
                pi_power = pi_power.checked_add(exponent).ok_or_else(fragment)?;
            }
            let power = i64::try_from(exponent / 2).map_err(|_| fragment())?;
            if power != 0 {
                scale = scale.mul(&root.value.pow(power, budget)?, budget)?;
            }
            if exponent % 2 != 0 {
                radicand = radicand.mul(&root.value, budget)?;
            }
        } else {
            result.push((atom, exponent));
        }
    }
    if pi_power >= 2 {
        pi::merge(&mut result, pi_power / 2, budget)?;
    }
    if pi_power % 2 != 0 {
        result.push((Atom::Root(Box::new(Root::new(&radicand, true, budget)?)), 1));
    } else if let Some(root) = radicand.square_root_exact(budget)? {
        scale = scale.mul(&root, budget)?;
    } else {
        result.push((
            Atom::Root(Box::new(Root::new(&radicand, false, budget)?)),
            1,
        ));
    }
    Ok((result, coefficient.mul(&scale, budget)?))
}

/// Ordinary atom comparisons retain their exact earlier charge; root keys include full components.
pub(super) fn comparison_bytes(atom: &Atom, budget: &Budget) -> Result<u64, Fault> {
    if let Atom::Root(root) = atom {
        Ok(root.bytes()?.max(budget.limits().name_bytes))
    } else {
        Ok(budget.limits().name_bytes)
    }
}
