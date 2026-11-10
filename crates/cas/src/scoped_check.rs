// SPDX-License-Identifier: AGPL-3.0-only
//! Transactional typed primitive checking and sealed public witnesses.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, Handle, Vertex, size};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, Judgment, ScopedConditions, Statement,
    VerifiedWitness,
};
use crate::rule::Primitive;

/// Static location for typed proof checking.
const AT: Location = Location::new("typed proof admission");

/// A statement cannot gain authority from an ill-formed application.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Check backward references and complete root reachability before any publication.
fn preflight(cas: &Cas, steps: &[ApplicationDraft], budget: &mut Budget) -> Result<(), Fault> {
    crate::work_observation::observe!(budget, AT, {
        budget.check(BudgetKind::ProofNodes, size(steps.len(), AT)?, AT)?;
        let root = steps.len().checked_sub(1).ok_or_else(invalid)?;
        let mut reached = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(index) = pending.pop() {
            budget.work(size(steps.len(), AT)?, AT)?;
            if !reached.insert(index) {
                continue;
            }
            let step = steps.get(index).ok_or_else(invalid)?;
            if let Statement::Series(value) = &step.conclusion {
                crate::series::check::precision(cas, value, budget)?;
            }
            if let Statement::Series(value) = &step.conclusion
                && matches!(
                    value.as_ref(),
                    crate::series::SeriesStatement::Reciprocal { .. }
                        | crate::series::SeriesStatement::Source(_)
                        | crate::series::SeriesStatement::EvaluatedResidual { .. }
                )
                && step.premises.len() != crate::series::check::premise_count(cas, value)?
            {
                return Err(invalid());
            }
            if !step.rule.specification().available {
                return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
            }
            crate::proof_view::measure(cas, step, budget)?;
            pending
                .try_reserve(step.premises.len())
                .map_err(|_| Fault::allocation(AT))?;
            for previous in &step.premises {
                let previous = usize::try_from(*previous).map_err(|_| invalid())?;
                if previous >= index {
                    return Err(invalid());
                }
                pending.push(previous);
            }
        }
        if reached.len() != steps.len() {
            return Err(invalid());
        }
        Ok(())
    })
}

