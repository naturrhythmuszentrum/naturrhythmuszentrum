// SPDX-License-Identifier: AGPL-3.0-only
//! Closed ordered-field rules, each reconstructed from the actual expression syntax.

use super::sign::{entails, mask, negate, scalar};
use super::{AT, invalid, sign_evidence};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::proof_model::CheckedApplication;
use crate::solving::{RealSignRule, RealSignStep};

/// The three possible strict signs, with weak facts expanded finitely.
const SIGNS: [u8; 3] = [1, 2, 4];

/// Ordered-field addition permits either sign when nonzero summands have opposite signs.
const fn sum(left: u8, right: u8) -> u8 {
    if left == 2 {
        right
    } else if right == 2 || left == right {
        left
    } else {
        7
    }
}

/// Zero annihilates; equal nonzero signs have a positive product.
const fn product(left: u8, right: u8) -> u8 {
    if left == 2 || right == 2 {
        2
    } else if left == right {
        4
    } else {
        1
    }
}

/// Apply a finite ordered-field sign table to every possibility retained by both premises.
pub(super) fn combine(left: u8, right: u8, multiply: bool) -> u8 {
    let mut result = 0;
    for first in SIGNS {
        for second in SIGNS {
            if left & first != 0 && right & second != 0 {
                result |= if multiply {
                    product(first, second)
                } else {
                    sum(first, second)
                };
            }
        }
    }
    result
}

/// Binary rules match exact operand subjects and check strict reciprocal definedness.
fn binary(
    cas: &Cas,
    step: &RealSignStep,
    arguments: &[&RealSignStep],
    multiply: bool,
) -> Result<(), Fault> {
    let [left, right] = arguments else {
        return Err(invalid());
    };
    let expression = cas.state.expression(step.subject.logical_id())?;
    if expression.operands != [left.subject.logical_id(), right.subject.logical_id()] {
        return inverse_binary(cas, step, arguments, multiply);
    }
    let first = mask(left.conclusion);
    let mut second = mask(right.conclusion);
    match (&expression.operator, multiply) {
        (Operator::Add, false) | (Operator::Mul, true) => {}
        (Operator::Sub, false) => second = negate(second),
        (Operator::Div, true) if second & 2 == 0 => {}
        _ => return Err(invalid()),
    }
    entails(combine(first, second, multiply), step.conclusion)
}

/// Reflect a known sum/product sign through a known operand using only its finite sign possibilities.
fn inverse_binary(
    cas: &Cas,
    step: &RealSignStep,
    arguments: &[&RealSignStep],
    multiply: bool,
) -> Result<(), Fault> {
    let [whole, known] = arguments else {
        return Err(invalid());
    };
    let expression = cas.state.expression(whole.subject.logical_id())?;
    let desired = if multiply {
        Operator::Mul
    } else {
        Operator::Add
    };
    let forward = expression.operands == [step.subject.logical_id(), known.subject.logical_id()];
    let backward = expression.operands == [known.subject.logical_id(), step.subject.logical_id()];
    if expression.operator != desired || !(forward || backward) {
        return pair(cas, step, arguments, multiply);
    }
    let mut possible = 0;
    for sign in SIGNS {
        if combine(sign, mask(known.conclusion), multiply) & mask(whole.conclusion) != 0 {
            possible |= sign;
        }
    }
    entails(possible, step.conclusion)
}

/// Powers are checked against their actual exponent; no inverse principal-root law is used.
fn power(
    cas: &Cas,
    region: &Handle,
    step: &RealSignStep,
    arguments: &[&RealSignStep],
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if step.exponent == 0 || !step.exponent.is_multiple_of(2) {
        return Err(invalid());
    }
    let expression = cas.state.expression(step.subject.logical_id())?;
    if expression.operator != Operator::IntegerPower(i64::from(step.exponent)) {
        let [source] = arguments else {
            return Err(invalid());
        };
        let powered = cas.state.expression(source.subject.logical_id())?;
        if powered.operator != Operator::IntegerPower(i64::from(step.exponent))
            || powered.operands != [step.subject.logical_id()]
        {
            return Err(invalid());
        }
        return match mask(source.conclusion) {
            2 => entails(2, step.conclusion),
            4 | 5 => entails(5, step.conclusion),
            _ => Err(invalid()),
        };
    }
    let [operand] = expression.operands.as_slice() else {
        return Err(invalid());
    };
    let possible = match arguments {
        [] => {
            crate::solving::elementary::total::check(cas, *operand, region, premises, budget)?;
            6
        }
        [source] if source.subject.logical_id() == *operand => {
            let signs = mask(source.conclusion);
            (if signs & 2 == 0 { 0 } else { 2 }) | (if signs & 5 == 0 { 0 } else { 4 })
        }
        _ => return Err(invalid()),
    };
    entails(possible, step.conclusion)
}

