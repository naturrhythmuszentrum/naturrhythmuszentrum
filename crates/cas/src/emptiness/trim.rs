// SPDX-License-Identifier: AGPL-3.0-only
//! Producer-only pruning shares exact regions within a fixed context and premise frame.

use super::{AT, EmptinessDag, EmptyEvidence, check, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Cas;

impl Cas {
    /// Discard dead exploration and share identical regions in one emptiness application.
    ///
    /// This supplies untrusted syntax only. The caller retains one fixed context and
    /// complete premise frame; `check_proof` independently checks the returned DAG.
    /// No key from another frame, request or store is substituted by this operation.
    /// # Errors
    /// Refuses foreign regions, forward reachable edges, empty roots and exhausted resources.
    pub fn trim_emptiness(
        &self,
        input: &EmptinessDag,
        budget: &mut Budget,
    ) -> Result<EmptinessDag, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = trim(self, input, budget);
        budget.restore_limits(previous);
        result
    }
}

/// Deduplicate only exact local regions within this application’s fixed premise frame.
fn trim(cas: &Cas, input: &EmptinessDag, budget: &mut Budget) -> Result<EmptinessDag, Fault> {
    super::measure(cas, input, budget)?;
    let reached = check::reachable(input, budget)?;
    let mut mapped = crate::solving::coefficients::storage::vector(input.nodes.len(), budget)?;
    mapped.resize(input.nodes.len(), None);
    let mut nodes = crate::solving::coefficients::storage::vector(input.nodes.len(), budget)?;
    let mut keys = std::collections::HashMap::new();
    keys.try_reserve(input.nodes.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (index, node) in input.nodes.iter().enumerate() {
        budget.work(1, AT)?;
        if !reached.get(index).copied().ok_or_else(invalid)? {
            continue;
        }
        let region = cas.checked(&node.region)?;
        cas.state.region(region)?;
        let slot = if let Some(slot) = keys.get(&region.serial) {
            *slot
        } else {
            let mut node = node.clone();
            if let EmptyEvidence::Split { children, .. } = &mut node.evidence {
                for child in children {
                    budget.work(1, AT)?;
                    *child = mapped
                        .get(usize::try_from(*child).map_err(|_| invalid())?)
                        .copied()
                        .flatten()
                        .ok_or_else(invalid)?;
                }
            }
            let slot = u32::try_from(nodes.len()).map_err(|_| invalid())?;
            nodes.push(node);
            keys.insert(region.serial, slot);
            slot
        };
        *mapped.get_mut(index).ok_or_else(invalid)? = Some(slot);
    }
    prune(
        EmptinessDag { nodes },
        mapped.last().copied().flatten().ok_or_else(invalid)?,
        budget,
    )
}

/// Sharing may close a parent before its old children; remove those newly dead nodes too.
fn prune(mut dag: EmptinessDag, root: u32, budget: &mut Budget) -> Result<EmptinessDag, Fault> {
    let length = usize::try_from(root)
        .map_err(|_| invalid())?
        .checked_add(1)
        .ok_or_else(invalid)?;
    dag.nodes.truncate(length);
    let reached = check::reachable(&dag, budget)?;
    let mut mapped = crate::solving::coefficients::storage::vector(dag.nodes.len(), budget)?;
    let mut nodes = crate::solving::coefficients::storage::vector(dag.nodes.len(), budget)?;
    for (index, mut node) in dag.nodes.into_iter().enumerate() {
        budget.work(1, AT)?;
        mapped.push(u32::try_from(nodes.len()).map_err(|_| invalid())?);
        if !reached.get(index).copied().ok_or_else(invalid)? {
            continue;
        }
        if let EmptyEvidence::Split { children, .. } = &mut node.evidence {
            for child in children {
                budget.work(1, AT)?;
                *child = *mapped
                    .get(usize::try_from(*child).map_err(|_| invalid())?)
                    .ok_or_else(invalid)?;
            }
        }
        nodes.push(node);
    }
    Ok(EmptinessDag { nodes })
}
