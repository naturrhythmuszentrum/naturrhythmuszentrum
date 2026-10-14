// SPDX-License-Identifier: AGPL-3.0-only
//! Exact reach masks keep inactive branch functions out of local value decisions.

use crate::budget::Budget;
use crate::case_syntax::{self, invalid};
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, LogicalId};
use std::collections::BTreeMap;

/// Joined reached region of each value dependency.
pub(crate) type Reaches = BTreeMap<LogicalId, LogicalId>;

/// Propagate applicability from roots to strict operands and native association values.
pub(crate) fn regions(
    cas: &mut Cas,
    roots: &[LogicalId],
    order: &[LogicalId],
    within: LogicalId,
    budget: &mut Budget,
) -> Result<Reaches, Fault> {
    let mut reaches = Reaches::new();
    for root in roots {
        reaches.insert(*root, within);
    }
    for id in order.iter().rev() {
        let active = *reaches.get(id).ok_or_else(invalid)?;
        let value = cas.state.expression(*id)?.clone();
        for operand in value.operands {
            add(cas, &mut reaches, operand, active, within, budget)?;
        }
        if let Some(domain) = value.domain {
            for arm in case_syntax::canonical(cas, &value.arms, budget)? {
                let arm = cas.state.arm(arm)?.clone();
                let region = masked_meet(cas, active, &[domain, arm.region], budget)?;
                add(cas, &mut reaches, arm.value, region, within, budget)?;
            }
        }
    }
    Ok(reaches)
}

/// Accumulate all uses of shared value syntax without losing a reachable branch.
fn add(
    cas: &mut Cas,
    reaches: &mut Reaches,
    value: LogicalId,
    region: LogicalId,
    within: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let region = if let Some(previous) = reaches.get(&value) {
        if *previous == region {
            region
        } else {
            case_syntax::union(cas, within, &[*previous, region], budget)?
        }
    } else {
        region
    };
    reaches.insert(value, region);
    Ok(())
}

/// Proved empty reach is discharged before touching a potentially unsupported branch predicate.
pub(crate) fn masked_meet(
    cas: &mut Cas,
    reached: LogicalId,
    members: &[LogicalId],
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    match crate::case_obligations::empty(cas, reached, budget) {
        Ok(true) => return case_syntax::region(cas, reached, RegionExpr::Empty, budget),
        Err(fault) if !crate::case_obligations::unknown(&fault) => return Err(fault),
        _ => {}
    }
    let mut all = vec![reached];
    all.try_reserve(members.len())
        .map_err(|_| Fault::allocation(crate::error::Location::new("case reach")))?;
    all.extend_from_slice(members);
    case_syntax::meet(cas, reached, &all, budget)
}
