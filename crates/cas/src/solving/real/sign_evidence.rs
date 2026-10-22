// SPDX-License-Identifier: AGPL-3.0-only
//! Exact identities, local trichotomy facts and prior Real root constraints.

use super::sign::{entails, mask, negate};
use super::{AT, invalid};
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, ExpressionAttachment, Handle, LogicalId};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{CheckedApplication, DomainContract, Statement};
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::elementary::{evidence, proof::premise, selected};
use crate::solving::{RealSignConclusion, RealSignStep, SolveStatement};

/// Interpret only the exact expression, its negation or its equality with literal zero.
fn predicate_mask(cas: &Cas, predicate: LogicalId, subject: &Handle) -> Result<u8, Fault> {
    let (kind, arguments) = evidence::predicate(cas, predicate)?;
    match (kind, arguments) {
        (
            PredicateKind::Positive | PredicateKind::NonNegative | PredicateKind::NonZero,
            [argument],
        ) => {
            let result = match kind {
                PredicateKind::Positive => 4,
                PredicateKind::NonNegative => 6,
                _ => 5,
            };
            if *argument == subject.logical_id() {
                return Ok(result);
            }
            let expression = cas.state.expression(*argument)?;
            if expression.operator == Operator::Neg && expression.operands == [subject.logical_id()]
            {
                return Ok(negate(result));
            }
        }
        (PredicateKind::Equal, [left, right]) => {
            let other = if *left == subject.logical_id() {
                *right
            } else if *right == subject.logical_id() {
                *left
            } else {
                return Err(invalid());
            };
            if matches!(&cas.state.expression(other)?.operator, Operator::Rational(value) if value.is_zero())
            {
                return Ok(2);
            }
        }
        _ => {}
    }
    Err(invalid())
}

/// A sign theorem may expose its precise ordinary predicate without losing its own statement shape.
pub(super) fn predicate(
    cas: &Cas,
    predicate: &Handle,
    subject: &Handle,
    actual: u8,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    if actual & !predicate_mask(cas, cas.checked(predicate)?, subject)? != 0 {
        return Err(invalid());
    }
    Ok(())
}

/// Read the same stratum only; a supplied sibling fact never qualifies.
fn fact(cas: &Cas, statement: &Statement, region: &Handle, subject: &Handle) -> Result<u8, Fault> {
    match statement {
        Statement::PredicateFact {
            region: domain,
            predicate,
        } if domain == region => predicate_mask(cas, predicate.logical_id(), subject),
        Statement::Solve(statement) => match statement.as_ref() {
            SolveStatement::RealSign(sign)
                if sign.region == *region && sign.subject == *subject =>
            {
                Ok(mask(sign.conclusion))
            }
            _ => Err(invalid()),
        },
        _ => Err(invalid()),
    }
}

/// Only actual ancestor region constructors supply implicit local trichotomy facts.
fn local(
    cas: &Cas,
    region: &Handle,
    subject: &Handle,
    claimed: RealSignConclusion,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = vector(1, budget)?;
    pending.push(region.logical_id());
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        match &cas.state.region(id)?.expression {
            RegionExpr::GuardTrue { base, predicate }
            | RegionExpr::GuardFalse { base, predicate } => {
                if let Ok(mut possible) = predicate_mask(cas, *predicate, subject) {
                    if matches!(
                        cas.state.region(id)?.expression,
                        RegionExpr::GuardFalse { .. }
                    ) {
                        possible ^= 7;
                    }
                    if entails(possible, claimed).is_ok() {
                        return Ok(());
                    }
                }
                reserve(&mut pending, 1, budget)?;
                pending.push(*base);
            }
            RegionExpr::Meet(members) => {
                reserve(&mut pending, members.len(), budget)?;
                pending.extend(members);
            }
            _ => {}
        }
    }
    Err(invalid())
}

