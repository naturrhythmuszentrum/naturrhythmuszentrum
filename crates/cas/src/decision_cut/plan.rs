// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded postorder over only the syntax used by the two cut premises.

use std::collections::BTreeSet;

use super::{AT, unsupported};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, LogicalId, Vertex, size};

/// Visit the acyclic region and expression closure once with an explicit bounded stack.
pub(crate) fn order(
    cas: &Cas,
    roots: &[LogicalId],
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(roots.len())
        .map_err(|_| Fault::allocation(AT))?;
    pending.extend(roots.iter().map(|id| (*id, false)));
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    while let Some((id, expanded)) = pending.pop() {
        budget.work(size(seen.len(), AT)?.saturating_add(1), AT)?;
        if seen.contains(&id) {
            continue;
        }
        budget.check(
            BudgetKind::ConditionAtoms,
            size(seen.len(), AT)?.saturating_add(1),
            AT,
        )?;
        if expanded {
            result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            seen.insert(id);
            result.push(id);
        } else {
            let retained = size(
                pending.len().saturating_add(seen.len()).saturating_add(1),
                AT,
            )?
            .saturating_mul(256);
            let children = budget
                .with_retained_payload(retained, AT, |budget| dependencies(cas, id, budget))?;
            let count = pending
                .len()
                .saturating_add(seen.len())
                .saturating_add(children.len())
                .saturating_add(2);
            budget.check(
                BudgetKind::StagedPayloadBytes,
                size(count, AT)?.saturating_mul(256),
                AT,
            )?;
            pending
                .try_reserve(children.len().saturating_add(1))
                .map_err(|_| Fault::allocation(AT))?;
            pending.push((id, true));
            pending.extend(children.into_iter().map(|child| (child, false)));
        }
    }
    Ok(result)
}

/// Retain all original dependencies, including those of unsupported expressions.
fn dependencies(cas: &Cas, id: LogicalId, budget: &mut Budget) -> Result<Vec<LogicalId>, Fault> {
    let mut result = Vec::new();
    match cas.state.node(id)? {
        Vertex::Expression(value) => copy(&mut result, &value.operands, budget)?,
        Vertex::Condition(value) => copy(&mut result, &value.arguments, budget)?,
        Vertex::Region(value) => match &value.expression {
            RegionExpr::Ambient | RegionExpr::Empty => {}
            RegionExpr::GuardTrue { base, predicate }
            | RegionExpr::GuardFalse { base, predicate } => {
                copy(&mut result, &[*base, *predicate], budget)?;
            }
            RegionExpr::Meet(values) | RegionExpr::Union(values) => {
                copy(&mut result, values, budget)?;
            }
            RegionExpr::Difference { base, excluded } => {
                copy(&mut result, &[*base, *excluded], budget)?;
            }
        },
        _ => return Err(unsupported()),
    }
    Ok(result)
}

/// Reserve dependency storage and charge visits before copying exact identities.
fn copy(
    target: &mut Vec<LogicalId>,
    source: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = size(source.len(), AT)?;
    budget.check(BudgetKind::ConditionAtoms, count, AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        count.saturating_mul(256),
        AT,
    )?;
    budget.work(count, AT)?;
    target
        .try_reserve_exact(source.len())
        .map_err(|_| Fault::allocation(AT))?;
    target.extend_from_slice(source);
    Ok(())
}
