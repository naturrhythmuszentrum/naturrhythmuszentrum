// SPDX-License-Identifier: AGPL-3.0-only
//! Projection is a checked copy of predicates, never authority from an explicit projection link.

use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId};
use crate::proof_model::{CheckedApplication, Statement};

/// Recognize only structural containment under the same coordinate interpretation.
pub(crate) fn subset(
    cas: &Cas,
    left: LogicalId,
    right: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let first = cas.state.region(left)?;
    let second = cas.state.region(right)?;
    if first.ambient != second.ambient
        || first.scope != second.scope
        || first.header != second.header
    {
        return Ok(false);
    }
    if let RegionExpr::Meet(members) = &second.expression {
        for member in members {
            if !subset(cas, left, *member, budget)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    let mut pending = vec![(left, 1_u64)];
    while let Some((id, depth)) = pending.pop() {
        budget.work(1, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        let value = cas.state.region(id)?;
        if id == right
            || matches!(value.expression, RegionExpr::Empty)
            || matches!(second.expression, RegionExpr::Ambient)
            || lifted(cas, id, right, budget)?
        {
            return Ok(true);
        }
        match &value.expression {
            RegionExpr::GuardTrue { base, .. }
            | RegionExpr::GuardFalse { base, .. }
            | RegionExpr::Difference { base, .. } => pending.push((*base, depth.saturating_add(1))),
            RegionExpr::Meet(parts) => {
                crate::solving::coefficients::storage::reserve(&mut pending, parts.len(), budget)?;
                pending.extend(parts.iter().map(|id| (*id, depth.saturating_add(1))));
            }
            _ => {}
        }
    }
    Ok(false)
}

/// The total region must actually contain a predicate-for-predicate lift of its stratum.
pub(crate) fn projection(
    cas: &Cas,
    total: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let value = cas.state.region(cas.checked(total)?)?;
    let over = value.lies_over.as_ref().ok_or_else(invalid)?;
    for premise in premises {
        budget.work(1, AT)?;
        if let Statement::Subset {
            subset: source,
            superset: target,
        } = &premise.judgment.statement
            && source == total
            && lifted(cas, over.stratum, target.logical_id(), budget)?
        {
            return Ok(());
        }
    }
    Err(invalid())
}

/// Compare the complete Boolean constructor tree; predicates retain their original declarations.
pub(crate) fn lifted(
    cas: &Cas,
    base: LogicalId,
    total: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut pending = vec![(base, total, 1_u64)];
    while let Some((source, target, depth)) = pending.pop() {
        budget.work(1, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        let left = cas.state.region(source)?;
        let right = cas.state.region(target)?;
        if left.header != right.header || left.scope != right.scope {
            return Ok(false);
        }
        match (&left.expression, &right.expression) {
            (RegionExpr::Ambient, RegionExpr::Ambient) | (RegionExpr::Empty, RegionExpr::Empty) => {
            }
            (
                RegionExpr::GuardTrue {
                    base: a,
                    predicate: p,
                },
                RegionExpr::GuardTrue {
                    base: b,
                    predicate: q,
                },
            )
            | (
                RegionExpr::GuardFalse {
                    base: a,
                    predicate: p,
                },
                RegionExpr::GuardFalse {
                    base: b,
                    predicate: q,
                },
            ) if p == q => pending.push((*a, *b, depth.saturating_add(1))),
            (RegionExpr::Meet(a), RegionExpr::Meet(b))
            | (RegionExpr::Union(a), RegionExpr::Union(b))
                if a.len() == b.len() =>
            {
                crate::solving::coefficients::storage::reserve(&mut pending, a.len(), budget)?;
                pending.extend(
                    a.iter()
                        .zip(b)
                        .map(|(a, b)| (*a, *b, depth.saturating_add(1))),
                );
            }
            (
                RegionExpr::Difference {
                    base: a,
                    excluded: p,
                },
                RegionExpr::Difference {
                    base: b,
                    excluded: q,
                },
            ) => {
                pending.extend([
                    (*a, *b, depth.saturating_add(1)),
                    (*p, *q, depth.saturating_add(1)),
                ]);
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// Recover one exact family from region ancestry, rejecting mixed-family intersections.
pub(crate) fn family(cas: &Cas, root: LogicalId, budget: &mut Budget) -> Result<LogicalId, Fault> {
    family_state(&cas.state, root, budget)
}

/// Shared admission follows exactly the same family ancestry.
pub(crate) fn family_state(
    state: &crate::graph::State,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut pending = vec![root];
    let mut found = None;
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let value = state.region(id)?;
        if let Some(over) = &value.lies_over {
            if found.is_some_and(|family| family != over.family) {
                return Err(invalid());
            }
            found = Some(over.family);
        }
        match &value.expression {
            RegionExpr::GuardTrue { base, .. }
            | RegionExpr::GuardFalse { base, .. }
            | RegionExpr::Difference { base, .. } => pending.push(*base),
            RegionExpr::Meet(parts) => {
                crate::solving::coefficients::storage::reserve(&mut pending, parts.len(), budget)?;
                pending.extend(parts);
            }
            _ => {}
        }
    }
    found.ok_or_else(invalid)
}

/// A complete family piece supplies a singleton cover only by exact structural containment.
pub(crate) fn identity_cover(
    cas: &Cas,
    domain: &Handle,
    members: &[Handle],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let [member] = members else {
        return Ok(false);
    };
    if family(cas, domain.logical_id(), budget)? != family(cas, member.logical_id(), budget)? {
        return Ok(false);
    }
    subset(cas, domain.logical_id(), member.logical_id(), budget)
}
