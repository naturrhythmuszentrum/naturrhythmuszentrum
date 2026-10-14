// SPDX-License-Identifier: AGPL-3.0-only
//! Graph publication of checked applications, proofs and improving derived shortcuts.

use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{
    Cas, Condition, Edge, Handle, LogicalId, State, Vertex, VertexKind, encode_id, size,
};
use crate::rule_check::mismatch;
use crate::scalar_model::{CheckedStep, Claim, Compiled, VerifiedWitness};

/// Static location for checked proof storage.
const AT: Location = Location::new("checked proof storage");

/// Encode every graph-owned application field for interning and storage accounting.
pub(crate) fn encode_application(value: &CheckedStep, out: &mut Encoder<'_>) -> Result<(), Fault> {
    let specification = value.primitive.specification();
    out.name(specification.key)?;
    out.count(usize::try_from(specification.version).map_err(|_| mismatch())?)?;
    crate::wire::encode_relation(&value.relation, out)?;
    for id in [value.context, value.left, value.right] {
        encode_id(id, out)?;
    }
    out.count(value.premises.len())?;
    for id in &value.premises {
        encode_id(*id, out)?;
    }
    out.count(value.path.len())?;
    for position in &value.path {
        out.count(usize::try_from(*position).map_err(|_| mismatch())?)?;
    }
    out.count(value.substitutions.len())?;
    for (key, id) in &value.substitutions {
        out.name(key)?;
        encode_id(*id, out)?;
    }
    if let Some(certificate) = &value.certificate {
        certificate.encode(out)?;
    }
    for conditions in [&value.conditions.requirements, &value.conditions.residuals] {
        out.count(conditions.len())?;
        for condition in conditions {
            out.byte(match condition.kind {
                crate::conditions::GuardKind::Defined => 0,
                crate::conditions::GuardKind::NonZero => 1,
            })?;
            encode_id(condition.term, out)?;
        }
    }
    out.count(value.conditions.hypotheses.len())?;
    for key in &value.conditions.hypotheses {
        out.name(key)?;
    }
    out.unsigned(value.depth)?;
    out.unsigned(value.cost)
}

/// Measure pending application weights, owned set keys and not-yet-assigned premise IDs.
pub(crate) fn pending_bytes(
    step: &CheckedStep,
    premises: usize,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let retained = match &step.certificate {
        Some(value) => value.payload(budget)?,
        None => 0,
    };
    let mut out = Encoder::new(budget, AT);
    encode_application(step, &mut out)?;
    let bytes = size(out.finish().len(), AT)?
        .checked_mul(3)
        .and_then(|n| n.checked_add(256))
        .and_then(|n| n.checked_add(retained))
        .and_then(|n| n.checked_add(size(premises, AT).ok()?.checked_mul(32)?))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}

/// Follow only the designated justification role, never arbitrary graph paths.
pub(crate) fn application(cas: &Cas, proof: LogicalId) -> Result<&CheckedStep, Fault> {
    let Vertex::Proof(id) = cas.state.node(proof)? else {
        return Err(mismatch());
    };
    match cas.state.node(*id)? {
        Vertex::Application(value) => Ok(value),
        Vertex::ScopedApplication(value) => value.scalar.as_ref().ok_or_else(mismatch),
        _ => Err(mismatch()),
    }
}

/// Stable checked position conversion.
fn position(index: usize) -> Result<u32, Fault> {
    u32::try_from(index).map_err(|_| Fault::capacity(BudgetKind::Edges, AT))
}

