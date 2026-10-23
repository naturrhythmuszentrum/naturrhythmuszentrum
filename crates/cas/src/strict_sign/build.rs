// SPDX-License-Identifier: AGPL-3.0-only
//! Postfix proposals preserve original operands and strict denominator signs.

use super::{AT, unavailable};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::{
    CertificateRealSign, RealSignConclusion as Sign, RealSignRule as Rule, RealSignStep,
};

/// Allocate within the simultaneous reservation established by the caller.
fn allocated<T>(count: usize) -> Result<Vec<T>, Fault> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(values)
}

/// Negation exchanges the two admitted strict signs.
const fn reversed(sign: Sign) -> Result<Sign, Fault> {
    match sign {
        Sign::Positive => Ok(Sign::Negative),
        Sign::Negative => Ok(Sign::Positive),
        _ => Err(unavailable()),
    }
}

/// Propose one existing rule using exact original operand identities.
fn step(
    cas: &Cas,
    id: LogicalId,
    region: &Handle,
    inputs: &[(u32, Sign)],
    budget: &mut Budget,
) -> Result<RealSignStep, Fault> {
    let expression = cas.state.expression(id)?;
    let subject = cas.handle(id);
    let (rule, conclusion, exponent) = match (&expression.operator, inputs) {
        (Operator::Rational(value), []) if !value.is_zero() => (
            Rule::Rational,
            if value.is_negative() {
                Sign::Negative
            } else {
                Sign::Positive
            },
            0,
        ),
        (Operator::Neg, [(_, sign)]) => (Rule::Negation, reversed(*sign)?, 0),
        (Operator::Mul | Operator::Div, [(_, left), (_, right)]) => (
            Rule::Product,
            if left == right {
                Sign::Positive
            } else {
                Sign::Negative
            },
            0,
        ),
        (Operator::IntegerPower(power), [(_, sign)]) if *power > 0 => {
            let exponent = u32::try_from(*power).map_err(|_| unavailable())?;
            if exponent.is_multiple_of(2) {
                (Rule::EvenPower, Sign::Positive, exponent)
            } else {
                (Rule::OddPowerReflection, *sign, exponent)
            }
        }
        (Operator::Rational(_), []) => return Err(unavailable()),
        (_, []) => (
            Rule::Hypothesis,
            super::guards::sign(cas, &subject, region, budget)?,
            0,
        ),
        _ => return Err(unavailable()),
    };
    let mut arguments = allocated(inputs.len())?;
    arguments.extend(inputs.iter().map(|(index, _)| *index));
    Ok(RealSignStep {
        rule,
        subject,
        conclusion,
        arguments,
        premises: Vec::new(),
        exponent,
    })
}

/// Admit only Real expressions in the exact regional frame.
fn operands<'a>(cas: &'a Cas, id: LogicalId, region: &Handle) -> Result<&'a [LogicalId], Fault> {
    let expression = cas.state.expression(id)?;
    let frame = cas.state.region(region.logical_id())?;
    if expression.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || expression.header != frame.header
        || expression.scope != frame.scope
        || expression.space.is_some()
    {
        return Err(unavailable());
    }
    match expression.operator {
        Operator::Neg | Operator::Mul | Operator::Div | Operator::IntegerPower(_) => {
            Ok(&expression.operands)
        }
        _ => Ok(&[]),
    }
}

/// Visit original occurrences in postfix order without recursive stack growth.
pub(super) fn certificate(
    cas: &Cas,
    subject: &Handle,
    region: &Handle,
    budget: &mut Budget,
) -> Result<CertificateRealSign, Fault> {
    let original = cas.state.expression(cas.checked(subject)?)?;
    budget.check(BudgetKind::Depth, original.depth, AT)?;
    budget.check(BudgetKind::ConditionSteps, original.unfolded, AT)?;
    let count = usize::try_from(original.unfolded).map_err(|_| unavailable())?;
    let mut pending = allocated(count)?;
    let mut values = allocated(count)?;
    let mut steps = allocated(count)?;
    pending.push((subject.logical_id(), false));
    while let Some((id, ready)) = pending.pop() {
        budget.work(1, AT)?;
        let children = operands(cas, id, region)?;
        if !ready && !children.is_empty() {
            pending.push((id, true));
            pending.extend(children.iter().rev().map(|child| (*child, false)));
        } else {
            let start = values
                .len()
                .checked_sub(children.len())
                .ok_or_else(unavailable)?;
            let proposal = step(
                cas,
                id,
                region,
                values.get(start..).ok_or_else(unavailable)?,
                budget,
            )?;
            let index = u32::try_from(steps.len()).map_err(|_| unavailable())?;
            values.truncate(start);
            values.push((index, proposal.conclusion));
            steps.push(proposal);
        }
    }
    let [(root, _)] = values.as_slice() else {
        return Err(unavailable());
    };
    Ok(CertificateRealSign {
        subject: subject.clone(),
        region: region.clone(),
        root: *root,
        steps,
    })
}
