// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-domain guarded conversion follows checked ordinary ancestor restrictions.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{Region, RegionExpr};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};

/// The typed capability refusal retains a dedicated diagnostic site.
const AT: Location = Location::new("scalar guarded-region transport");

/// Unsupported whole-domain conversion is distinct from malformed evidence.
const fn unsupported() -> Fault {
    Fault::invalid(InvalidCode::ScalarTransportUnsupported, AT)
}

/// Equal regions retain the existing zero-inspection path and original bytes.
pub(super) fn check(
    cas: &Cas,
    context: LogicalId,
    region: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let frame = cas.state.context(context, budget)?;
    if region == frame.admissible {
        return Ok(());
    }
    cas.state.region(region, budget)?;
    budget.check(BudgetKind::HypothesisDepth, frame.depth, AT)?;
    let previous = budget.limits().clone();
    let result = inspect(cas, context, region, budget);
    budget.restore_limits(previous);
    result
}

/// Retain each scheduled entry until traversal ends; every entry is prepaid independently.
fn schedule(
    pending: &mut Vec<(LogicalId, u64)>,
    id: LogicalId,
    depth: u64,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.check(BudgetKind::Depth, depth, AT)?;
    budget.work(96, AT)?;
    budget.reserve_temporary(96, AT)?;
    pending
        .try_reserve_exact(1)
        .map_err(|_| Fault::allocation(AT))?;
    pending.push((id, depth));
    Ok(())
}

/// Expand only unmatched meets while retaining every prepaid entry through completion.
fn inspect(
    cas: &Cas,
    context: LogicalId,
    region: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let admissible = cas.state.context(context, budget)?.admissible;
    if cas.state.region(admissible, budget)?.lies_over.is_some() {
        return Err(unsupported());
    }
    let mut pending = Vec::new();
    schedule(&mut pending, region, 1, budget)?;
    let mut cursor = 0;
    let mut anchored = false;
    while let Some(&(id, depth)) = pending.get(cursor) {
        cursor += 1;
        if id == admissible {
            anchored = true;
            continue;
        }
        if ancestor_region(cas, context, id, budget)? {
            continue;
        }
        let value = cas.state.region(id, budget)?;
        let RegionExpr::Meet(members) = &value.expression else {
            return Err(unsupported());
        };
        if value.lies_over.is_some() || members.is_empty() {
            return Err(unsupported());
        }
        let next = depth
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?;
        for member in members {
            schedule(&mut pending, *member, next, budget)?;
        }
    }
    if !anchored {
        return Err(unsupported());
    }
    Ok(())
}

/// Every crossed edge must reconstruct the same unprojected canonical restriction meet.
/// Ineligible edges stop this probe; category and resource faults still propagate.
fn ancestor_region(
    cas: &Cas,
    context: LogicalId,
    region: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut child = visit(cas, context, budget)?;
    loop {
        if child.admissible == region {
            return Ok(true);
        }
        let Some(parent_id) = child.parent else {
            return Ok(false);
        };
        let parent = visit(cas, parent_id, budget)?;
        let Some(restriction) = child.restriction else {
            return Ok(false);
        };
        let local = cas.state.region(child.admissible, budget)?;
        let outer = cas.state.region(parent.admissible, budget)?;
        let restricted = cas.state.region(restriction, budget)?;
        for value in [local, outer, restricted] {
            if value.lies_over.is_some() {
                return Ok(false);
            }
            prepay_metadata(value, budget)?;
        }
        let header = child.header_id;
        if header != parent.header_id
            || local.header != header
            || outer.header != header
            || restricted.header != header
            || local.scope != outer.scope
            || local.scope != restricted.scope
            || local.ambient != outer.ambient
            || local.ambient != restricted.ambient
        {
            return Ok(false);
        }
        let RegionExpr::Meet(members) = &local.expression else {
            return Ok(false);
        };
        // Canonical admission sorts and deduplicates this fixed two-input constructor.
        let mut expected = [parent.admissible, restriction];
        expected.sort_unstable();
        let expected = if expected[0] == expected[1] {
            &expected[..1]
        } else {
            &expected[..]
        };
        budget.work(size(expected.len(), AT)?, AT)?;
        if members.as_slice() != expected {
            return Ok(false);
        }
        child = parent;
    }
}

/// Pay before inspecting a frame; a crossed parent carries this payment into the next iteration.
fn visit<'a>(
    cas: &'a Cas,
    context: LogicalId,
    budget: &mut Budget,
) -> Result<&'a crate::graph::Context, Fault> {
    budget.condition(1, AT)?;
    let frame = cas.state.context(context, budget)?;
    budget.check(BudgetKind::HypothesisDepth, frame.depth, AT)?;
    Ok(frame)
}

/// Metadata is borrowed; its complete comparison is charged before equality testing.
fn prepay_metadata(value: &Region, budget: &mut Budget) -> Result<(), Fault> {
    let mut out = crate::encoding::Encoder::new(budget, AT);
    crate::encoding::scope(&value.scope, &mut out)?;
    crate::domain_encoding::ambient(&value.ambient, &mut out)?;
    let bytes = size(out.finish().len(), AT)?;
    budget.work(bytes, AT)
}
