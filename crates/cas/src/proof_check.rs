// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic proof admission and independent canonical replay.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::conditions::{Conditions, Obligation};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::rule::Primitive;
use crate::rule_check::{mismatch, premise};
use crate::scalar_model::{
    CheckedStep, Claim, Compiled, Guard, ProofStep, Relation, VerifiedWitness,
};

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
        preflight_step(cas, context, step, budget)?;
    }
    if reached.len() == steps.len() {
        Ok(())
    } else {
        Err(mismatch())
    }
}

/// Validate one scalar schema shape equally for direct admission and wire replay.
fn preflight_step(
    cas: &Cas,
    context: LogicalId,
    step: &ProofStep,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if (step.primitive != Primitive::Congruence && !step.path.is_empty())
        || (step.primitive != Primitive::Substitute && !step.substitutions.is_empty())
        || (step.primitive == Primitive::SquareFree) != step.certificate.is_some()
    {
        return Err(mismatch());
    }
    let left = cas.state.expression(cas.checked(&step.left)?)?;
    let right = cas.state.expression(cas.checked(&step.right)?)?;
    if left.space.is_some() || right.space.is_some() {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
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
        || !crate::header::embeds(
            &cas.state,
            left.header,
            cas.state.context(context)?.header_id,
            budget,
        )?
        || !crate::header::embeds(
            &cas.state,
            right.header,
            cas.state.context(context)?.header_id,
            budget,
        )?
        || (step.primitive != Primitive::Jacobian && left.sort != right.sort)
    {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    budget.check(BudgetKind::Depth, size(step.path.len(), AT)?, AT)?;
    Ok(())
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
pub(crate) fn compile_step(
    cas: &mut Cas,
    context: LogicalId,
    step: &ProofStep,
    earlier: &[CheckedStep],
    budget: &mut Budget,
) -> Result<CheckedStep, Fault> {
    crate::context::require_closed(&cas.state, context)?;
    measure_input(cas, std::slice::from_ref(step), budget)?;
    preflight_step(cas, context, step, budget)?;
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
pub(crate) fn replay_budget<T>(
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

    /// Seal only after complete canonical encoding and checked graph publication.
    pub(crate) fn finish_proof(
        &mut self,
        compiled: &Compiled,
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
        let bytes = crate::wire::encode(self, proof.logical_id(), budget)?;
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
        let actual_claim = crate::proof_view::scalar_claim(&fresh, &replayed.judgment)?;
        let actual = crate::wire::claim_key(&fresh, &actual_claim, budget)?;
        if expected == actual {
            Ok(())
        } else {
            Err(Fault::invalid(InvalidCode::EndpointMismatch, AT))
        }
    }
}
