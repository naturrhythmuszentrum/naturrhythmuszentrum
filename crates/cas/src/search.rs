// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded heuristic search over independently checked conditional identities.

use std::collections::BTreeSet;
use std::fmt;

use crate::budget::{Budget, BudgetKind, BudgetUsage};
use crate::cost::CostPolicy;
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::frontier::{Frontier, Metrics, Rank};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::producer::{Plan, step};
use crate::rewrite::Action;
use crate::rule::Primitive;
use crate::scalar_model::{Relation, VerifiedWitness};

/// Request policy kept separate from the mathematical proof format.
#[derive(Clone, Debug)]
pub struct SearchPolicy {
    /// Immutable complete primitive weights.
    pub costs: CostPolicy,
    /// Whether results with the original domain's unresolved guards may be returned.
    pub allow_conditional: bool,
}

impl Default for SearchPolicy {
    fn default() -> Self {
        Self {
            costs: CostPolicy::default(),
            allow_conditional: true,
        }
    }
}

/// Fixed-size search counters, independent of mathematical truth.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchStatistics {
    /// Attempted rule/position matches, including failures.
    pub matches: u64,
    /// Proposed transitions, including size-refused candidates.
    pub transitions: u64,
    /// Labels discarded by conservative dominance.
    pub dominated: u64,
    /// Completed queue visits, including resumed cursors.
    pub visits: u64,
}

/// A checked candidate and its explicitly heuristic path accounting.
#[derive(Clone)]
pub struct SearchResult {
    /// Sealed typed calculus facts, including incomplete derivative maps.
    pub payload: Option<Box<crate::derivative_model::ResultPayload>>,
    /// Kernel-checked mathematical witness; no optimality is implied.
    pub witness: crate::proof_model::VerifiedWitness,
    /// Positive logical transition sum under the pinned policy.
    pub cost: u64,
    /// Maximum unfolded expression size along this route.
    pub peak: u64,
    /// Cumulative positive expression growth, never refunded.
    pub expansion_credits: u64,
    /// Expanded primitive/composition proof count.
    pub primitive_steps: u64,
    /// Exact ordered mathematical endpoints visited by the route.
    pub trace: Vec<Handle>,
    /// Request's complete immutable cost table.
    pub policy: CostPolicy,
    /// Finite exploration counters.
    pub statistics: SearchStatistics,
    /// Actual checker counters, separate from generation's work.
    pub replay_usage: BudgetUsage,
    /// Canonical response envelope; absent means `NotExported` under the output allowance.
    pub response: Option<Vec<u8>>,
}

impl fmt::Debug for SearchResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchResult")
            .field("witness", &self.witness)
            .field("cost", &self.cost)
            .field("peak", &self.peak)
            .field("expansion_credits", &self.expansion_credits)
            .field("primitive_steps", &self.primitive_steps)
            .field("exported", &self.response.is_some())
            .finish_non_exhaustive()
    }
}

/// One replayed search label and its resumable matching cursor.
struct Label {
    /// Full untrusted derivation, rechecked for every accepted candidate.
    plan: Plan,
    /// Independently verified conclusion.
    witness: VerifiedWitness,
    /// Current endpoint.
    endpoint: LogicalId,
    /// Complete state key, including exact domain class.
    key: Vec<u8>,
    /// Heuristic goal rank.
    rank: Rank,
    /// Path resources and support.
    metrics: Metrics,
    /// Stable finite rewrite cursor.
    actions: Vec<Action>,
    /// Next untried action.
    cursor: usize,
    /// Complete transition endpoint trace.
    trace: Vec<Handle>,
}

/// A request's finite exploration state; pending work never enters the kernel.
pub(crate) struct Search {
    /// Both scheduling orders and resource labels.
    frontier: Frontier,
    /// Retained checked candidates.
    labels: Vec<Label>,
    /// Best within the original residual class only.
    best: Option<usize>,
    /// Fixed original equality endpoint.
    original: LogicalId,
    /// Optional explicit equality target.
    target: Option<LogicalId>,
    /// Full proof context.
    context: LogicalId,
    /// Pinned cost and conditional-result policy.
    policy: SearchPolicy,
    /// Immutable pre-request proof/shortcut snapshot.
    known: Vec<crate::known::Known>,
    /// Monotone attempt statistics.
    statistics: SearchStatistics,
}

