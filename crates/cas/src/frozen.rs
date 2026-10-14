// SPDX-License-Identifier: AGPL-3.0-only
//! Complete bounded shortest paths on an immutable projection of checked equalities.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::budget::{Budget, BudgetKind};
use crate::cost::CostPolicy;
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, Vertex, encode_id, size, vertex_key};
use crate::proof_model::{Claim, Relation};

/// A sealed finite-snapshot cost claim, independent of mathematical witness validity.
#[derive(Clone)]
pub struct FrozenCertificate {
    /// Exact ordered application manifest and complete cost-policy bytes.
    manifest: Vec<u8>,
    /// Complete positive policy pinned for this finite graph.
    policy: CostPolicy,
    /// None means the target was unreachable in the fully settled snapshot.
    shortest_cost: Option<u64>,
    /// All reachable endpoints were settled, including those beyond the target.
    settled: u64,
}

impl fmt::Debug for FrozenCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FrozenCertificate")
            .field("manifest_bytes", &self.manifest.len())
            .field("shortest_cost", &self.shortest_cost)
            .field("settled", &self.settled)
            .finish_non_exhaustive()
    }
}

impl FrozenCertificate {
    /// Borrow the exact immutable application manifest.
    #[must_use]
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }

    /// Read the finite snapshot's target distance, without an ungenerated-identity claim.
    #[must_use]
    pub const fn shortest_cost(&self) -> Option<u64> {
        self.shortest_cost
    }

    /// Number of reachable endpoints settled before this certificate was minted.
    #[must_use]
    pub const fn settled(&self) -> u64 {
        self.settled
    }

    /// Borrow the complete policy, including every primitive weight.
    #[must_use]
    pub const fn policy(&self) -> &CostPolicy {
        &self.policy
    }
}

/// One actual checked logical transition; arbitrary graph roles never enter this projection.
#[derive(Clone, Copy)]
pub(crate) struct Arc {
    /// Mathematical source endpoint.
    pub(crate) left: LogicalId,
    /// Mathematical destination endpoint.
    pub(crate) right: LogicalId,
    /// Positive cost in this snapshot's policy.
    pub(crate) cost: u64,
}

/// Snapshot certification location.
const AT: Location = Location::new("frozen cost certification");

impl Cas {
    /// Certify the shortest equality route through all currently admitted matching applications.
    ///
    /// This is a finite positive-weight claim, not a claim about ungenerated identities or
    /// path-dependent expansion allowances. Every reachable endpoint must settle successfully.
    ///
    /// # Errors
    /// Refuses non-equality relations, conflicting policy versions, invalid handles or any
    /// resource exhaustion. Failure produces no certificate and publishes no policy record.
    pub fn certify_frozen_cost(
        &mut self,
        claim: &Claim,
        policy: &CostPolicy,
        budget: &mut Budget,
    ) -> Result<FrozenCertificate, Fault> {
        self.proof_limits(budget, |cas, budget| {
            let mut candidate = cas.fork(budget)?;
            candidate.pin_policy(policy, budget)?;
            let (arcs, manifest) = snapshot(&candidate, claim, policy, budget)?;
            let start = candidate.checked(&claim.left)?;
            let target = candidate.checked(&claim.right)?;
            let (shortest_cost, settled) = dijkstra(&arcs, start, target, budget)?;
            cas.state = candidate.state;
            Ok(FrozenCertificate {
                manifest,
                policy: policy.clone(),
                shortest_cost,
                settled,
            })
        })
    }
}