/// Hypotheses retain totality, exact region and explicit provenance through inherited conditions.
pub(super) fn hypothesis(
    cas: &Cas,
    region: &Handle,
    step: &RealSignStep,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if !step.arguments.is_empty() || step.exponent != 0 {
        return Err(invalid());
    }
    if let [slot] = step.premises.as_slice()
        && matches!(&premise(premises, *slot)?.judgment.statement, Statement::Solve(statement)
            if matches!(statement.as_ref(), SolveStatement::RealSign(sign)
                if sign.region == *region && sign.subject == step.subject))
    {
        return entails(
            fact(
                cas,
                &premise(premises, *slot)?.judgment.statement,
                region,
                &step.subject,
            )?,
            step.conclusion,
        );
    }
    crate::solving::elementary::total::check(
        cas,
        step.subject.logical_id(),
        region,
        premises,
        budget,
    )?;
    match step.premises.as_slice() {
        [] => local(cas, region, &step.subject, step.conclusion, budget),
        [slot] => entails(
            fact(
                cas,
                &premise(premises, *slot)?.judgment.statement,
                region,
                &step.subject,
            )?,
            step.conclusion,
        ),
        _ => Err(invalid()),
    }
}

/// Identity transport accepts precisely a previously checked defined equality on this stratum.
pub(super) fn identity(
    cas: &Cas,
    region: &Handle,
    step: &RealSignStep,
    arguments: &[&RealSignStep],
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if arguments.is_empty() {
        return normalized(cas, region, step, premises, budget);
    }
    let ([source], [slot]) = (arguments, step.premises.as_slice()) else {
        return Err(invalid());
    };
    if step.exponent != 0 {
        return Err(invalid());
    }
    let Statement::Equality {
        left,
        right,
        domain,
        domain_contract: DomainContract::DefinedAndEqual,
    } = &premise(premises, *slot)?.judgment.statement
    else {
        return Err(invalid());
    };
    let forward = *left == source.subject && *right == step.subject;
    let backward = *right == source.subject && *left == step.subject;
    if domain != region || !(forward || backward) {
        return Err(invalid());
    }
    super::sign::scalar(cas, &source.subject, region)?;
    entails(mask(source.conclusion), step.conclusion)
}

/// A positive/nonnegative selected square root has its actual bound-slot predicate and prior existence.
pub(super) fn root(
    cas: &Cas,
    region: &Handle,
    step: &RealSignStep,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if !step.arguments.is_empty() || step.exponent != 0 {
        return Err(invalid());
    }
    let [slot] = step.premises.as_slice() else {
        return Err(invalid());
    };
    let expression = cas.state.expression(step.subject.logical_id())?;
    let (Operator::RootValue { slot: 0 }, Some(ExpressionAttachment::SelectedRoot(selection))) =
        (&expression.operator, &expression.attachment)
    else {
        return Err(invalid());
    };
    let Statement::Solve(statement) = &premise(premises, *slot)?.judgment.statement else {
        return Err(invalid());
    };
    if !matches!(statement.as_ref(), SolveStatement::RootExistence(value)
        if value.region == *region && value.relation.logical_id() == *selection)
    {
        return Err(invalid());
    }
    let selection = selected::selection(cas, &cas.handle(*selection))?;
    if selection.relations.len() != 1 {
        return Err(invalid());
    }
    let strict = crate::solving::towers::real_roots::positive_selection(cas, selection, budget)?;
    entails(if strict { 4 } else { 6 }, step.conclusion)
}

/// An explicit prior unit chooses the coefficient algebra; exact evaluation checks the zero identity.
fn normalized(
    cas: &Cas,
    region: &Handle,
    step: &RealSignStep,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [slot] = step.premises.as_slice() else {
        return Err(invalid());
    };
    if step.exponent != 0 {
        return Err(invalid());
    }
    let Statement::Solve(statement) = &premise(premises, *slot)?.judgment.statement else {
        return Err(invalid());
    };
    let SolveStatement::Unit(unit) = statement.as_ref() else {
        return Err(invalid());
    };
    if unit.region != *region {
        return Err(invalid());
    }
    let evaluation = crate::solving::elementary::evaluate::Evaluation {
        cas,
        engine: crate::solving::coefficients::arithmetic::Engine {
            state: &cas.state,
            domain: &unit.domain,
        },
        region,
        premises,
        free: &[],
        bound: &[],
    };
    let value = evaluation.expression(step.subject.logical_id(), true, budget)?;
    evaluation.zero(&value, budget)?;
    entails(2, step.conclusion)
}