/// Search accounting location.
const AT: Location = Location::new("identity search");

impl Search {
    /// Initialize from a checked reflexivity proof retaining the entire original domain.
    pub(crate) fn new(
        cas: &mut Cas,
        context: LogicalId,
        original: LogicalId,
        target: Option<LogicalId>,
        policy: &SearchPolicy,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<Self, Fault> {
        let known = crate::known::snapshot(cas, context, &policy.costs, generation)?;
        let mut plan = Plan::default();
        plan.push(
            step(
                cas,
                Primitive::Reflexivity,
                Relation::EqualOnDomain,
                original,
                original,
                Vec::new(),
            ),
            generation,
        )?;
        let witness = plan.verify(cas, context, checker)?;
        let peak = cas.state.expression(original)?.unfolded;
        let metrics = metrics(cas, &witness, 0, peak, 0, generation)?;
        let mut result = Self {
            frontier: Frontier::default(),
            labels: Vec::new(),
            best: None,
            original,
            target,
            context,
            policy: policy.clone(),
            known,
            statistics: SearchStatistics::default(),
        };
        result.insert(
            cas,
            plan,
            witness,
            metrics,
            vec![cas.handle(original)],
            generation,
        )?;
        Ok(result)
    }

    /// Explore until a target is reached, the finite frontier empties, or a ledger refuses.
    pub(crate) fn explore(
        &mut self,
        cas: &mut Cas,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<(), Fault> {
        while !self.reached() {
            let Some(id) = self.frontier.pop(generation)? else {
                break;
            };
            self.statistics.visits = self
                .statistics
                .visits
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::QueueEntries, AT))?;
            for _ in 0..generation.limits().match_batch {
                let label = self.labels.get_mut(id).ok_or_else(invalid)?;
                let Some(action) = label.actions.get(label.cursor).cloned() else {
                    break;
                };
                label.cursor = label.cursor.checked_add(1).ok_or_else(invalid)?;
                self.statistics.matches = increment(
                    self.statistics.matches,
                    BudgetKind::MatchAttempts,
                    generation,
                )?;
                self.attempt(cas, id, &action, generation, checker)?;
                if self.reached() {
                    break;
                }
            }
            let label = self.labels.get(id).ok_or_else(invalid)?;
            if label.cursor < label.actions.len() && !self.reached() {
                self.frontier.resume(id, generation)?;
            }
        }
        Ok(())
    }

