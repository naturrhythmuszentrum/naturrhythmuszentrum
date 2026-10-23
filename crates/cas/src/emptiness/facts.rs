// SPDX-License-Identifier: AGPL-3.0-only
//! Exact ancestral guards and checked premises supply local contradictions without search.

use super::model::EmptyFact;
use super::{AT, invalid};
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::{RealSignConclusion, SolveStatement};

/// Structural ancestors are explicit set containments, not an inferred sign partition.
pub(super) fn contained(
    cas: &Cas,
    region: &Handle,
    ancestor: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut pending = crate::solving::coefficients::storage::vector(1, budget)?;
    pending.push(cas.checked(region)?);
    let target = cas.checked(ancestor)?;
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        if id == target {
            return Ok(true);
        }
        match &cas.state.region(id)?.expression {
            RegionExpr::GuardTrue { base, .. }
            | RegionExpr::GuardFalse { base, .. }
            | RegionExpr::Difference { base, .. } => pending.push(*base),
            RegionExpr::Meet(members) => {
                crate::solving::coefficients::storage::reserve(
                    &mut pending,
                    members.len(),
                    budget,
                )?;
                pending.extend(members);
            }
            _ => {}
        }
    }
    Ok(false)
}

/// Literal zero never licenses cancellation of an undefined expression.
fn literal_zero(cas: &Cas, id: crate::graph::LogicalId) -> Result<bool, Fault> {
    let expression = cas.state.expression(id)?;
    Ok(matches!(&expression.operator, Operator::Rational(value) if value.is_zero()))
}

/// A true equality guard on any explicit ancestor makes this exact subject zero.
pub(super) fn zero(
    cas: &Cas,
    region: &Handle,
    subject: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = crate::solving::coefficients::storage::vector(1, budget)?;
    pending.push(cas.checked(region)?);
    let subject = cas.checked(subject)?;
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        match &cas.state.region(id)?.expression {
            RegionExpr::GuardTrue { base, predicate } => {
                let (kind, arguments) =
                    crate::solving::elementary::evidence::predicate(cas, *predicate)?;
                if let (PredicateKind::Equal, [left, right]) = (kind, arguments) {
                    let forward = *left == subject && literal_zero(cas, *right)?;
                    let reverse = *right == subject && literal_zero(cas, *left)?;
                    if forward || reverse {
                        return Ok(());
                    }
                }
                pending.push(*base);
            }
            RegionExpr::GuardFalse { base, .. } | RegionExpr::Difference { base, .. } => {
                pending.push(*base);
            }
            RegionExpr::Meet(members) => {
                crate::solving::coefficients::storage::reserve(
                    &mut pending,
                    members.len(),
                    budget,
                )?;
                pending.extend(members);
            }
            _ => {}
        }
    }
    Err(invalid())
}

/// The fact must name exactly this subject; sign spelling alone grants no transfer.
fn predicate(cas: &Cas, predicate: crate::graph::LogicalId, subject: &Handle) -> Result<(), Fault> {
    let (kind, arguments) = crate::solving::elementary::evidence::predicate(cas, predicate)?;
    if matches!(kind, PredicateKind::NonZero | PredicateKind::Positive)
        && arguments == [cas.checked(subject)?]
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Local premise indices refer only to independently checked applications.
pub(super) fn premise(
    premises: &[CheckedApplication],
    slot: u32,
) -> Result<&CheckedApplication, Fault> {
    premises
        .get(usize::try_from(slot).map_err(|_| invalid())?)
        .ok_or_else(invalid)
}

/// Strict signs and positive/nonzero predicates exclude zero on their exact containing region.
pub(super) fn nonzero(
    cas: &Cas,
    region: &Handle,
    subject: &Handle,
    fact: &EmptyFact,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    match fact {
        EmptyFact::Guard(ancestor) => {
            if !contained(cas, region, ancestor, budget)? {
                return Err(invalid());
            }
            let RegionExpr::GuardTrue {
                predicate: guard, ..
            } = &cas.state.region(cas.checked(ancestor)?)?.expression
            else {
                return Err(invalid());
            };
            predicate(cas, *guard, subject)
        }
        EmptyFact::Premise(slot) => match &premise(premises, *slot)?.judgment.statement {
            Statement::PredicateFact {
                region: domain,
                predicate: guard,
            } => {
                if !contained(cas, region, domain, budget)? {
                    return Err(invalid());
                }
                predicate(cas, cas.checked(guard)?, subject)
            }
            Statement::Solve(statement) => {
                let SolveStatement::RealSign(sign) = statement.as_ref() else {
                    return Err(invalid());
                };
                let strict = matches!(
                    sign.conclusion,
                    RealSignConclusion::Positive
                        | RealSignConclusion::Negative
                        | RealSignConclusion::NonZero
                );
                if sign.subject != *subject
                    || !strict
                    || !contained(cas, region, &sign.region, budget)?
                {
                    return Err(invalid());
                }
                Ok(())
            }
            _ => Err(invalid()),
        },
    }
}
