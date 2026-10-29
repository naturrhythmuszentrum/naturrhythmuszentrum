// SPDX-License-Identifier: AGPL-3.0-only
//! Closed finite-real sign rules inspect every original operation, including hidden divisors.

use std::collections::BTreeMap;

use super::graph::{AT, order};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{LogicalId, State};
use crate::model::Operator;
use crate::number::Rational;

/// A proved sign or an explicitly unresolved comparison of exact constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Sign {
    /// Strictly negative and finite.
    Negative,
    /// Exactly zero.
    Zero,
    /// Strictly positive and finite.
    Positive,
    /// Finite, with no established sign.
    Unknown,
}

impl Sign {
    /// Rational literals have an exact sign without numerical evaluation.
    pub(crate) fn rational(value: &Rational) -> Self {
        if value.is_zero() {
            Self::Zero
        } else if value.is_negative() {
            Self::Negative
        } else {
            Self::Positive
        }
    }

    /// Negation preserves finiteness and reverses strict signs.
    const fn negated(self) -> Self {
        match self {
            Self::Negative => Self::Positive,
            Self::Positive => Self::Negative,
            other => other,
        }
    }

    /// Partial sign addition deliberately refuses mixed strict signs.
    const fn add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Zero, right) => right,
            (left, Self::Zero) => left,
            (Self::Positive, Self::Positive) => Self::Positive,
            (Self::Negative, Self::Negative) => Self::Negative,
            _ => Self::Unknown,
        }
    }

    /// The product sign rule applies only after both original operands are finite.
    const fn mul(self, other: Self) -> Self {
        match (self, other) {
            (Self::Zero, _) | (_, Self::Zero) => Self::Zero,
            (Self::Positive, right) => right,
            (Self::Negative, right) => right.negated(),
            _ => Self::Unknown,
        }
    }

    /// A divisor requires a checked strict sign, never an assumption.
    const fn nonzero(self) -> Result<(), Fault> {
        match self {
            Self::Positive | Self::Negative => Ok(()),
            Self::Zero => Err(Fault::invalid(InvalidCode::RefutedDomain, AT)),
            Self::Unknown => Err(Fault::invalid(InvalidCode::PendingPremise, AT)),
        }
    }
}

/// Exact rational subresults strengthen the closed sign rules without approximating constants.
struct Fact {
    /// A rational subexpression when all its operands are rational.
    rational: Option<Rational>,
    /// Sign evidence reconstructed from the operation and its checked operands.
    sign: Sign,
}

/// Check finiteness before normalization and derive a conservative sign on one ledger.
pub(crate) fn check(state: &State, root: LogicalId, budget: &mut Budget) -> Result<Sign, Fault> {
    let mut facts = BTreeMap::new();
    let mut retained = 0_u64;
    for id in order(state, root, budget)? {
        let term = state.expression(id)?;
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = fact(&term.operator, &term.operands, &facts, budget);
        budget.restore_limits(previous);
        let result = result?;
        let bits = result.rational.as_ref().map_or(0, |value| {
            let (numerator, denominator) = value.component_bits();
            numerator.saturating_add(denominator)
        });
        retained = retained
            .saturating_add(128)
            .saturating_add(bits.div_ceil(8));
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        budget.work(retained, AT)?;
        facts.insert(id, result);
    }
    facts
        .remove(&root)
        .map(|value| value.sign)
        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
}

/// Rational evaluation and sign inference share the same original children.
fn fact(
    operator: &Operator,
    operands: &[LogicalId],
    facts: &BTreeMap<LogicalId, Fact>,
    budget: &mut Budget,
) -> Result<Fact, Fault> {
    let child = |position: usize| {
        operands
            .get(position)
            .and_then(|id| facts.get(id))
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
    };
    if let Operator::Rational(value) = operator {
        value.check(budget, AT)?;
        return Ok(Fact {
            rational: Some(value.clone()),
            sign: Sign::rational(value),
        });
    }
    if matches!(operator, Operator::Constant(_)) {
        return Ok(Fact {
            rational: None,
            sign: Sign::Positive,
        });
    }
    let first = child(0)?;
    let rational = match operator {
        Operator::Neg => first
            .rational
            .as_ref()
            .map(|value| value.negated(budget))
            .transpose()?,
        Operator::IntegerPower(exponent) => {
            if *exponent <= 0 {
                first.sign.nonzero()?;
            }
            first
                .rational
                .as_ref()
                .map(|value| value.pow(*exponent, budget))
                .transpose()?
        }
        Operator::GeneralPower(crate::model::Branch::StrictReal) => {
            square_root(first, child(1)?, budget)?
        }
        _ => rational_binary(operator, first, child(1)?, budget)?,
    };
    let sign = if let Some(value) = &rational {
        Sign::rational(value)
    } else {
        symbolic(
            operator,
            first.sign,
            operands.get(1).and_then(|id| facts.get(id)).map(|v| v.sign),
        )?
    };
    Ok(Fact { rational, sign })
}

/// Half powers require strict positivity of the original radicand before any normalization.
fn square_root(
    first: &Fact,
    exponent: &Fact,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    if exponent.rational.as_ref() != Some(&Rational::from_ratio(1, 2, budget)?) {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    match first.sign {
        Sign::Positive => {}
        Sign::Negative | Sign::Zero => return Err(Fault::invalid(InvalidCode::RefutedDomain, AT)),
        Sign::Unknown => return Err(Fault::invalid(InvalidCode::PendingPremise, AT)),
    }
    first
        .rational
        .as_ref()
        .map(|value| value.square_root_exact(budget))
        .transpose()
        .map(Option::flatten)
}

/// A rational divisor is checked even when its numerator contains an irrational constant.
fn rational_binary(
    operator: &Operator,
    first: &Fact,
    second: &Fact,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    if matches!(operator, Operator::Div) {
        second.sign.nonzero()?;
    }
    let Some((left, right)) = first.rational.as_ref().zip(second.rational.as_ref()) else {
        return Ok(None);
    };
    Ok(Some(match operator {
        Operator::Add => left.add(right, budget)?,
        Operator::Sub => left.sub(right, budget)?,
        Operator::Mul => left.mul(right, budget)?,
        Operator::Div => left.div(right, budget)?,
        _ => return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT)),
    }))
}

/// Finite closed constants obey elementary real arithmetic; mixed sums can remain unproved.
fn symbolic(operator: &Operator, first: Sign, second: Option<Sign>) -> Result<Sign, Fault> {
    match operator {
        Operator::Neg => Ok(first.negated()),
        Operator::GeneralPower(crate::model::Branch::StrictReal) => Ok(Sign::Positive),
        Operator::IntegerPower(exponent) => {
            if *exponent <= 0 {
                first.nonzero()?;
            }
            let zero_power = *exponent == 0;
            let even_nonzero =
                exponent % 2 == 0 && matches!(first, Sign::Negative | Sign::Positive);
            Ok(if zero_power || even_nonzero {
                Sign::Positive
            } else {
                first
            })
        }
        _ => {
            let second = second.ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?;
            Ok(match operator {
                Operator::Add => first.add(second),
                Operator::Sub => first.add(second.negated()),
                Operator::Mul | Operator::Div => first.mul(second),
                _ => return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT)),
            })
        }
    }
}
