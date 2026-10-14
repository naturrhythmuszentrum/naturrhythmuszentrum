// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable request snapshots of already checked logical equality routes.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::cost::CostPolicy;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, Vertex, size};
use crate::producer::{Plan, step};
use crate::proof_model::Relation;
use crate::rule::Primitive;
use crate::substitution::Substitution;

/// An older proof eligible under the exact pinned policy and context.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Known {
    /// Older graph proof, expanded before reuse.
    pub(crate) proof: LogicalId,
    /// Input endpoint used for matching.
    pub(crate) left: LogicalId,
    /// Root primitive for deterministic schedule ordering.
    pub(crate) primitive: Primitive,
    /// Logical cost of the entire transition, independent of expanded replay work.
    pub(crate) cost: u64,
}

/// Existing-proof accounting location.
const AT: Location = Location::new("known identity snapshot");

/// Freeze eligible old proofs before a request starts generating new applications.
pub(crate) fn snapshot(
    cas: &Cas,
    context: LogicalId,
    policy: &CostPolicy,
    budget: &mut Budget,
) -> Result<Vec<Known>, Fault> {
    let mut result = Vec::new();
    let default = *policy == CostPolicy::default();
    for id in cas.state.indices.keys() {
        budget.work(32, AT)?;
        let (proof, cost) = match cas.state.node(*id)? {
            Vertex::Proof(_) => {
                let value = crate::proof_storage::application(cas, *id)?;
                (*id, policy.weight(value.primitive)?)
            }
            Vertex::Shortcut { proof, cost } if default => (*proof, *cost),
            Vertex::PolicyShortcut {
                proof,
                cost,
                policy: identity,
            } => {
                let Vertex::Policy(previous) = cas.state.node(*identity)? else {
                    return Err(invalid());
                };
                if *previous != *policy {
                    continue;
                }
                (*proof, *cost)
            }
            _ => continue,
        };
        let value = crate::proof_storage::application(cas, proof)?;
        if value.context != context || value.relation != Relation::EqualOnDomain {
            continue;
        }
        let count = size(result.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Transitions, AT))?;
        budget.check(BudgetKind::Transitions, count, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count
                .checked_mul(128)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
            AT,
        )?;
        result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        result.push(Known {
            proof,
            left: value.left,
            primitive: value.primitive,
            cost,
        });
    }
    let count = size(result.len(), AT)?;
    budget.work(
        count
            .checked_mul(count)
            .and_then(|n| n.checked_mul(64))
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
        AT,
    )?;
    result.sort_by_key(|value| {
        (
            value.primitive.specification().key,
            value.left,
            value.cost,
            value.proof,
        )
    });
    Ok(result)
}

/// Expand a previously checked acyclic proof into untrusted local proposals.
pub(crate) fn append(
    plan: &mut Plan,
    cas: &Cas,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<u32, Fault> {
    let mut pending = vec![root];
    let mut reached = BTreeSet::new();
    while let Some(id) = pending.pop() {
        budget.work(
            size(reached.len(), AT)?
                .checked_add(1)
                .ok_or_else(invalid)?,
            AT,
        )?;
        if !reached.insert(id) {
            continue;
        }
        budget.check(BudgetKind::ProofNodes, size(reached.len(), AT)?, AT)?;
        let value = crate::proof_storage::application(cas, id)?;
        pending
            .try_reserve(value.premises.len())
            .map_err(|_| Fault::allocation(AT))?;
        for previous in &value.premises {
            if *previous >= id {
                return Err(invalid());
            }
            pending.push(*previous);
        }
    }
    let mut indices = BTreeMap::new();
    for id in reached {
        let value = crate::proof_storage::application(cas, id)?;
        let bytes = crate::proof_storage::pending_bytes(value, value.premises.len(), budget)?;
        budget.work(bytes, AT)?;
        let mut premises = Vec::new();
        premises
            .try_reserve_exact(value.premises.len())
            .map_err(|_| Fault::allocation(AT))?;
        for previous in &value.premises {
            premises.push(*indices.get(previous).ok_or_else(invalid)?);
        }
        let mut proposal = step(
            cas,
            value.primitive,
            value.relation.clone(),
            value.left,
            value.right,
            premises,
        );
        proposal.path.clone_from(&value.path);
        proposal
            .substitutions
            .try_reserve_exact(value.substitutions.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (key, term) in &value.substitutions {
            proposal.substitutions.push(Substitution {
                key: key.clone(),
                replacement: cas.handle(*term),
            });
        }
        indices.insert(id, plan.push(proposal, budget)?);
    }
    indices.get(&root).copied().ok_or_else(invalid)
}

/// Stored proof inconsistencies remain explicit errors.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