    /// Stage one proposed transition; a failed candidate never replaces the current store.
    fn attempt(
        &mut self,
        cas: &mut Cas,
        id: usize,
        action: &Action,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<(), Fault> {
        if !action.plausible(cas)? {
            return Ok(());
        }
        let previous = self.labels.get(id).ok_or_else(invalid)?;
        let mut candidate = cas.fork(generation)?;
        let proposal = crate::rewrite::propose(
            &mut candidate,
            previous.endpoint,
            self.original,
            &previous.plan,
            action,
            generation,
        );
        let plan = match proposal {
            Ok(Some(plan)) => plan,
            Ok(None)
            | Err(Fault {
                kind: FaultKind::Invalid(InvalidCode::UnsupportedFragment),
                ..
            }) => return Ok(()),
            Err(fault) => return Err(fault),
        };
        self.statistics.transitions = increment(
            self.statistics.transitions,
            BudgetKind::Transitions,
            generation,
        )?;
        let endpoint = candidate.checked(&plan.steps.last().ok_or_else(invalid)?.right)?;
        let unfolded = candidate.state.expression(endpoint)?.unfolded;
        let earlier_size = candidate.state.expression(previous.endpoint)?.unfolded;
        let growth = unfolded.saturating_sub(earlier_size);
        let credits = previous
            .metrics
            .credits
            .checked_add(growth)
            .ok_or_else(|| Fault::capacity(BudgetKind::ExpansionCredits, AT))?;
        let peak = previous.metrics.peak.max(unfolded);
        generation.check(BudgetKind::ExpansionCredits, credits, AT)?;
        generation.check(BudgetKind::PeakExpressionSize, peak, AT)?;
        let cost = previous
            .metrics
            .cost
            .checked_add(action.known.map_or_else(
                || self.policy.costs.weight(action.primitive),
                |known| Ok(known.cost),
            )?)
            .ok_or_else(|| Fault::capacity(BudgetKind::Transitions, AT))?;
        let witness = plan.check(&mut candidate, self.context, checker)?;
        if self.target.is_none_or(|target| target == endpoint) {
            candidate
                .recheck_witness(&witness, checker)
                .map_err(crate::producer::replay_fault)?;
        }
        let measured = metrics(&candidate, &witness, cost, peak, credits, generation)?;
        let mut trace = previous.trace.clone();
        trace.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        trace.push(candidate.handle(endpoint));
        self.insert(&candidate, plan, witness, measured, trace, generation)?;
        cas.state = candidate.state;
        Ok(())
    }

    /// Keep separate theorem classes and admit only labels surviving all resource dimensions.
    fn insert(
        &mut self,
        cas: &Cas,
        plan: Plan,
        witness: VerifiedWitness,
        measured: Metrics,
        trace: Vec<Handle>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let endpoint = cas.checked(&witness.claim.right)?;
        let rank = rank(cas, endpoint, self.target, budget)?;
        let key = crate::wire::claim_key(cas, &witness.claim, budget)?;
        let id = self.labels.len();
        if !self
            .frontier
            .insert(id, key.clone(), rank, measured.clone(), budget)?
        {
            return Ok(());
        }
        let actions = crate::rewrite::actions(
            cas,
            endpoint,
            self.target,
            &self.known,
            measured.credits != 0,
            budget,
        )?;
        let bytes = size(plan.steps.len(), AT)?
            .checked_mul(1024)
            .and_then(|n| n.checked_add(size(actions.len(), AT).ok()?.checked_mul(512)?))
            .and_then(|n| n.checked_add(size(witness.bytes.len(), AT).ok()?))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        let retained = self
            .labels
            .iter()
            .try_fold(bytes, |total, label| {
                total
                    .checked_add(size(label.plan.steps.len(), AT).ok()?.checked_mul(1024)?)
                    .and_then(|n| n.checked_add(size(label.witness.bytes.len(), AT).ok()?))
            })
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        budget.work(bytes, AT)?;
        let same_class = self
            .labels
            .first()
            .is_none_or(|initial| initial.witness.claim.residuals == witness.claim.residuals);
        let allowed = (self.policy.allow_conditional || witness.is_discharged())
            && self.target.is_none_or(|target| target == endpoint);
        if same_class
            && allowed
            && self.best.is_none_or(|previous| {
                self.labels.get(previous).is_some_and(|old| {
                    (rank, measured.cost, &key) < (old.rank, old.metrics.cost, &old.key)
                })
            })
        {
            self.best = Some(id);
        }
        self.labels
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.labels.push(Label {
            plan,
            witness,
            endpoint,
            key,
            rank,
            metrics: measured,
            actions,
            cursor: 0,
            trace,
        });
        Ok(())
    }

    /// Equality requests stop only at their exact target; simplification explores to its bound.
    fn reached(&self) -> bool {
        self.target.is_some_and(|goal| {
            self.best
                .and_then(|id| self.labels.get(id))
                .is_some_and(|label| label.endpoint == goal)
        })
    }

    /// Return only an already checked candidate; a target miss is explicitly unresolved.
    pub(crate) fn result(
        mut self,
        cas: &Cas,
        checker: &mut Budget,
    ) -> Result<Option<SearchResult>, Fault> {
        let Some(id) = self.best else {
            return Ok(None);
        };
        self.labels.get(id).ok_or_else(invalid)?;
        let label = self.labels.swap_remove(id);
        if self.target.is_some_and(|goal| label.endpoint != goal) {
            return Ok(None);
        }
        let mut statistics = self.statistics;
        statistics.dominated = self.frontier.dominated;
        Ok(Some(SearchResult {
            payload: None,
            witness: crate::proof_view::native_witness(cas, label.witness, checker)?,
            cost: label.metrics.cost,
            peak: label.metrics.peak,
            expansion_credits: label.metrics.credits,
            primitive_steps: u64::try_from(label.plan.steps.len()).map_err(|_| invalid())?,
            trace: label.trace,
            policy: self.policy.costs,
            statistics,
            replay_usage: checker.usage(),
            response: None,
        }))
    }
}

/// Charge a named monotone search counter before trying the next item.
fn increment(value: u64, kind: BudgetKind, budget: &Budget) -> Result<u64, Fault> {
    let next = value
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(kind, AT))?;
    budget.check(kind, next, AT)?;
    Ok(next)
}

