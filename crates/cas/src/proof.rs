// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic proof admission and independent canonical replay.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::conditions::{Conditions, Obligation};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::proof_model::{
    CheckedStep, Claim, Compiled, Guard, ProofStep, Relation, VerifiedWitness,
};
use crate::rule::Primitive;
use crate::rule_check::{mismatch, premise};

/// Static location for proof transactions.
const AT: Location = Location::new("proof admission");

/// Measure all caller-owned collections before cloning proof payloads or reserving stacks.
fn measure_input(cas: &Cas, steps: &[ProofStep], budget: &mut Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::ProofNodes, size(steps.len(), AT)?, AT)?;
    let mut total = 0_u64;
    for step in steps {
        budget.check(BudgetKind::ProofNodes, size(step.premises.len(), AT)?, AT)?;
        budget.check(BudgetKind::Depth, size(step.path.len(), AT)?, AT)?;
        budget.check(
            BudgetKind::Vertices,
            size(step.substitutions.len(), AT)?,
            AT,
        )?;
        if let Relation::Jacobian { variables } = &step.relation {
            budget.check(
                BudgetKind::PolynomialVariables,
                size(variables.len(), AT)?,
                AT,
            )?;
        }
        let certificate_bytes = match &step.certificate {
            Some(value) => value.payload(budget)?,
            None => 0,
        };
        let mut out = crate::encoding::Encoder::new(budget, AT);
        crate::wire::encode_relation(&step.relation, &mut out)?;
        if let Some(value) = &step.certificate {
            value.encode(&mut out)?;
        }
        out.count(step.premises.len())?;
        for index in &step.premises {
            crate::wire::index(*index, &mut out)?;
        }
        out.count(step.path.len())?;
        for index in &step.path {
            crate::wire::index(*index, &mut out)?;
        }
        let mut previous: Option<&str> = None;
        out.count(step.substitutions.len())?;
        for substitution in &step.substitutions {
            if previous.is_some_and(|key| key >= substitution.key.as_str()) {
                return Err(mismatch());
            }
            previous = Some(&substitution.key);
            out.name(&substitution.key)?;
            crate::graph::encode_id(cas.checked(&substitution.replacement)?, &mut out)?;
        }
        total = total
            .checked_add(size(out.finish().len(), AT)?)
            .and_then(|n| n.checked_add(512))
            .and_then(|n| n.checked_add(certificate_bytes))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    }
    let peak = total
        .checked_mul(4)
        .and_then(|n| n.checked_add(cas.counts().payload_bytes))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(BudgetKind::StagedPayloadBytes, peak, AT)?;
    budget.work(total, AT)
}