impl Cas {
    /// Publish into an already private transaction candidate; its caller owns rollback.
    /// Move that candidate's state instead of allocating a second complete graph copy.
    pub(crate) fn publish_proof(
        &mut self,
        compiled: &Compiled,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let mut pending = 0_u64;
        for (step, premises) in compiled.steps.iter().zip(&compiled.premises) {
            pending = pending
                .checked_add(pending_bytes(step, premises.len(), budget)?)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        }
        let peak = self
            .counts()
            .payload_bytes
            .checked_add(pending)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, peak, AT)?;
        let mut staged = std::mem::replace(&mut self.state, State::new());
        let mut proofs = Vec::new();
        proofs
            .try_reserve_exact(compiled.steps.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (step, local_premises) in compiled.steps.iter().zip(&compiled.premises) {
            let mut out = Encoder::new(budget, AT);
            encode_application(step, &mut out)?;
            let payload = size(out.finish().len(), AT)?;
            budget.work(payload, AT)?;
            let mut value = step.clone();
            let mut edges = vec![
                (value.context, Edge::InContext),
                (value.left, Edge::ConclusionLeft),
                (value.right, Edge::ConclusionRight),
            ];
            edges
                .try_reserve(
                    local_premises
                        .len()
                        .checked_add(value.conditions.requirements.len())
                        .ok_or_else(|| Fault::capacity(BudgetKind::Edges, AT))?,
                )
                .map_err(|_| Fault::allocation(AT))?;
            for (index, previous) in local_premises.iter().enumerate() {
                let id = usize::try_from(*previous)
                    .ok()
                    .and_then(|index| proofs.get(index))
                    .copied()
                    .ok_or_else(mismatch)?;
                value
                    .premises
                    .try_reserve(1)
                    .map_err(|_| Fault::allocation(AT))?;
                value.premises.push(id);
                edges.push((id, Edge::Premise(position(index)?)));
            }
            self.publish_conditions(&mut staged, &value, &mut edges, pending, budget)?;
            let application = self.insert(
                &mut staged,
                Vertex::Application(value),
                &edges,
                pending,
                budget,
            )?;
            let proof = self.insert(
                &mut staged,
                Vertex::Proof(application),
                &[(application, Edge::Justifies)],
                pending,
                budget,
            )?;
            proofs.push(proof);
        }
        let total = staged
            .indices
            .keys()
            .filter(|id| id.kind == VertexKind::Proof)
            .count();
        budget.check(BudgetKind::ProofNodes, size(total, AT)?, AT)?;
        let root = proofs.last().copied().ok_or_else(mismatch)?;
        self.state = staged;
        Ok(self.handle(root))
    }

    /// Retain every scalar domain predicate as an explicit application dependency.
    fn publish_conditions(
        &self,
        state: &mut State,
        value: &CheckedStep,
        edges: &mut Vec<(LogicalId, Edge)>,
        pending: u64,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        for (index, required) in value.conditions.requirements.iter().enumerate() {
            let condition = Condition {
                kind: required.kind.predicate(),
                header: state.context(value.context)?.header_id,
                scope: state.expression(required.term)?.scope.clone(),
                arguments: vec![required.term],
            };
            let condition_header = state.context(value.context)?.header_id;
            let condition = self.insert(
                state,
                Vertex::Condition(condition),
                &[
                    (condition_header, Edge::InHeader),
                    (required.term, Edge::PredicateArgument(0)),
                ],
                pending,
                budget,
            )?;
            edges.push((condition, Edge::Obligation(position(index)?)));
        }
        Ok(())
    }

    /// Register an older sealed proof as a strictly cheaper shortcut under the current logical
    /// policy.
    ///
    /// # Errors
    /// Refuses a foreign proof, a different expected theorem, nonpositive/excessive cost,
    /// failure to improve the current known route, or exhaustion. Dependencies always precede it.
    pub(crate) fn register_scalar_derived(
        &mut self,
        witness: &VerifiedWitness,
        expected: &Claim,
        cost: u64,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        self.proof_limits(budget, |cas, budget| {
            let proof = cas.checked(&witness.proof)?;
            let checked = application(cas, proof)?;
            if cost == 0 || cost > 1_000_000 {
                return Err(Fault::invalid(InvalidCode::Policy, AT));
            }
            let expected = crate::wire::claim_key(cas, expected, budget)?;
            if expected != crate::wire::claim_key(cas, &witness.claim, budget)? {
                return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
            }
            cas.recheck_witness(witness, budget)?;
            let mut incumbent = checked.cost;
            for id in cas.state.indices.keys() {
                budget.work(1, AT)?;
                let (value, weight) = match cas.state.node(*id)? {
                    Vertex::Application(value) => (value, value.cost),
                    Vertex::ScopedApplication(value) => {
                        let Some(checked) = &value.scalar else {
                            continue;
                        };
                        (checked, checked.cost)
                    }
                    Vertex::Shortcut { proof, cost } => (application(cas, *proof)?, *cost),
                    _ => continue,
                };
                let mut out = Encoder::new(budget, AT);
                encode_application(value, &mut out)?;
                let bytes = size(out.finish().len(), AT)?;
                budget.work(bytes, AT)?;
                if value.context == checked.context
                    && value.left == checked.left
                    && value.right == checked.right
                    && value.relation == checked.relation
                    && value.conditions.residuals == checked.conditions.residuals
                {
                    incumbent = incumbent.min(weight);
                }
            }
            if cost >= incumbent {
                return Err(Fault::invalid(InvalidCode::Policy, AT));
            }
            let context = checked.context;
            let left = checked.left;
            let right = checked.right;
            let candidate = cas.fork(budget)?;
            let mut staged = candidate.state;
            let id = cas.insert(
                &mut staged,
                Vertex::Shortcut { proof, cost },
                &[
                    (proof, Edge::DependsOn),
                    (context, Edge::InContext),
                    (left, Edge::ConclusionLeft),
                    (right, Edge::ConclusionRight),
                ],
                0,
                budget,
            )?;
            cas.state = staged;
            Ok(cas.handle(id))
        })
    }
}
