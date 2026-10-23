// SPDX-License-Identifier: AGPL-3.0-only
//! Each reachable node and evidence edge is checked once, independently of discovery.

use super::{AT, EmptinessDag, EmptyEvidence, facts, invalid, leaves};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Cas;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};

/// Reject forward edges and dead exploration with a linear backward reachability pass.
pub(super) fn reachable(dag: &EmptinessDag, budget: &mut Budget) -> Result<Vec<bool>, Fault> {
    let mut reached = crate::solving::coefficients::storage::vector(dag.nodes.len(), budget)?;
    reached.resize(dag.nodes.len(), false);
    *reached.last_mut().ok_or_else(invalid)? = true;
    for (index, node) in dag.nodes.iter().enumerate().rev() {
        budget.work(1, AT)?;
        if !reached.get(index).copied().ok_or_else(invalid)? {
            continue;
        }
        if let EmptyEvidence::Split { children, .. } = &node.evidence {
            for child in children {
                budget.work(1, AT)?;
                let child = usize::try_from(*child).map_err(|_| invalid())?;
                if child >= index {
                    return Err(invalid());
                }
                *reached.get_mut(child).ok_or_else(invalid)? = true;
            }
        }
    }
    Ok(reached)
}

/// A supplied cover must name this exact parent and every explicitly proved-empty child.
fn split(
    premises: &[CheckedApplication],
    dag: &EmptinessDag,
    index: usize,
    cover: u32,
    children: &[u32],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let node = dag.nodes.get(index).ok_or_else(invalid)?;
    let Statement::Cover { domain, members } = &facts::premise(premises, cover)?.judgment.statement
    else {
        return Err(invalid());
    };
    if *domain != node.region || members.len() != children.len() {
        return Err(invalid());
    }
    let mut regions = std::collections::HashSet::new();
    regions
        .try_reserve(members.len())
        .map_err(|_| Fault::allocation(AT))?;
    for member in members {
        budget.work(1, AT)?;
        if !regions.insert(member.logical_id().serial) {
            return Err(invalid());
        }
    }
    for child in children {
        budget.work(1, AT)?;
        let child = usize::try_from(*child).map_err(|_| invalid())?;
        if child >= index {
            return Err(invalid());
        }
        let region = &dag.nodes.get(child).ok_or_else(invalid)?.region;
        if !regions.remove(&region.logical_id().serial) {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Conditions from every checked premise remain attached; no citation erases an assumption.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let (Statement::Empty { region }, Certificate::Emptiness(dag)) =
        (&draft.conclusion, &draft.certificate)
    else {
        return Err(invalid());
    };
    if !draft.substitutions.is_empty() || dag.nodes.last().ok_or_else(invalid)?.region != *region {
        return Err(invalid());
    }
    if reachable(dag, budget)?.iter().any(|reached| !reached) {
        return Err(invalid());
    }
    let mut conditions = ScopedConditions::default();
    for premise in premises {
        if premise.judgment.context != draft.context {
            return Err(invalid());
        }
        crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
    }
    for (index, node) in dag.nodes.iter().enumerate() {
        budget.work(1, AT)?;
        cas.state.region(cas.checked(&node.region)?)?;
        match &node.evidence {
            EmptyEvidence::Split { cover, children } => {
                split(premises, dag, index, *cover, children, budget)?;
            }
            EmptyEvidence::Leaf(leaf) => leaves::check(cas, &node.region, leaf, premises, budget)?,
            EmptyEvidence::Lemma { premise } => {
                let expected = Statement::Empty {
                    region: node.region.clone(),
                };
                if facts::premise(premises, *premise)?.judgment.statement != expected {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(conditions)
}