/// Check references, types and complete premise reachability before mathematical work.
fn preflight(
    cas: &Cas,
    context: LogicalId,
    steps: &[ProofStep],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.check(BudgetKind::ProofNodes, size(steps.len(), AT)?, AT)?;
    let final_index = steps.len().checked_sub(1).ok_or_else(mismatch)?;
    let mut reached = BTreeSet::new();
    let mut pending = vec![final_index];
    while let Some(index) = pending.pop() {
        budget.work(
            size(reached.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if !reached.insert(index) {
            continue;
        }
        let step = steps.get(index).ok_or_else(mismatch)?;
        pending
            .try_reserve(step.premises.len())
            .map_err(|_| Fault::allocation(AT))?;
        for previous in &step.premises {
            let previous = usize::try_from(*previous).map_err(|_| mismatch())?;
            if previous >= index {
                return Err(mismatch());
            }
            pending.push(previous);
        }
        if (step.primitive != Primitive::Congruence && !step.path.is_empty())
            || (step.primitive != Primitive::Substitute && !step.substitutions.is_empty())
            || (step.primitive == Primitive::SquareFree) != step.certificate.is_some()
        {
            return Err(mismatch());
        }
        let left = cas.state.expression(cas.checked(&step.left)?)?;
        let right = cas.state.expression(cas.checked(&step.right)?)?;
        for term in [left, right] {
            crate::rule_check::prepay_shape(term, budget)?;
            budget.check(BudgetKind::Depth, term.depth, AT)?;
            budget.check(BudgetKind::Depth, size(term.scope.len(), AT)?, AT)?;
            budget.check(BudgetKind::PeakExpressionSize, term.unfolded, AT)?;
        }
        let header = &cas.state.context(context)?.header;
        budget.work(
            size(header.len(), AT)?
                .checked_mul(2)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if left.scope != right.scope
            || cas.state.context(left.context)?.header != *header
            || cas.state.context(right.context)?.header != *header
            || (step.primitive != Primitive::Jacobian && left.sort != right.sort)
        {
            return Err(Fault::invalid(InvalidCode::Sort, AT));
        }
        budget.check(BudgetKind::Depth, size(step.path.len(), AT)?, AT)?;
    }
    if reached.len() == steps.len() {
        Ok(())
    } else {
        Err(mismatch())
    }
}

/// Compile every schema without consulting prior proof vertices or a search cache.
pub(crate) fn compile(
    cas: &mut Cas,
    context: LogicalId,
    steps: &[ProofStep],
    budget: &mut Budget,
) -> Result<Compiled, Fault> {
    measure_input(cas, steps, budget)?;
    preflight(cas, context, steps, budget)?;
    let mut result = Compiled {
        steps: Vec::new(),
        premises: Vec::new(),
    };
    result
        .steps
        .try_reserve_exact(steps.len())
        .map_err(|_| Fault::allocation(AT))?;
    result
        .premises
        .try_reserve_exact(steps.len())
        .map_err(|_| Fault::allocation(AT))?;
    let mut pending = 0_u64;
    for step in steps {
        let retained = cas
            .counts()
            .payload_bytes
            .checked_add(pending)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        let previous = budget.reserve_temporary(retained, AT)?;
        let checked = compile_step(cas, context, step, &result.steps, budget);
        budget.restore_limits(previous);
        let checked = checked?;
        pending = pending
            .checked_add(crate::proof_storage::pending_bytes(
                &checked,
                step.premises.len(),
                budget,
            )?)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        let peak = cas
            .counts()
            .payload_bytes
            .checked_add(pending)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, peak, AT)?;
        result.steps.push(checked);
        result.premises.push(step.premises.clone());
    }
    Ok(result)
}

/// Reconstruct one judgment after every ordered premise has been checked.
fn compile_step(
    cas: &mut Cas,
    context: LogicalId,
    step: &ProofStep,
    earlier: &[CheckedStep],
    budget: &mut Budget,
) -> Result<CheckedStep, Fault> {
    budget.work(1, AT)?;
    if matches!(step.relation, Relation::SquareFree { .. })
        && step.primitive != Primitive::SquareFree
    {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    let mut conditions = Conditions::default();
    let mut depth = 1_u64;
    let mut cost = step.primitive.specification().weight;
    for (position, _) in step.premises.iter().enumerate() {
        let previous = premise(step, earlier, position)?;
        let mut out = crate::encoding::Encoder::new(budget, AT);
        crate::wire::encode_relation(&previous.relation, &mut out)?;
        let bytes = size(out.finish().len(), AT)?;
        budget.work(bytes, AT)?;
        depth = depth.max(
            previous
                .depth
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?,
        );
        if step.primitive == Primitive::Transitivity {
            cost = cost
                .checked_add(previous.cost)
                .ok_or_else(|| Fault::capacity(BudgetKind::Transitions, AT))?;
        }
        if step.primitive != Primitive::Substitute {
            conditions.extend(&previous.conditions, budget)?;
        }
    }
    budget.check(BudgetKind::Depth, depth, AT)?;
    let left = cas.checked(&step.left)?;
    let right = cas.checked(&step.right)?;
    conditions.endpoint(&cas.state, context, left, budget)?;
    conditions.endpoint(&cas.state, context, right, budget)?;
    if step.primitive == Primitive::Substitute {
        substitute(cas, context, step, earlier, &mut conditions, budget)?;
    } else if step.primitive == Primitive::SquareFree {
        crate::square_free_check::check(cas, context, step, budget)?;
    } else if step.relation == Relation::EqualOnDomain {
        crate::rule_check::equality(cas, step, earlier, context, budget)?;
    } else {
        crate::derivative_check::check(cas, step, earlier, context, budget)?;
    }
    let mut substitutions = Vec::new();
    substitutions
        .try_reserve_exact(step.substitutions.len())
        .map_err(|_| Fault::allocation(AT))?;
    for substitution in &step.substitutions {
        substitutions.push((
            substitution.key.clone(),
            cas.checked(&substitution.replacement)?,
        ));
    }
    Ok(CheckedStep {
        primitive: step.primitive,
        relation: step.relation.clone(),
        context,
        left,
        right,
        premises: Vec::new(),
        path: step.path.clone(),
        substitutions,
        certificate: step.certificate.clone(),
        conditions,
        depth,
        cost,
    })
}

/// Substitute the theorem and every primitive requirement, including formerly assumed guards.
fn substitute(
    cas: &mut Cas,
    context: LogicalId,
    step: &ProofStep,
    earlier: &[CheckedStep],
    conditions: &mut Conditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if step.relation != Relation::EqualOnDomain || step.premises.len() != 1 {
        return Err(mismatch());
    }
    let previous = premise(step, earlier, 0)?;
    if previous.relation != Relation::EqualOnDomain {
        return Err(mismatch());
    }
    let left = cas.substitute(&cas.handle(previous.left), &step.substitutions, budget)?;
    let right = cas.substitute(&cas.handle(previous.right), &step.substitutions, budget)?;
    if left != step.left || right != step.right {
        return Err(mismatch());
    }
    for required in &previous.conditions.requirements {
        let term = cas.substitute(&cas.handle(required.term), &step.substitutions, budget)?;
        let id = cas.checked(&term)?;
        conditions.endpoint(&cas.state, context, id, budget)?;
        conditions.require(
            &cas.state,
            context,
            Obligation {
                kind: required.kind,
                term: id,
            },
            budget,
        )?;
    }
    Ok(())
}

/// Form the public claim after all rule and condition checks succeed.
pub(crate) fn claim(cas: &Cas, step: &CheckedStep, budget: &mut Budget) -> Result<Claim, Fault> {
    let mut out = crate::encoding::Encoder::new(budget, AT);
    crate::wire::encode_relation(&step.relation, &mut out)?;
    let bytes = size(out.finish().len(), AT)?;
    budget.work(bytes, AT)?;
    let mut residuals = Vec::new();
    residuals
        .try_reserve_exact(step.conditions.residuals.len())
        .map_err(|_| Fault::allocation(AT))?;
    for condition in &step.conditions.residuals {
        residuals.push(Guard {
            kind: condition.kind,
            term: cas.handle(condition.term),
        });
    }
    Ok(Claim {
        context: cas.handle(step.context),
        relation: step.relation.clone(),
        left: cas.handle(step.left),
        right: cas.handle(step.right),
        residuals,
    })
}

/// Keep the whole replay transaction inside its separate allowance, including claim comparison.
fn replay_budget<T>(
    budget: &mut Budget,
    operation: impl FnOnce(&mut Budget) -> Result<T, Fault>,
) -> Result<T, Fault> {
    let initial = budget.usage().work;
    let previous = budget.restrict_replay()?;
    let limited_by_replay = budget.limits().work < previous.work;
    let allowance = previous.replay_work;
    let result = operation(budget);
    budget.restore_limits(previous);
    result.map_err(|fault| {
        if limited_by_replay
            && let crate::error::FaultKind::Exhausted {
                budget: BudgetKind::Work,
                needed,
                ..
            } = fault.kind
        {
            return Fault::exhausted(
                BudgetKind::ReplayWork,
                allowance,
                needed.checked_sub(initial).unwrap_or(u64::MAX),
                fault.location,
            );
        }
        fault
    })
}

impl Cas {
    /// Intersect permanent storage ceilings for the complete proof transaction.
    pub(crate) fn proof_limits<T>(
        &mut self,
        budget: &mut Budget,
        operation: impl FnOnce(&mut Self, &mut Budget) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = operation(self, budget);
        budget.restore_limits(previous);
        result
    }

    /// Check a complete supplied proof and publish its applications atomically.
    ///
    /// # Errors
    /// Refuses missing/forward premises, false schemas, refuted guards and resource limits.
    /// Unknown guards remain visible in the sealed result; no search is invoked.
    pub fn check_proof(
        &mut self,
        context: &Handle,
        steps: &[ProofStep],
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            let context = cas.checked(context)?;
            let mut candidate = cas.fork(budget)?;
            let compiled = compile(&mut candidate, context, steps, budget)?;
            let bytes = crate::wire::encode(&candidate, &compiled, budget)?;
            let witness = candidate.finish_proof(&compiled, bytes, budget)?;
            cas.state = candidate.state;
            Ok(witness)
        })
    }

    /// Replay canonical bytes against the exact theorem the caller intends to accept.
    ///
    /// # Errors
    /// Refuses malformed/noncanonical bytes, unknown versions, bad proofs, a different
    /// context/endpoint/relation/residual set, or exhaustion. Failure publishes nothing.
    pub fn replay(
        &mut self,
        bytes: &[u8],
        expected: &Claim,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            replay_budget(budget, |budget| {
                let expected = crate::wire::claim_key(cas, expected, budget)?;
                let mut candidate = cas.fork(budget)?;
                let witness = candidate.inspect_limited(bytes, budget)?;
                let actual = crate::wire::claim_key(&candidate, &witness.claim, budget)?;
                budget.work(
                    size(expected.len(), AT)?
                        .checked_add(size(actual.len(), AT)?)
                        .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                    AT,
                )?;
                if expected != actual {
                    return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
                }
                cas.state = candidate.state;
                Ok(witness)
            })
        })
    }

    /// Inspect what canonical bytes prove without treating it as a caller's desired claim.
    ///
    /// # Errors
    /// Refuses malformed proofs and exhaustion; all imported records remain unpublished on failure.
    pub fn inspect(&mut self, bytes: &[u8], budget: &mut Budget) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            replay_budget(budget, |budget| {
                let mut candidate = cas.fork(budget)?;
                let witness = candidate.inspect_limited(bytes, budget)?;
                cas.state = candidate.state;
                Ok(witness)
            })
        })
    }

    /// Reconstruct every rule and compare the entire canonical stream within its allowance.
    fn inspect_limited(
        &mut self,
        bytes: &[u8],
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        budget.check(BudgetKind::WitnessBytes, size(bytes.len(), AT)?, AT)?;
        let decoded = crate::wire::decode(self, bytes, budget)?;
        let compiled = compile(self, decoded.context, &decoded.steps, budget)?;
        let canonical = crate::wire::encode(self, &compiled, budget)?;
        budget.work(size(bytes.len(), AT)?, AT)?;
        if canonical != bytes {
            return Err(Fault::invalid(InvalidCode::Encoding, AT));
        }
        self.finish_proof(&compiled, canonical, budget)
    }

    /// Seal only after complete canonical encoding and checked graph publication.
    pub(crate) fn finish_proof(
        &mut self,
        compiled: &Compiled,
        bytes: Vec<u8>,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        let final_step = compiled.steps.last().ok_or_else(mismatch)?;
        let claim = claim(self, final_step, budget)?;
        let mut hypotheses = Vec::new();
        hypotheses
            .try_reserve_exact(final_step.conditions.hypotheses.len())
            .map_err(|_| Fault::allocation(AT))?;
        hypotheses.extend(final_step.conditions.hypotheses.iter().cloned());
        let proof = self.publish_proof(compiled, budget)?;
        Ok(VerifiedWitness {
            proof,
            claim,
            hypotheses,
            bytes,
        })
    }

    /// Expand a shortcut candidate by replaying its complete bytes in an independent empty store.
    pub(crate) fn recheck_witness(
        &self,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        self.checked(&witness.proof)?;
        let expected = crate::wire::claim_key(self, &witness.claim, budget)?;
        let mut fresh = Self::new(self.limits.clone())?;
        let replayed = fresh.inspect(&witness.bytes, budget)?;
        let actual = crate::wire::claim_key(&fresh, &replayed.claim, budget)?;
        if expected == actual {
            Ok(())
        } else {
            Err(Fault::invalid(InvalidCode::EndpointMismatch, AT))
        }
    }

    /// Export the canonical proof bytes carried by a sealed witness from this store.
    ///
    /// # Errors
    /// Refuses foreign proof handles or output/work/allocation limits.
    pub fn export_witness(
        &self,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<Vec<u8>, Fault> {
        self.checked(&witness.proof)?;
        let length = size(witness.bytes.len(), AT)?;
        budget.check(BudgetKind::WitnessBytes, length, AT)?;
        budget.check(BudgetKind::OutputBytes, length, AT)?;
        budget.work(length, AT)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(witness.bytes.len())
            .map_err(|_| Fault::allocation(AT))?;
        bytes.extend_from_slice(&witness.bytes);
        Ok(bytes)
    }
}
