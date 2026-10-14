// SPDX-License-Identifier: AGPL-3.0-only
//! Unproved exact domain syntax and bounded expression dependency plans for cases.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{RegionDraft, RegionExpr};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference, Vertex, size};
use crate::model::{AtomicSort, Operator, PredicateKind, Sort};
use std::collections::{BTreeMap, BTreeSet};

/// Shared case construction location.
const AT: Location = Location::new("case domain syntax");
/// Exact defined-region syntax for every reached expression.
pub(crate) type Domains = BTreeMap<LogicalId, LogicalId>;

/// Admit one ordinary untrusted draft under the caller's existing ledger.
pub(crate) fn admit(cas: &mut Cas, draft: Draft, budget: &mut Budget) -> Result<Handle, Fault> {
    cas.admit(&[draft], budget)?
        .first()
        .cloned()
        .ok_or_else(invalid)
}

/// Preserve the exact ambient/header/scope when constructing a region.
pub(crate) fn region(
    cas: &mut Cas,
    basis: LogicalId,
    expression: RegionExpr,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let value = cas.state.region(basis)?;
    let draft = RegionDraft {
        header: Reference::Existing(cas.handle(value.header)),
        scope: value.scope.clone(),
        ambient: value.ambient.clone(),
        expression,
    };
    Ok(admit(cas, Draft::Region(draft), budget)?.logical_id())
}

/// Construct a finite intersection without claiming its membership has been proved.
pub(crate) fn meet(
    cas: &mut Cas,
    basis: LogicalId,
    members: &[LogicalId],
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let members = members
        .iter()
        .map(|id| Reference::Existing(cas.handle(*id)))
        .collect();
    region(cas, basis, RegionExpr::Meet(members), budget)
}

/// Construct an exact finite union, retaining every constituent region.
pub(crate) fn union(
    cas: &mut Cas,
    basis: LogicalId,
    members: &[LogicalId],
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let members = members
        .iter()
        .map(|id| Reference::Existing(cas.handle(*id)))
        .collect();
    region(cas, basis, RegionExpr::Union(members), budget)
}

/// Form a strict predicate over a value, preserving its actual syntax header and scope.
pub(crate) fn predicate(
    cas: &mut Cas,
    term: LogicalId,
    kind: PredicateKind,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let value = cas.state.expression(term)?;
    let draft = Draft::Condition {
        header: Reference::Existing(cas.handle(value.header)),
        scope: value.scope.clone(),
        kind,
        arguments: vec![Reference::Existing(cas.handle(term))],
    };
    Ok(admit(cas, draft, budget)?.logical_id())
}

/// Retain points where the given condition is defined and true.
pub(crate) fn guarded(
    cas: &mut Cas,
    base: LogicalId,
    predicate: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let expression = RegionExpr::GuardTrue {
        base: Reference::Existing(cas.handle(base)),
        predicate: Reference::Existing(cas.handle(predicate)),
    };
    region(cas, base, expression, budget)
}

/// Turn a condition selector into its exact Boolean expression for a Defined requirement.
pub(crate) fn guard_expression(
    cas: &mut Cas,
    id: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let Vertex::Condition(value) = cas.state.node(id)? else {
        cas.state.expression(id)?;
        return Ok(id);
    };
    let draft = crate::graph::ExpressionDraft {
        header: Reference::Existing(cas.handle(value.header)),
        scope: value.scope.clone(),
        sort: Sort::Value(AtomicSort::Boolean),
        operator: Operator::Predicate(value.kind.clone()),
        binding: None,
        operands: value
            .arguments
            .iter()
            .map(|id| Reference::Existing(cas.handle(*id)))
            .collect(),
    };
    Ok(admit(cas, Draft::Expression(draft), budget)?.logical_id())
}

/// Find each value dependency once, with children preceding parents.
pub(crate) fn order(
    cas: &Cas,
    roots: &[LogicalId],
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    let mut pending: Vec<_> = roots.iter().rev().map(|id| (*id, false)).collect();
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    while let Some((id, expanded)) = pending.pop() {
        budget.work(size(seen.len(), AT)?.saturating_add(1), AT)?;
        if seen.contains(&id) {
            continue;
        }
        let value = cas.state.expression(id)?;
        if expanded {
            seen.insert(id);
            result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            result.push(id);
            continue;
        }
        let count = value
            .operands
            .len()
            .checked_add(value.arms.len())
            .ok_or_else(invalid)?;
        budget.check(
            BudgetKind::Vertices,
            size(seen.len().saturating_add(count), AT)?,
            AT,
        )?;
        pending
            .try_reserve(count.saturating_add(1))
            .map_err(|_| Fault::allocation(AT))?;
        pending.push((id, true));
        for arm in canonical(cas, &value.arms, budget)?.iter().rev() {
            pending.push((cas.state.arm(*arm)?.value, false));
        }
        pending.extend(value.operands.iter().rev().map(|id| (*id, false)));
    }
    Ok(result)
}