/// Own the complete premise inventory until the checked step has been published.
pub(crate) fn check_supplied(
    cas: &mut Cas,
    step: &ApplicationDraft,
    proofs: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    crate::proof_view::premise_arity(
        cas,
        step.rule,
        &step.conclusion,
        step.premises.len(),
        budget,
        AT,
    )?;
    let mut ids = Vec::new();
    ids.try_reserve_exact(step.premises.len())
        .map_err(|_| Fault::allocation(AT))?;
    for index in &step.premises {
        let proof = proofs
            .get(usize::try_from(*index).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        ids.push(cas.checked(proof)?);
    }
    if let Statement::Series(value) = &step.conclusion
        && let crate::series::SeriesStatement::Source(value) = value.as_ref()
    {
        let proofs: Vec<_> = ids.iter().map(|id| cas.handle(*id)).collect();
        crate::series::source::preflight_slots(cas, value.clause, &proofs, budget)?;
    }
    let bytes = crate::proof_view::premises_bytes(cas, &ids, budget, AT)?;
    budget.with_retained_payload(bytes, AT, |budget| {
        let mut premises = Vec::new();
        premises
            .try_reserve_exact(ids.len())
            .map_err(|_| Fault::allocation(AT))?;
        for id in &ids {
            premises.push(crate::proof_view::application(cas, *id, budget)?);
        }
        let mut application = check_application(cas, step, &premises, budget)?;
        application.premises = ids;
        cas.publish_scoped(application, budget)
    })
}

/// Preserve native conditions while using the existing exact scalar schema calculation.
fn scalar(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(ScopedConditions, crate::scalar_model::CheckedStep), Fault> {
    let context = cas.checked(&draft.context)?;
    let mut step = crate::proof_view::scalar_step(cas, draft)?;
    if draft.rule == Primitive::Substitute {
        for previous in premises {
            for used in &previous.conditions.hypotheses {
                if cas
                    .state
                    .context(cas.checked(&used.context)?)?
                    .parent
                    .is_some()
                {
                    return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
                }
            }
        }
    }
    let mut earlier = Vec::new();
    earlier
        .try_reserve_exact(premises.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (index, previous) in premises.iter().enumerate() {
        if previous.judgment.context != draft.context {
            return Err(invalid());
        }
        earlier.push(crate::proof_view::scalar_application(
            cas, previous, budget,
        )?);
        let slot = step.premises.get_mut(index).ok_or_else(invalid)?;
        *slot = u32::try_from(index).map_err(|_| invalid())?;
    }
    let checked = crate::proof_check::compile_step(cas, context, &step, &earlier, budget)?;
    let mut result = crate::proof_view::scalar_conditions(cas, &checked, budget)?;
    if draft.rule != Primitive::Substitute {
        for previous in premises {
            crate::statement::merge(cas, &mut result, &previous.conditions, budget)?;
        }
    }
    Ok((result, checked))
}

/// Dispatch each closed native family while keeping scalar projection explicit.
#[expect(
    clippy::too_many_lines,
    reason = "The closed primitive dispatcher keeps every registered checker visible together"
)]
fn rule_conditions(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(ScopedConditions, Option<crate::scalar_model::CheckedStep>), Fault> {
    let mut scalar_view = None;
    let conditions = match draft.rule {
        Primitive::SeriesTruncate
        | Primitive::SeriesAdd
        | Primitive::SeriesScale
        | Primitive::SeriesDerive
        | Primitive::SeriesSource
        | Primitive::SeriesIntegrate
        | Primitive::SeriesReciprocal
        | Primitive::SeriesMul
        | Primitive::SeriesResidual
        | Primitive::SeriesCompose
        | Primitive::SeriesCoefficient => {
            crate::series::check::check(cas, draft, premises, budget)?
        }
        Primitive::SinOdd | Primitive::CosEven | Primitive::AtanOdd => {
            crate::trig::parity::check(cas, draft, premises, budget)?
        }
        Primitive::TrigDefined => crate::trig::defined::check(cas, draft, premises, budget)?,
        Primitive::HalfPiWitness => crate::trig::check(cas, draft, premises, budget)?,
        Primitive::SinHalfPi | Primitive::CosHalfPi | Primitive::AtanZero | Primitive::AtanOne => {
            crate::trig::value_check(cas, draft, premises, budget)?
        }
        Primitive::Solve(_) => crate::solving::check(cas, draft, premises, budget)?,
        Primitive::LimitCaseRestrict | Primitive::SeriesCaseRestrict => {
            crate::germ_check::check(cas, draft, premises, budget)?
        }
        Primitive::IntegralCaseSplit | Primitive::SumCaseSplit => {
            crate::split_check::check(cas, draft, premises, budget)?
        }
        Primitive::DistAffinePullback
        | Primitive::DistSimpleZeroPullback
        | Primitive::DistPrimitiveUnique
        | Primitive::DistRegular
        | Primitive::DistAe
        | Primitive::DistLinear
        | Primitive::DistDerivative
        | Primitive::DistDeltaPair
        | Primitive::DistSmooth
        | Primitive::DistDeltaMultiply
        | Primitive::DistSupportAction
        | Primitive::DistJump => crate::distribution_check::check(cas, draft, premises, budget)?,
        Primitive::JetPolynomial | Primitive::CaseInterior | Primitive::CaseBoundary => {
            crate::boundary_check::check(cas, draft, premises, budget)?
        }
        Primitive::CaseSelect
            if matches!(
                &draft.conclusion,
                Statement::LatticeSelection(_) | Statement::LatticeNearestSelection(_)
            ) =>
        {
            crate::lattice::selection::check(cas, draft, premises, budget)?
        }
        Primitive::CaseSelect if matches!(&draft.conclusion, Statement::IntegerSelection(_)) => {
            crate::decision_integer_selection::check(cas, draft, premises, budget)?
        }
        Primitive::CaseOrdered
        | Primitive::CaseDomain
        | Primitive::CaseSelect
        | Primitive::CaseCongruence
        | Primitive::CaseEliminate => crate::case_check::check(cas, draft, premises, budget)?,
        Primitive::ContextHypothesis => crate::context_check::hypothesis(cas, draft, budget)?,
        Primitive::ContextWeaken => crate::context_check::weaken(cas, draft, premises, budget)?,
        Primitive::ContextClose if draft.certificate == Certificate::DecisionDischarge => {
            crate::decision_sign::discharge(cas, draft, premises, budget)?
        }
        Primitive::ContextClose => crate::context_check::close(cas, draft, premises, budget)?,
        Primitive::RegionDecide if matches!(&draft.conclusion, Statement::LatticeCover(_)) => {
            crate::lattice::cover::check::check(cas, draft, premises, budget)?
        }
        Primitive::RegionDecide if matches!(&draft.conclusion, Statement::IntegerFamily(_)) => {
            crate::decision_cut::family::check(cas, draft, premises, budget)?
        }
        Primitive::RegionDecide
            if matches!(&draft.certificate, Certificate::DecisionNonZero(_)) =>
        {
            crate::decision_sign::check(cas, draft, premises, budget)?
        }
        Primitive::RegionDecide if matches!(&draft.certificate, Certificate::Decision(_)) => {
            crate::decision_introduction::check(cas, draft, premises, budget)?
        }
        Primitive::RegionDecide if matches!(&draft.certificate, Certificate::Emptiness(_)) => {
            crate::emptiness::check(cas, draft, premises, budget)?
        }
        Primitive::RegionDecide | Primitive::RegionConstruct => {
            if !premises.is_empty() || !draft.substitutions.is_empty() {
                return Err(invalid());
            }
            if draft.rule == Primitive::RegionDecide {
                let Certificate::Region(_) = &draft.certificate else {
                    return Err(invalid());
                };
                crate::region_check::decide(cas, &draft.conclusion, &draft.certificate, budget)?;
            } else {
                crate::region_check::construct(cas, &draft.conclusion, &draft.certificate, budget)?;
            }
            ScopedConditions::default()
        }
        _ => {
            let (conditions, checked) = scalar(cas, draft, premises, budget)?;
            scalar_view = Some(checked);
            conditions
        }
    };
    Ok((conditions, scalar_view))
}

/// Replay one rule with already checked premise applications, in an unpublished candidate.
pub(crate) fn check_application(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<CheckedApplication, Fault> {
    let _attribution = crate::work_observation::Scope::rule(|| draft.rule.specification().key);
    crate::work_observation::observe!(budget, crate::error::Location::new(""), {
        crate::proof_view::measure(cas, draft, budget)?;
        crate::solving::coefficients::scope::check(cas, draft, premises, budget)?;
        let (mut conditions, scalar_view) = rule_conditions(cas, draft, premises, budget)?;
        conditions.requirements = crate::statement::union(cas, &conditions.requirements, budget)?;
        conditions.residuals = crate::statement::union(cas, &conditions.residuals, budget)?;
        crate::statement::canonical_hypotheses(cas, &mut conditions.hypotheses, budget)?;
        for hypothesis in &conditions.hypotheses {
            if !crate::context_check::ancestor(
                cas,
                cas.checked(&hypothesis.context)?,
                cas.checked(&draft.context)?,
                budget,
            )? {
                return Err(invalid());
            }
        }
        let mut depth = 1_u64;
        for premise in premises {
            depth = depth.max(
                premise
                    .depth
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?,
            );
        }
        budget.check(BudgetKind::Depth, depth, AT)?;
        let layer = crate::statement::layer(&draft.conclusion);
        let judgment = Judgment {
            context: draft.context.clone(),
            layer,
            space: crate::statement::space(&draft.conclusion),
            statement: draft.conclusion.clone(),
            residuals: conditions.residuals.clone(),
        };
        crate::proof_view::validate_judgment(cas, &judgment, budget)?;
        Ok(CheckedApplication {
            scalar: scalar_view,
            rule: draft.rule,
            judgment,
            premises: Vec::new(),
            substitutions: draft.substitutions.clone(),
            certificate: draft.certificate.clone(),
            conditions,
            depth,
        })
    })
}

/// Exact local handles prove identity directly; unequal local views retain portable comparison.
fn local_judgment_equal(
    expected: &Judgment,
    actual: &Judgment,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let _phase = crate::work_observation::Scope::phase("overhead.judgment-comparison");
    crate::work_observation::observe!(budget, AT, {
        let length = {
            let mut out = crate::encoding::Encoder::new(budget, AT);
            crate::statement::judgment(expected, &mut out)?;
            size(out.finish().len(), AT)?
        };
        budget.work(length, AT)?;
        Ok(expected == actual)
    })
}

impl Cas {
    /// Retain one checked native application; the caller owns the enclosing transaction.
    pub(crate) fn publish_scoped(
        &mut self,
        mut application: CheckedApplication,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        if application.scalar.is_none() && !application.rule.native() {
            application.scalar = Some(crate::proof_view::scalar_application(
                self,
                &application,
                budget,
            )?);
        }
        if let Some(checked) = &mut application.scalar {
            checked.premises.clone_from(&application.premises);
        }
        let mut edges = vec![(
            self.checked(&application.judgment.context)?,
            Edge::InContext,
        )];
        if let Certificate::LatticeSelection(value) = &application.certificate {
            crate::lattice::selection::wire::dependencies(self, value, &mut edges, budget)?;
        }
        if let Certificate::IntegerSelection(value) = &application.certificate {
            crate::decision_integer_wire::dependencies(self, value, &mut edges, budget)?;
        }
        if let Certificate::Decision(value) = &application.certificate {
            crate::decision_introduction::dependencies(self, value, &mut edges, budget)?;
        }
        let mut objects = crate::statement::objects(&application.judgment.statement);
        if matches!(&application.judgment.statement, Statement::Cover { .. }) {
            let count = size(objects.len(), AT)?;
            budget.work(
                count
                    .checked_mul(count)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                AT,
            )?;
            // Set roles use the same local identity order as admitted region constructors.
            objects
                .get_mut(1..)
                .ok_or_else(invalid)?
                .sort_unstable_by_key(|value| value.logical_id());
        }
        edges
            .try_reserve(
                objects
                    .len()
                    .checked_add(application.premises.len())
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| Fault::allocation(AT))?;
        for (index, object) in objects.iter().enumerate() {
            let position = u32::try_from(index).map_err(|_| invalid())?;
            edges.push((
                self.checked(object)?,
                crate::statement::edge_role(&application.judgment.statement, position),
            ));
        }
        for (index, premise) in application.premises.iter().enumerate() {
            edges.push((
                *premise,
                Edge::Premise(u32::try_from(index).map_err(|_| invalid())?),
            ));
        }
        for (index, guarded) in application.conditions.requirements.iter().enumerate() {
            edges.push((
                self.checked(&guarded.predicate)?,
                Edge::Obligation(u32::try_from(index).map_err(|_| invalid())?),
            ));
        }
        if matches!(application.judgment.statement, Statement::Series(_)) {
            return self.publish_series(application, &edges, budget);
        }
        self.with_state(|cas, state| {
            let id = cas.insert(
                state,
                Vertex::ScopedApplication(Box::new(application)),
                &edges,
                0,
                budget,
            )?;
            let proof = cas.insert(
                state,
                Vertex::Proof(id),
                &[(id, Edge::Justifies)],
                0,
                budget,
            )?;
            budget.check(BudgetKind::ProofNodes, state.proof_count(), AT)?;
            Ok(cas.handle(proof))
        })
    }

    /// Check a complete typed proof and publish only its fully checked dependency closure.
    ///
    /// # Errors
    /// Refuses invalid scopes, unsupported rules, false certificates and exhausted resources.
    pub fn check_proof(
        &mut self,
        context: &Handle,
        steps: &[ApplicationDraft],
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            preflight(cas, steps, budget)?;
            if steps.last().ok_or_else(invalid)?.context != *context {
                return Err(invalid());
            }
            cas.transaction(budget, |candidate, budget| {
                if steps
                    .iter()
                    .all(|step| step.context == *context && !step.rule.native())
                {
                    let mut scalar = Vec::new();
                    scalar
                        .try_reserve_exact(steps.len())
                        .map_err(|_| Fault::allocation(AT))?;
                    for step in steps {
                        scalar.push(crate::proof_view::scalar_step(candidate, step)?);
                    }
                    let context_id = candidate.checked(context)?;
                    let compiled =
                        crate::proof_check::compile(candidate, context_id, &scalar, budget)?;
                    let witness = candidate.finish_proof(&compiled, budget)?;
                    let witness = crate::proof_view::native_witness(candidate, witness, budget)?;
                    return Ok(witness);
                }
                let mut proofs: Vec<Handle> = Vec::new();
                proofs
                    .try_reserve_exact(steps.len())
                    .map_err(|_| Fault::allocation(AT))?;
                for step in steps {
                    if matches!(&step.conclusion, Statement::Series(value) if matches!(value.as_ref(), crate::series::SeriesStatement::Source(_))) {
                        proofs.push(check_source_step(candidate, step, &proofs, budget)?);
                        continue;
                    }
                    let proof = check_supplied(candidate, step, &proofs, budget)?;
                    proofs.push(proof);
                }
                let witness = candidate.seal_scoped(proofs.last().ok_or_else(invalid)?, budget)?;
                Ok(witness)
            })
        })
    }

    /// Encode the checked theorem including every residual and qualified hypothesis.
    pub(crate) fn seal_scoped(
        &self,
        proof: &Handle,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        let application = crate::proof_view::application(self, self.checked(proof)?, budget)?;
        let bytes = crate::wire::encode(self, proof.logical_id(), budget)?;
        Ok(VerifiedWitness {
            proof: proof.clone(),
            judgment: application.judgment,
            hypotheses: application.conditions.hypotheses,
            bytes,
        })
    }

    /// Independently inspect a canonical witness in an unpublished arena.
    ///
    /// # Errors
    /// Refuses malformed or noncanonical records, false rules and exhausted resources.
    pub fn inspect(&mut self, bytes: &[u8], budget: &mut Budget) -> Result<VerifiedWitness, Fault> {
        crate::wire_decode::magic(bytes)?;
        self.proof_limits(budget, |cas, budget| {
            crate::proof_check::replay_budget(budget, |budget| {
                cas.transaction(budget, |candidate, budget| {
                    let witness = crate::wire_decode::inspect(candidate, bytes, budget)?;
                    Ok(witness)
                })
            })
        })
    }

    /// Replay against the complete expected typed judgment before publishing any imported record.
    ///
    /// # Errors
    /// Refuses different contexts, regions, layers, spaces, predicates or statements.
    pub fn replay(
        &mut self,
        bytes: &[u8],
        expected: &Judgment,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        crate::wire_decode::magic(bytes)?;
        self.proof_limits(budget, |cas, budget| {
            crate::proof_check::replay_budget(budget, |budget| {
                crate::proof_view::validate_judgment(cas, expected, budget)?;
                cas.transaction(budget, |candidate, budget| {
                    let witness = crate::wire_decode::inspect(candidate, bytes, budget)?;
                    if local_judgment_equal(expected, &witness.judgment, budget)? {
                        return Ok(witness);
                    }
                    let expected = crate::wire_build::judgment_key(candidate, expected, budget)?;
                    let actual =
                        crate::wire_build::judgment_key(candidate, &witness.judgment, budget)?;
                    if actual != expected {
                        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
                    }
                    Ok(witness)
                })
            })
        })
    }

    /// Copy the current canonical witness after reserving its exact staged payload.
    ///
    /// The copy costs one Work per byte. Use `VerifiedWitness::exported_byte_len`
    /// before export and `Budget::with_retained_payload` while retaining other buffers.
    ///
    /// # Errors
    /// Refuses a foreign proof or a staged, witness, output or Work resource limit.
    pub fn export_witness(
        &self,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<Vec<u8>, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = (|| {
            self.checked(&witness.proof)?;
            let count = size(witness.bytes.len(), AT)?;
            budget.check(BudgetKind::WitnessBytes, count, AT)?;
            budget.check(BudgetKind::OutputBytes, count, AT)?;
            budget.check(BudgetKind::StagedPayloadBytes, count, AT)?;
            budget.work(count, AT)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(witness.bytes.len())
                .map_err(|_| Fault::allocation(AT))?;
            bytes.extend_from_slice(&witness.bytes);
            Ok(bytes)
        })();
        budget.restore_limits(previous);
        result
    }
}

/// New source paths retain every simultaneous premise copy independently of generic admission.
fn check_source_step(
    cas: &mut Cas,
    step: &ApplicationDraft,
    proofs: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let Statement::Series(statement) = &step.conclusion else {
        return Err(invalid());
    };
    if step.premises.len() != crate::series::check::premise_count(cas, statement)? {
        return Err(invalid());
    }
    let crate::series::SeriesStatement::Source(source) = statement.as_ref() else {
        return Err(invalid());
    };
    if matches!(
        source.clause,
        crate::series::SourceClause::Reciprocal | crate::series::SourceClause::Quotient
    ) {
        let slots = step
            .premises
            .iter()
            .map(|index| {
                proofs
                    .get(usize::try_from(*index).map_err(|_| invalid())?)
                    .cloned()
                    .ok_or_else(invalid)
            })
            .collect::<Result<Vec<_>, Fault>>()?;
        crate::series::source::preflight_slots(cas, source.clause, &slots, budget)?;
    }
    let mut bytes = crate::series::codec::source_workspace(&step.certificate, budget)?;
    for index in &step.premises {
        let proof = proofs
            .get(usize::try_from(*index).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        bytes = bytes
            .checked_add(crate::proof_view::application_bytes(
                cas,
                cas.checked(proof)?,
                budget,
            )?)
            .ok_or_else(invalid)?;
    }
    budget.with_retained_payload(bytes, AT, |budget| {
        let mut premises = Vec::new();
        let mut ids = Vec::new();
        premises
            .try_reserve_exact(step.premises.len())
            .map_err(|_| Fault::allocation(AT))?;
        ids.try_reserve_exact(step.premises.len())
            .map_err(|_| Fault::allocation(AT))?;
        for index in &step.premises {
            let proof = proofs
                .get(usize::try_from(*index).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            ids.push(cas.checked(proof)?);
            #[cfg(test)]
            crate::series::source::observe_slot_copy(source.clause);
            premises.push(crate::proof_view::application(
                cas,
                cas.checked(proof)?,
                budget,
            )?);
        }
        let mut application = check_application(cas, step, &premises, budget)?;
        application.premises = ids;
        cas.publish_scoped(application, budget)
    })
}
