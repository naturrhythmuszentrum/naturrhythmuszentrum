// SPDX-License-Identifier: AGPL-3.0-only
//! Exact zero and nonzero facts entailed by contained regional guards.

use super::polynomial::Polynomial;
use super::storage::{AT, domain, invalid, reserve, vector};
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Handle, LogicalId, State, Vertex};
use crate::model::PredicateKind;
use crate::solving::{CoefficientDomainData, SolveDraft};

/// Only intersections and guard bases imply containment; unions and differences are not decided.
fn bases(
    expression: &RegionExpr<LogicalId>,
    pending: &mut Vec<LogicalId>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    match expression {
        RegionExpr::GuardTrue { base, .. } | RegionExpr::GuardFalse { base, .. } => {
            reserve(pending, 1, budget)?;
            pending.push(*base);
        }
        RegionExpr::Meet(members) => {
            reserve(pending, members.len(), budget)?;
            pending.extend(members);
        }
        _ => {}
    }
    Ok(())
}

/// Check containment structurally without inventing a general region decision procedure.
pub(crate) fn contained(
    state: &State,
    region: LogicalId,
    target: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut pending = vector(1, budget)?;
    pending.push(region);
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        if id == target {
            return Ok(true);
        }
        bases(&state.region(id)?.expression, &mut pending, budget)?;
    }
    Ok(false)
}

/// Equality is recognized only against exact zero; explicit nonzero has the opposite truth value.
fn guard(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    region: LogicalId,
    predicate: LogicalId,
    budget: &mut Budget,
) -> Result<Option<bool>, Fault> {
    let Vertex::Condition(condition) = state.node(predicate)? else {
        return Ok(None);
    };
    let frame = state.region(region)?;
    if (condition.header, &condition.scope) != (frame.header, &frame.scope) {
        return Ok(None);
    }
    match (&condition.kind, condition.arguments.as_slice()) {
        (PredicateKind::NonZero, [argument]) => {
            let Some(value) =
                super::guard_polynomial::read(state, source, region, *argument, budget)?
            else {
                return Ok(None);
            };
            Ok(polynomial.equals(&value, budget)?.then_some(false))
        }
        (PredicateKind::Equal, [left, right]) => {
            let Some(first) = super::guard_polynomial::read(state, source, region, *left, budget)?
            else {
                return Ok(None);
            };
            let Some(second) =
                super::guard_polynomial::read(state, source, region, *right, budget)?
            else {
                return Ok(None);
            };
            Ok(
                ((first.zero(budget)? && polynomial.equals(&second, budget)?)
                    || (second.zero(budget)? && polynomial.equals(&first, budget)?))
                .then_some(true),
            )
        }
        _ => Ok(None),
    }
}

/// Inspect exact contained guards, preserving the region's header and binder frame.
fn at_source(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    region: &Handle,
    zero: bool,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut pending = vector(1, budget)?;
    pending.push(region.logical_id());
    let frame = state.region(region.logical_id())?;
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let current = state.region(id)?;
        if (current.header, &current.scope, &current.ambient)
            != (frame.header, &frame.scope, &frame.ambient)
        {
            return Err(invalid());
        }
        let guarded = match &current.expression {
            RegionExpr::GuardTrue { predicate, .. } => Some((*predicate, true)),
            RegionExpr::GuardFalse { predicate, .. } => Some((*predicate, false)),
            _ => None,
        };
        if let Some((predicate, truth)) = guarded
            && let Some(when_true) = guard(state, source, polynomial, id, predicate, budget)?
            && (when_true == truth) == zero
        {
            return Ok(true);
        }
        bases(&current.expression, &mut pending, budget)?;
    }
    Ok(false)
}

/// Independently recompute a regional fact, including canonical parent-field inclusions.
pub(crate) fn proves(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    region: &Handle,
    zero: bool,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut current = source;
    loop {
        budget.work(1, AT)?;
        if at_source(state, current, polynomial, region, zero, budget)? {
            return Ok(true);
        }
        let CoefficientDomainData::Quotient { level } = domain(state, current)? else {
            return Ok(false);
        };
        let SolveDraft::ExtensionLevel(extension) =
            crate::solving::graph::object(state, level.logical_id())?
        else {
            return Err(invalid());
        };
        current = &extension.parent;
    }
}