/// Rebuild strict definedness as set syntax before asking any region theorem checker.
pub(crate) fn domains(
    cas: &mut Cas,
    order: &[LogicalId],
    within: LogicalId,
    budget: &mut Budget,
) -> Result<Domains, Fault> {
    let mut result = Domains::new();
    for id in order {
        budget.condition(1, AT)?;
        let value = cas.state.expression(*id)?.clone();
        let domain = if let Some(domain) = value.domain {
            case_domain(cas, domain, &value.arms, within, &result, budget)?
        } else {
            scalar_domain(cas, *id, within, &result, budget)?
        };
        result.insert(*id, domain);
    }
    Ok(result)
}

/// Restrict every association's defined value to both its declared and requested domains.
fn case_domain(
    cas: &mut Cas,
    domain: LogicalId,
    arms: &[LogicalId],
    within: LogicalId,
    known: &Domains,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut pieces = Vec::new();
    pieces
        .try_reserve_exact(arms.len())
        .map_err(|_| Fault::allocation(AT))?;
    for arm in arms {
        let value = cas.state.arm(*arm)?.clone();
        let defined = *known.get(&value.value).ok_or_else(invalid)?;
        let reached = meet(cas, within, &[within, domain, value.region], budget)?;
        pieces.push(crate::case_reach::masked_meet(
            cas,
            reached,
            &[defined],
            budget,
        )?);
    }
    union(cas, within, &pieces, budget)
}

/// Strict operations retain every operand domain and their additional zero exclusions.
fn scalar_domain(
    cas: &mut Cas,
    id: LogicalId,
    within: LogicalId,
    known: &Domains,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let value = cas.state.expression(id)?.clone();
    let mut members = vec![within];
    members
        .try_reserve(value.operands.len().saturating_add(1))
        .map_err(|_| Fault::allocation(AT))?;
    for operand in &value.operands {
        members.push(*known.get(operand).ok_or_else(invalid)?);
    }
    let denominator_slot = match value.operator {
        Operator::Div => value.operands.get(1).copied(),
        Operator::IntegerPower(power) if power <= 0 => value.operands.first().copied(),
        Operator::Rational(_)
        | Operator::Free(_)
        | Operator::Bound { .. }
        | Operator::Constant(_)
        | Operator::Neg
        | Operator::Add
        | Operator::Sub
        | Operator::Mul
        | Operator::IntegerPower(_)
        | Operator::IntegerEmbed => None,
        _ => {
            let defined = predicate(cas, id, PredicateKind::Defined, budget)?;
            members.push(guarded(cas, within, defined, budget)?);
            return meet(cas, within, &members, budget);
        }
    };
    if let Some(denominator) = denominator_slot {
        members.push(nonzero(cas, denominator, within, budget)?);
    }
    meet(cas, within, &members, budget)
}

/// Sort semantic objects by complete portable structural keys, independent of admission order.
pub(crate) fn canonical(
    cas: &Cas,
    values: &[LogicalId],
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    let mut keyed = BTreeMap::new();
    let mut bytes = 0_u64;
    for id in values {
        let key = crate::wire_build::syntax_key(cas, *id, budget)?;
        bytes = bytes
            .checked_add(size(key.len(), AT)?.saturating_add(64))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.work(
            size(key.len(), AT)?.saturating_mul(size(values.len(), AT)?),
            AT,
        )?;
        keyed.insert(key, *id);
    }
    Ok(keyed.into_values().collect())
}

/// A malformed case dependency never becomes evidence.
pub(crate) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Distribute a zero exclusion through native associations without assuming a selected priority.
fn nonzero(
    cas: &mut Cas,
    root: LogicalId,
    within: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut pending = vec![(root, within)];
    let mut pieces = Vec::new();
    while let Some((id, region)) = pending.pop() {
        budget.condition(1, AT)?;
        let value = cas.state.expression(id)?.clone();
        if let Some(domain) = value.domain {
            pending
                .try_reserve(value.arms.len())
                .map_err(|_| Fault::allocation(AT))?;
            for arm in value.arms {
                let arm = cas.state.arm(arm)?.clone();
                let reached = meet(cas, within, &[region, domain, arm.region], budget)?;
                pending.push((arm.value, reached));
            }
        } else {
            let predicate = predicate(cas, id, PredicateKind::NonZero, budget)?;
            pieces.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            pieces.push(guarded(cas, region, predicate, budget)?);
        }
    }
    union(cas, within, &pieces, budget)
}