/// Enumerate exact context/relation/residual classes, never syntactic operand edges.
pub(crate) fn snapshot(
    cas: &Cas,
    claim: &Claim,
    policy: &CostPolicy,
    budget: &mut Budget,
) -> Result<(Vec<Arc>, Vec<u8>), Fault> {
    if claim.relation != Relation::EqualOnDomain {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    let desired = crate::wire::claim_key(cas, claim, budget)?;
    validate_domain(cas, claim, budget)?;
    let context = cas.checked(&claim.context)?;
    let mut records = Vec::new();
    let mut arcs = Vec::new();
    let mut retained = 0_u64;
    let default = *policy == CostPolicy::default();
    for id in cas.state.indices.keys() {
        budget.work(1, AT)?;
        let node = cas.state.node(*id)?;
        let (application, cost) = match node {
            Vertex::Application(value) => (value, policy.weight(value.primitive)?),
            Vertex::Shortcut { proof, cost } if default => {
                (crate::proof_storage::application(cas, *proof)?, *cost)
            }
            Vertex::PolicyShortcut {
                proof,
                cost,
                policy: identity,
            } => {
                let Vertex::Policy(pinned) = cas.state.node(*identity)? else {
                    return Err(invalid());
                };
                if *pinned != *policy {
                    continue;
                }
                (crate::proof_storage::application(cas, *proof)?, *cost)
            }
            _ => continue,
        };
        if application.context != context || application.relation != claim.relation {
            continue;
        }
        let mut actual = crate::proof::claim(cas, application, budget)?;
        actual.left = claim.left.clone();
        actual.right = claim.right.clone();
        if crate::wire::claim_key(cas, &actual, budget)? != desired {
            continue;
        }
        let count = size(arcs.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Transitions, AT))?;
        budget.check(BudgetKind::Transitions, count, AT)?;
        let (_, key) = vertex_key(node, &cas.state, budget)?;
        retained = retained
            .checked_add(size(key.len(), AT)?)
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        records.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        records.push((*id, key));
        arcs.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        arcs.push(Arc {
            left: application.left,
            right: application.right,
            cost,
        });
    }
    let mut out = Encoder::output(budget, AT);
    out.raw(b"NRZCOST1")?;
    policy.encode(&mut out)?;
    out.count(desired.len())?;
    out.raw(&desired)?;
    out.count(records.len())?;
    for (id, key) in records {
        encode_id(id, &mut out)?;
        out.count(key.len())?;
        out.raw(&key)?;
    }
    Ok((arcs, out.finish()))
}

/// Even an empty path must preserve the source's complete defined domain.
fn validate_domain(cas: &Cas, claim: &Claim, budget: &mut Budget) -> Result<(), Fault> {
    let context = cas.checked(&claim.context)?;
    let mut conditions = crate::conditions::Conditions::default();
    conditions.endpoint(&cas.state, context, cas.checked(&claim.left)?, budget)?;
    let mut supplied = BTreeSet::new();
    for guard in &claim.residuals {
        budget.work(
            size(supplied.len(), AT)?
                .checked_add(1)
                .ok_or_else(invalid)?,
            AT,
        )?;
        supplied.insert(crate::conditions::Obligation {
            kind: guard.kind,
            term: cas.checked(&guard.term)?,
        });
    }
    if !conditions.residuals.is_subset(&supplied) {
        return Err(Fault::invalid(InvalidCode::ConditionMismatch, AT));
    }
    for guard in supplied {
        conditions.require(&cas.state, context, guard, budget)?;
    }
    Ok(())
}

/// Complete Dijkstra; stopping at the target would not certify the required settled snapshot.
pub(crate) fn dijkstra(
    arcs: &[Arc],
    start: LogicalId,
    target: LogicalId,
    budget: &mut Budget,
) -> Result<(Option<u64>, u64), Fault> {
    budget.check(BudgetKind::FrontierLabels, 1, AT)?;
    budget.check(BudgetKind::QueueEntries, 1, AT)?;
    let mut distances = BTreeMap::new();
    let mut pending = BTreeSet::new();
    let mut settled = BTreeSet::new();
    distances.insert(start, 0_u64);
    pending.insert((0_u64, start));
    while let Some((cost, endpoint)) = pending.pop_first() {
        let count = size(distances.len(), AT)?;
        budget.check(BudgetKind::FrontierLabels, count, AT)?;
        budget.check(BudgetKind::QueueEntries, size(pending.len(), AT)?, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count
                .checked_mul(256)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
            AT,
        )?;
        budget.work(
            count
                .checked_mul(32)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if settled.contains(&endpoint) || distances.get(&endpoint) != Some(&cost) {
            continue;
        }
        settled.insert(endpoint);
        for arc in arcs {
            budget.work(count, AT)?;
            if arc.cost == 0 {
                return Err(Fault::invalid(InvalidCode::Policy, AT));
            }
            if arc.left != endpoint {
                continue;
            }
            let proposed = cost
                .checked_add(arc.cost)
                .ok_or_else(|| Fault::capacity(BudgetKind::Transitions, AT))?;
            if distances
                .get(&arc.right)
                .is_none_or(|previous| proposed < *previous)
            {
                let queued = size(pending.len(), AT)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::QueueEntries, AT))?;
                budget.check(BudgetKind::QueueEntries, queued, AT)?;
                let vertices = count
                    .checked_add(u64::from(!distances.contains_key(&arc.right)))
                    .ok_or_else(|| Fault::capacity(BudgetKind::FrontierLabels, AT))?;
                budget.check(BudgetKind::FrontierLabels, vertices, AT)?;
                distances.insert(arc.right, proposed);
                pending.insert((proposed, arc.right));
            }
        }
    }
    Ok((distances.get(&target).copied(), size(settled.len(), AT)?))
}

/// A malformed stored relation cannot become a cost certificate.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