/// Measure trusted proof payload and full support without consulting a producer success flag.
fn metrics(
    cas: &Cas,
    witness: &VerifiedWitness,
    cost: u64,
    peak: u64,
    credits: u64,
    budget: &mut Budget,
) -> Result<Metrics, Fault> {
    let proof = crate::proof_storage::application(cas, cas.checked(&witness.proof)?)?;
    let bytes = size(witness.bytes.len(), AT)?;
    budget.work(bytes, AT)?;
    let depth = proof.depth;
    let mut support = BTreeSet::new();
    let mut pending = vec![cas.checked(&witness.proof)?];
    while let Some(id) = pending.pop() {
        budget.work(
            size(support.len(), AT)?
                .checked_add(1)
                .ok_or_else(invalid)?,
            AT,
        )?;
        if !support.insert(id) {
            continue;
        }
        let value = crate::proof_storage::application(cas, id)?;
        support.insert(value.context);
        for required in &value.conditions.requirements {
            support.insert(required.term);
        }
        pending
            .try_reserve(value.premises.len())
            .map_err(|_| Fault::allocation(AT))?;
        pending.extend_from_slice(&value.premises);
    }
    Ok(Metrics {
        cost,
        peak,
        credits,
        depth,
        bytes,
        support,
        encoding: witness.bytes.clone(),
    })
}

/// Internal invalid indices remain values.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}

/// Rank local unresolved scalar operations before unfolded size and depth.
fn rank(
    cas: &Cas,
    root: LogicalId,
    target: Option<LogicalId>,
    budget: &mut Budget,
) -> Result<Rank, Fault> {
    use crate::model::Operator;

    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    let mut unresolved = 0_u64;
    while let Some(id) = pending.pop() {
        budget.work(
            size(visited.len(), AT)?
                .checked_add(1)
                .ok_or_else(invalid)?,
            AT,
        )?;
        if !visited.insert(id) {
            continue;
        }
        let expression = cas.state.expression(id)?;
        let repeated = expression.operands.len() == 2
            && expression.operands.first() == expression.operands.get(1);
        let literals = !expression.operands.is_empty()
            && expression.operands.iter().all(|child| {
                cas.state
                    .expression(*child)
                    .is_ok_and(|value| matches!(value.operator, Operator::Rational(_),))
            });
        let pending_scalar = matches!(
            expression.operator,
            Operator::Neg
                | Operator::Sub
                | Operator::Div
                | Operator::Call(_)
                | Operator::GeneralPower(_)
        ) || repeated
            || literals;
        unresolved = unresolved
            .checked_add(u64::from(pending_scalar))
            .ok_or_else(|| Fault::capacity(BudgetKind::PeakExpressionSize, AT))?;
        pending
            .try_reserve(expression.operands.len())
            .map_err(|_| Fault::allocation(AT))?;
        pending.extend_from_slice(&expression.operands);
    }
    let expression = cas.state.expression(root)?;
    Ok([
        u64::from(target.is_some_and(|goal| goal != root)),
        unresolved,
        expression.unfolded,
        expression.depth,
    ])
}

/// Native result dominance retains full proof/source support without a scalar claim projection.
pub(crate) fn typed_metrics(
    cas: &Cas,
    result: &SearchResult,
    request: &crate::request_model::Request,
    budget: &mut Budget,
) -> Result<Metrics, Fault> {
    let mut support: BTreeSet<_> = request
        .inputs
        .iter()
        .map(|input| input.root.logical_id())
        .collect();
    let mut pending = vec![result.witness.proof().logical_id()];
    let mut reached = BTreeSet::new();
    let mut depth = 0;
    while let Some(id) = pending.pop() {
        if !reached.insert(id) {
            continue;
        }
        let value = crate::proof_view::application(cas, id, budget)?;
        support.insert(id);
        support.insert(value.judgment.context.logical_id());
        for object in crate::statement::objects(&value.judgment.statement) {
            support.insert(object.logical_id());
        }
        pending.extend(value.premises);
        depth = depth.max(value.depth);
    }
    Ok(Metrics {
        cost: result.cost,
        peak: result.peak,
        credits: result.expansion_credits,
        depth,
        bytes: size(result.witness.bytes.len(), AT)?,
        support,
        encoding: result.witness.bytes.clone(),
    })
}