/// The real map x -> x^n is order-reflecting for a checked positive odd integer n.
fn reflection(cas: &Cas, step: &RealSignStep, arguments: &[&RealSignStep]) -> Result<(), Fault> {
    let [source] = arguments else {
        return Err(invalid());
    };
    if step.exponent == 0 || step.exponent.is_multiple_of(2) {
        return Err(invalid());
    }
    let target = cas.state.expression(step.subject.logical_id())?;
    if target.operator == Operator::IntegerPower(i64::from(step.exponent))
        && target.operands == [source.subject.logical_id()]
    {
        return entails(mask(source.conclusion), step.conclusion);
    }
    let expression = cas.state.expression(source.subject.logical_id())?;
    if expression.operator == Operator::IntegerPower(i64::from(step.exponent))
        && expression.operands == [step.subject.logical_id()]
    {
        return entails(mask(source.conclusion), step.conclusion);
    }
    if expression.operator != Operator::Sub || target.operator != Operator::Sub {
        return Err(invalid());
    }
    if expression.operands.len() != 2 || target.operands.len() != 2 {
        return Err(invalid());
    }
    for (powered, endpoint) in expression.operands.iter().zip(&target.operands) {
        let power = cas.state.expression(*powered)?;
        if power.operator != Operator::IntegerPower(i64::from(step.exponent))
            || power.operands != [*endpoint]
        {
            return Err(invalid());
        }
    }
    entails(mask(source.conclusion), step.conclusion)
}

/// Rules consume only their defined data; surplus exponent or premise fields are rejected.
pub(super) fn check(
    cas: &Cas,
    region: &Handle,
    step: &RealSignStep,
    arguments: &[&RealSignStep],
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    scalar(cas, &step.subject, region)?;
    match step.rule {
        RealSignRule::Hypothesis => {
            return sign_evidence::hypothesis(cas, region, step, premises, budget);
        }
        RealSignRule::Identity => {
            return sign_evidence::identity(cas, region, step, arguments, premises, budget);
        }
        RealSignRule::Root => return sign_evidence::root(cas, region, step, premises, budget),
        _ => {}
    }
    if !step.premises.is_empty() {
        return Err(invalid());
    }
    match step.rule {
        RealSignRule::EvenPower => power(cas, region, step, arguments, premises, budget),
        RealSignRule::OddPowerReflection => reflection(cas, step, arguments),
        _ if step.exponent != 0 => Err(invalid()),
        RealSignRule::Sum => binary(cas, step, arguments, false),
        RealSignRule::Product => binary(cas, step, arguments, true),
        RealSignRule::Rational => rational(cas, step, arguments, budget),
        RealSignRule::Negation => negation(cas, step, arguments),
        _ => Err(invalid()),
    }
}

/// Exact literal comparison needs no floating evaluation or producer oracle.
fn rational(
    cas: &Cas,
    step: &RealSignStep,
    arguments: &[&RealSignStep],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if !arguments.is_empty() {
        return Err(invalid());
    }
    let Operator::Rational(value) = &cas.state.expression(step.subject.logical_id())?.operator
    else {
        return Err(invalid());
    };
    let possible = match value.compare(&crate::number::Rational::zero(), budget)? {
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => 2,
        std::cmp::Ordering::Greater => 4,
    };
    entails(possible, step.conclusion)
}

/// Unary negation exchanges negative and positive possibilities.
fn negation(cas: &Cas, step: &RealSignStep, arguments: &[&RealSignStep]) -> Result<(), Fault> {
    let [source] = arguments else {
        return Err(invalid());
    };
    let expression = cas.state.expression(step.subject.logical_id())?;
    if expression.operator != Operator::Neg || expression.operands != [source.subject.logical_id()]
    {
        return Err(invalid());
    }
    entails(negate(mask(source.conclusion)), step.conclusion)
}

/// A common pair's sum and product constrain both signs by the same finite ordered-field tables.
fn pair(
    cas: &Cas,
    step: &RealSignStep,
    arguments: &[&RealSignStep],
    multiply: bool,
) -> Result<(), Fault> {
    let [sum_fact, product_fact] = arguments else {
        return Err(invalid());
    };
    if multiply {
        return Err(invalid());
    }
    let sum_expression = cas.state.expression(sum_fact.subject.logical_id())?;
    let product_expression = cas.state.expression(product_fact.subject.logical_id())?;
    if sum_expression.operator != Operator::Add
        || product_expression.operator != Operator::Mul
        || sum_expression.operands != product_expression.operands
        || !sum_expression.operands.contains(&step.subject.logical_id())
    {
        return Err(invalid());
    }
    let mut possible = 0;
    for left in SIGNS {
        for right in SIGNS {
            if sum(left, right) & mask(sum_fact.conclusion) != 0
                && product(left, right) & mask(product_fact.conclusion) != 0
            {
                possible |= left;
            }
        }
    }
    entails(possible, step.conclusion)
}
