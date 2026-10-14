// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted case proposals checked with one shared independent checker ledger.

use crate::budget::{Budget, BudgetKind};
use crate::case_syntax::invalid;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, size};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, Statement, VerifiedWitness,
};
use crate::rule::Primitive;
use crate::search::{SearchPolicy, SearchResult, SearchStatistics};

/// A recipe's checked applications and positive transition accounting.
pub(crate) struct Plan<'a> {
    /// Complete pinned request policy.
    policy: &'a SearchPolicy,
    /// Logical cost of its actual recorded transitions.
    cost: u64,
    /// Number of emitted primitive applications.
    steps: u64,
    /// Maximum generated typed expression size.
    peak: u64,
    /// Positive expansion increments, never refunded by later contraction.
    credits: u64,
    /// Actual proposed schemas and attempted refinement pairs.
    matches: u64,
    /// Recipe proposals and resumed queue visits.
    visits: u64,
}

impl<'a> Plan<'a> {
    /// Begin a recipe without granting it a free theorem edge.
    pub(crate) const fn new(policy: &'a SearchPolicy) -> Self {
        Self {
            policy,
            cost: 0,
            steps: 0,
            peak: 0,
            credits: 0,
            matches: 0,
            visits: 0,
        }
    }

    /// Rank a pending refinement using the actual policy and already recorded route cost.
    pub(crate) fn refinement_cost(&self) -> Result<u64, Fault> {
        self.cost
            .checked_add(self.policy.costs.weight(Primitive::CaseCongruence)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))
    }

    /// Charge each completed refinement's positive growth on the same request ledger.
    pub(crate) fn expansion(
        &mut self,
        before: u64,
        after: u64,
        budget: &Budget,
    ) -> Result<(), Fault> {
        self.peak = self.peak.max(before).max(after);
        self.credits = self
            .credits
            .checked_add(after.saturating_sub(before))
            .ok_or_else(|| Fault::capacity(BudgetKind::ExpansionCredits, AT))?;
        budget.check(BudgetKind::PeakExpressionSize, self.peak, AT)?;
        budget.check(BudgetKind::ExpansionCredits, self.credits, AT)
    }

    /// Pending alternatives retain all growth already consumed by their route.
    pub(crate) const fn credits(&self) -> u64 {
        self.credits
    }

    /// Charge an attempted schema or pair without refunding failed candidates.
    pub(crate) fn attempt(&mut self, budget: &mut Budget) -> Result<(), Fault> {
        let next = self
            .matches
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::MatchAttempts, AT))?;
        budget.check(BudgetKind::MatchAttempts, next, AT)?;
        budget.work(1, AT)?;
        self.matches = next;
        Ok(())
    }

    /// Count every actual recipe or scheduler visit independently of successful transitions.
    pub(crate) fn visit(&mut self) -> Result<(), Fault> {
        self.visits = self
            .visits
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
        Ok(())
    }

    /// Propose exact evidence, then independently check and retain one application.
    #[expect(
        clippy::too_many_arguments,
        reason = "schema inputs and independent generation/checker ledgers stay explicit"
    )]
    pub(crate) fn push(
        &mut self,
        cas: &mut Cas,
        context: &Handle,
        rule: Primitive,
        conclusion: Statement,
        premises: &[Handle],
        certificate: Certificate,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<Handle, Fault> {
        let next = self
            .steps
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Transitions, AT))?;
        generation.check(BudgetKind::Transitions, next, AT)?;
        self.attempt(generation)?;
        self.visit()?;
        let (draft, earlier) = proposal(
            cas,
            context,
            rule,
            conclusion,
            premises,
            certificate,
            generation,
        )?;
        let mut application =
            crate::scoped_check::check_application(cas, &draft, &earlier, checker)?;
        application.premises = premises.iter().map(Handle::logical_id).collect();
        let proof = cas.publish_scoped(application, checker)?;
        self.cost = self
            .cost
            .checked_add(self.policy.costs.weight(rule)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
        self.steps = next;
        Ok(proof)
    }

    /// Replay the expansion in an independent empty store before reporting success.
    pub(crate) fn finish(
        self,
        cas: &Cas,
        proof: &Handle,
        trace: Vec<Handle>,
        checker: &mut Budget,
    ) -> Result<SearchResult, Fault> {
        let witness = cas.seal_scoped(proof, checker)?;
        if !self.policy.allow_conditional && !witness.is_discharged() {
            return Err(Fault::invalid(InvalidCode::PendingPremise, AT));
        }
        replay(cas, &witness, checker)?;
        let peak = trace
            .iter()
            .filter_map(|handle| cas.state.expression(handle.logical_id()).ok())
            .map(|value| value.unfolded)
            .max()
            .unwrap_or(1);
        Ok(SearchResult {
            witness,
            cost: self.cost,
            peak: peak.max(self.peak),
            expansion_credits: self.credits,
            primitive_steps: self.steps,
            trace,
            policy: self.policy.costs.clone(),
            statistics: SearchStatistics {
                transitions: self.steps,
                matches: self.matches,
                visits: self.visits,
                dominated: 0,
            },
            replay_usage: checker.usage(),
            response: None,
        })
    }
}

