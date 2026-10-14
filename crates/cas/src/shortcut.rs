// SPDX-License-Identifier: AGPL-3.0-only
//! Policy-specific shortcut admission by replay and strict known-route improvement.

use crate::budget::{Budget, BudgetKind};
use crate::cost::CostPolicy;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, Handle, Vertex};
use crate::scalar_model::{Claim, VerifiedWitness};

impl Cas {
    /// Admit a replayed equality shortcut only after recomputing improvement in this policy.
    ///
    /// Old improvement records from another policy are excluded. All proof dependencies already
    /// exist, so a new shortcut cannot depend on itself or on a later registration.
    ///
    /// # Errors
    /// Refuses changed claims, conflicting policy identities, nonpositive costs, a non-improving
    /// route, non-equality relations or any resource limit before publication.
    pub(crate) fn register_scalar_derived_with_policy(
        &mut self,
        witness: &VerifiedWitness,
        expected: &Claim,
        policy: &CostPolicy,
        cost: u64,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let at = Location::new("policy shortcut");
        self.proof_limits(budget, |cas, budget| {
            if cost == 0 || cost > 1_000_000 {
                return Err(Fault::invalid(InvalidCode::Policy, at));
            }
            let proof = cas.checked(&witness.proof)?;
            let desired = crate::wire::claim_key(cas, expected, budget)?;
            let actual = crate::wire::claim_key(cas, &witness.claim, budget)?;
            budget.work(
                u64::try_from(desired.len()).map_err(|_| Fault::capacity(BudgetKind::Work, at))?,
                at,
            )?;
            if actual != desired {
                return Err(Fault::invalid(InvalidCode::EndpointMismatch, at));
            }
            cas.recheck_witness(witness, budget)?;
            let mut candidate = cas.fork(budget)?;
            let identity = candidate.pin_policy(policy, budget)?;
            let (arcs, _) = crate::frozen::snapshot(&candidate, expected, policy, budget)?;
            let left = candidate.checked(&expected.left)?;
            let right = candidate.checked(&expected.right)?;
            let (incumbent, _) = crate::frozen::dijkstra(&arcs, left, right, budget)?;
            if incumbent.is_some_and(|previous| cost >= previous) {
                return Err(Fault::invalid(InvalidCode::Policy, at));
            }
            let context = candidate.checked(&expected.context)?;
            let mut state = candidate.state;
            let id = cas.insert(
                &mut state,
                Vertex::PolicyShortcut {
                    proof,
                    cost,
                    policy: identity,
                },
                &[
                    (proof, Edge::DependsOn),
                    (identity, Edge::CostPolicy),
                    (context, Edge::InContext),
                    (left, Edge::ConclusionLeft),
                    (right, Edge::ConclusionRight),
                ],
                0,
                budget,
            )?;
            cas.state = state;
            Ok(cas.handle(id))
        })
    }
}
