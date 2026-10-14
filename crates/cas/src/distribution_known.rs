// SPDX-License-Identifier: AGPL-3.0-only
//! Older native equality routes retain their full typed judgment and replay expansion.

use crate::budget::{Budget, BudgetKind};
use crate::cost::CostPolicy;
use crate::distribution_model::DistributionStatement;
use crate::distribution_syntax::{AT, invalid};
use crate::error::Fault;
use crate::graph::{Cas, LogicalId, Vertex, size};
use crate::proof_model::{Judgment, Statement};
use crate::rule::Primitive;

/// An immutable older typed equality and its exact logical transition cost.
#[derive(Clone, Debug)]
pub(crate) struct Known {
    /// Full replayable primitive expansion.
    pub(crate) proof: LogicalId,
    /// Exact input distribution.
    pub(crate) left: LogicalId,
    /// Exact output distribution.
    pub(crate) right: LogicalId,
    /// Root schema for stable producer ordering.
    pub(crate) rule: Primitive,
    /// Positive policy-specific transition cost.
    pub(crate) cost: u64,
    /// Complete theorem class, including layer, space and residuals.
    pub(crate) judgment: Judgment,
    /// Portable expansion bytes supply the allocation-independent final tie breaker.
    pub(crate) encoding: Vec<u8>,
}

/// Enumerate before generation; no in-round proof can retrospectively enter this list.
pub(crate) fn snapshot(
    cas: &Cas,
    context: LogicalId,
    policy: &CostPolicy,
    budget: &mut Budget,
) -> Result<Vec<Known>, Fault> {
    let mut result = Vec::new();
    let mut retained = 0_u64;
    for id in cas.state.indices.keys() {
        budget.work(1, AT)?;
        let Some((proof, shortcut)) = candidate(cas, *id, policy)? else {
            continue;
        };
        let application = crate::proof_view::application(cas, proof, budget)?;
        if application.judgment.context.logical_id() != context {
            continue;
        }
        let Statement::Distribution(value) = &application.judgment.statement else {
            continue;
        };
        let DistributionStatement::Equality { left, right, .. } = value.as_ref() else {
            continue;
        };
        budget.check(
            BudgetKind::Transitions,
            size(result.len().saturating_add(1), AT)?,
            AT,
        )?;
        let encoding = crate::wire::encode(cas, proof, budget)?;
        retained = retained
            .checked_add(size(encoding.len(), AT)?)
            .and_then(|bytes| bytes.checked_add(256))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        result.push(Known {
            proof,
            left: left.logical_id(),
            right: right.logical_id(),
            rule: application.rule,
            cost: shortcut.unwrap_or(policy.weight(application.rule)?),
            judgment: application.judgment,
            encoding,
        });
    }
    result.sort_by(|left, right| {
        (left.rule.specification().key, left.cost, &left.encoding).cmp(&(
            right.rule.specification().key,
            right.cost,
            &right.encoding,
        ))
    });
    Ok(result)
}

/// Resolve a policy-matching proof without using the scalar application projection.
fn candidate(
    cas: &Cas,
    id: LogicalId,
    policy: &CostPolicy,
) -> Result<Option<(LogicalId, Option<u64>)>, Fault> {
    Ok(match cas.state.node(id)? {
        Vertex::Proof(_) => Some((id, None)),
        Vertex::Shortcut { proof, cost } if *policy == CostPolicy::default() => {
            Some((*proof, Some(*cost)))
        }
        Vertex::PolicyShortcut {
            proof,
            cost,
            policy: identity,
            ..
        } => {
            let Vertex::Policy(actual) = cas.state.node(*identity)? else {
                return Err(invalid());
            };
            if actual == policy {
                Some((*proof, Some(*cost)))
            } else {
                None
            }
        }
        _ => None,
    })
}

/// Count every distinct primitive in a retained expansion, separately from logical transitions.
pub(crate) fn expanded_steps(
    cas: &Cas,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let mut pending = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        if !seen.insert(id) {
            continue;
        }
        budget.check(BudgetKind::ProofNodes, size(seen.len(), AT)?, AT)?;
        let application = crate::proof_view::application(cas, id, budget)?;
        pending
            .try_reserve(application.premises.len())
            .map_err(|_| Fault::allocation(AT))?;
        pending.extend(application.premises);
    }
    size(seen.len(), AT)
}

/// Compare exact theorem classes by replacing only the equality endpoints.
pub(crate) fn same_class(
    cas: &Cas,
    first: &Judgment,
    second: &Judgment,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut actual = first.clone();
    let Statement::Distribution(desired) = &second.statement else {
        return Ok(false);
    };
    let Statement::Distribution(value) = &mut actual.statement else {
        return Ok(false);
    };
    let (
        DistributionStatement::Equality { left, right, .. },
        DistributionStatement::Equality {
            left: target_left,
            right: target_right,
            ..
        },
    ) = (value.as_mut(), desired.as_ref())
    else {
        return Ok(false);
    };
    left.clone_from(target_left);
    right.clone_from(target_right);
    Ok(crate::wire_build::judgment_key(cas, &actual, budget)?
        == crate::wire_build::judgment_key(cas, second, budget)?)
}