/// Build a proposal from syntax and supplied proofs; no certificate bypasses replay.
fn proposal(
    cas: &mut Cas,
    context: &Handle,
    rule: Primitive,
    conclusion: Statement,
    premises: &[Handle],
    certificate: Certificate,
    generation: &mut Budget,
) -> Result<(ApplicationDraft, Vec<CheckedApplication>), Fault> {
    let mut earlier = Vec::new();
    earlier
        .try_reserve_exact(premises.len())
        .map_err(|_| Fault::allocation(AT))?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(premises.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (index, proof) in premises.iter().enumerate() {
        earlier.push(crate::proof_view::application(
            cas,
            cas.checked(proof)?,
            generation,
        )?);
        indices.push(u32::try_from(index).map_err(|_| invalid())?);
    }
    let mut draft = ApplicationDraft {
        context: context.clone(),
        rule,
        conclusion,
        premises: indices,
        substitutions: Vec::new(),
        certificate,
    };
    if matches!(
        rule,
        Primitive::CaseOrdered
            | Primitive::CaseDomain
            | Primitive::CaseSelect
            | Primitive::CaseCongruence
            | Primitive::CaseEliminate
    ) {
        let (certificate, _) = crate::case_check::reconstruct(cas, &draft, &earlier, generation)?;
        draft.certificate = Certificate::Cases(Box::new(certificate));
    } else if rule == Primitive::RegionDecide {
        draft.certificate = Certificate::Region(crate::region_check::evidence(
            cas,
            &draft.conclusion,
            generation,
        )?);
    }
    Ok((draft, earlier))
}

/// Compare both canonical bytes and the exact expected theorem in a fresh store.
fn replay(cas: &Cas, witness: &VerifiedWitness, checker: &mut Budget) -> Result<(), Fault> {
    let expected = crate::wire_build::judgment_key(cas, witness.judgment(), checker)?;
    let bytes = cas.export_witness(witness, checker)?;
    let mut receiver = Cas::new(checker.limits().clone())?;
    let replayed = receiver.inspect(&bytes, checker)?;
    if receiver.export_witness(&replayed, checker)? != bytes
        || crate::wire_build::judgment_key(&receiver, replayed.judgment(), checker)? != expected
    {
        return Err(invalid());
    }
    Ok(())
}

/// Case recipe accounting location.
const AT: Location = Location::new("native case recipe");

/// Build a checked domain theorem with the exact reconstructed strict domain.
pub(crate) fn domain(
    plan: &mut Plan<'_>,
    cas: &mut Cas,
    context: &Handle,
    function: &Handle,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<(Handle, Handle), Fault> {
    let within = cas.state.context(cas.checked(context)?)?.admissible;
    let root = cas.checked(function)?;
    let order = crate::case_syntax::order(cas, &[root], generation)?;
    let domains = crate::case_syntax::domains(cas, &order, within, generation)?;
    let domain = cas.handle(*domains.get(&root).ok_or_else(invalid)?);
    let proof = plan.push(
        cas,
        context,
        Primitive::CaseDomain,
        Statement::DefinedDomain {
            function: function.clone(),
            domain: domain.clone(),
        },
        &[],
        Certificate::None,
        generation,
        checker,
    )?;
    Ok((proof, domain))
}

/// Charge an explicit finite family's size before recipe allocation.
pub(crate) fn family(count: usize, budget: &mut Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::CaseArms, size(count, AT)?, AT)?;
    budget.work(size(count, AT)?.saturating_mul(256), AT)
}
