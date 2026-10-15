// SPDX-License-Identifier: AGPL-3.0-only
//! Exact joint-predicate transport across a disjoint component split.

use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::CheckedApplication;
use crate::solving::coefficients::storage::{AT, invalid};
use crate::solving::{
    ComponentSplitData, RootRelationData, RootSelectionData, SelectionMode, SolveDraft,
};

/// Borrow the exact admitted joint selection; metadata never supplies existence.
fn selection<'a>(cas: &'a Cas, handle: &Handle) -> Result<&'a RootSelectionData<Handle>, Fault> {
    let SolveDraft::RootSelection(value) =
        crate::solving::graph::object(&cas.state, cas.checked(handle)?)?
    else {
        return Err(invalid());
    };
    Ok(value)
}

/// Borrow a relation without replacing its coefficient interpretation.
fn relation<'a>(cas: &'a Cas, handle: &Handle) -> Result<&'a RootRelationData<Handle>, Fault> {
    let SolveDraft::RootRelation(value) =
        crate::solving::graph::object(&cas.state, cas.checked(handle)?)?
    else {
        return Err(invalid());
    };
    Ok(value)
}

/// Exactly one corresponding slot changes from the left factor to the right factor.
fn pair(
    cas: &Cas,
    split: &ComponentSplitData<Handle>,
    left: &Handle,
    right: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let first = relation(cas, left)?;
    let second = relation(cas, right)?;
    if first.polynomial != split.left_factor
        || second.polynomial != split.right_factor
        || first.region != split.region
        || second.region != split.region
    {
        return Err(invalid());
    }
    if first.header != second.header
        || first.scope != second.scope
        || first.ambient != second.ambient
        || first.binder != second.binder
        || first.map != second.map
        || first.multiplicity != second.multiplicity
    {
        return Err(invalid());
    }
    let SolveDraft::CoefficientMap(mapping) =
        crate::solving::graph::object(&cas.state, first.map.logical_id())?
    else {
        return Err(invalid());
    };
    crate::solving::coefficients::proof::identity_map(cas, mapping, premises, budget)
}

/// Identical predicates preserve all links and constraints while the factors partition one root slot.
pub(crate) fn check(
    cas: &Cas,
    split: &ComponentSplitData<Handle>,
    handles: &[Handle],
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [left, right] = handles else {
        return Err(invalid());
    };
    let first = selection(cas, left)?;
    let second = selection(cas, right)?;
    if first.header != second.header
        || first.scope != second.scope
        || first.predicate != second.predicate
        || first.mode != second.mode
        || first.convention != second.convention
        || first.interval != second.interval
    {
        return Err(invalid());
    }
    if !matches!(first.mode, SelectionMode::Any | SelectionMode::Constrained)
        || first.relations.len() != second.relations.len()
    {
        return Err(invalid());
    }
    budget.solving(
        BudgetKind::SelectionSlots,
        crate::graph::size(first.relations.len(), AT)?,
        AT,
    )?;
    let mut changed = false;
    for (source, target) in first.relations.iter().zip(&second.relations) {
        budget.work(1, AT)?;
        if source == target {
            continue;
        }
        if changed {
            return Err(invalid());
        }
        pair(cas, split, source, target, premises, budget)?;
        changed = true;
    }
    if !changed {
        return Err(invalid());
    }
    Ok(())
}
