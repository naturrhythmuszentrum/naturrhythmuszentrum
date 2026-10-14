// SPDX-License-Identifier: AGPL-3.0-only
//! Request execution with separate monotone generation and replay ledgers.

use crate::budget::{Budget, BudgetKind, BudgetUsage, Limits};
use crate::calculus::Deriver;
use crate::encoding::Encoder;
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, encode_id, size};
use crate::model::Operation;
use crate::request::{ContextConsistency, Outcome, Request, UnsupportedReason, failure};
use crate::search::{Search, SearchPolicy, SearchResult, SearchStatistics};

/// Service location for output and envelope accounting.
const AT: Location = Location::new("request execution");

/// A fully checked prefix and an optional reason that further generation stopped.
struct Execution {
    /// Only a kernel-checked candidate is retained.
    result: Option<SearchResult>,
    /// A failed search/generation stage does not certify absence of another route.
    fault: Option<Fault>,
}

impl Cas {
    /// Execute with a complete pinned cost policy and explicit conditional-result permission.
    ///
    /// Generated proof steps are independently reconstructed and replayed. Search makes no
    /// shortest-path claim. Unsupported calculus children remain unresolved, with a blocker.
    pub fn run_with_policy(&mut self, request: &Request, policy: &SearchPolicy) -> Outcome {
        let resolved = match self.limits.resolve(&request.limits) {
            Ok(value) => value,
            Err(fault) => return failure(fault, BudgetUsage::default()),
        };
        let mut generation = match Budget::new(resolved.clone()) {
            Ok(value) => value,
            Err(fault) => return failure(fault, BudgetUsage::default()),
        };
        generation.restrict_storage(&self.limits);
        let mut replay_limits = generation.limits().clone();
        replay_limits.work = replay_limits.replay_work;
        let mut checker = match Budget::new(replay_limits) {
            Ok(value) => value,
            Err(fault) => return failure(fault, generation.usage()),
        };
        let prepared = self.prepare_execution(request, policy, &resolved, &mut generation);
        let (mut candidate, record, unsupported) = match prepared {
            Ok(value) => value,
            Err(fault) => return failure(fault, generation.usage()),
        };
        if unsupported {
            self.state = candidate.state;
            return Outcome::Unsupported {
                request: record,
                reason: UnsupportedReason::Field,
                consistency: ContextConsistency::Unknown,
                usage: generation.usage(),
            };
        }
        let mut execution = candidate.execute(request, policy, &mut generation, &mut checker);
        if let Some(result) = &mut execution.result {
            result.replay_usage = checker.usage();
            result.response = envelope(result, &resolved, generation.usage(), &mut generation).ok();
        }
        // Only complete expression/proof batches live in this candidate; the failed proposal
        // has already been dropped by its transaction boundary.
        self.state = candidate.state;
        outcome(record, execution, generation.usage(), checker.usage())
    }

    /// Validate policy and original receipts before publishing any operation work.
    fn prepare_execution(
        &self,
        request: &Request,
        policy: &SearchPolicy,
        limits: &Limits,
        generation: &mut Budget,
    ) -> Result<(Self, Handle, bool), Fault> {
        policy.costs.validate(generation)?;
        let mut candidate = self.fork(generation)?;
        candidate.pin_policy(&policy.costs, generation)?;
        let (record, unsupported) =
            candidate.prepare_request(request, limits.clone(), generation)?;
        Ok((candidate, record, unsupported))
    }

    /// Dispatch a prepared operation, retaining a checked search prefix on later exhaustion.
    fn execute(
        &mut self,
        request: &Request,
        policy: &SearchPolicy,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Execution {
        let result = self.execute_inner(request, policy, generation, checker);
        match result {
            Ok(result) => result,
            Err(fault) => Execution {
                result: None,
                fault: Some(fault),
            },
        }
    }

    /// Resolve occurrence roots to immutable syntax before choosing an admitted producer.
    fn execute_inner(
        &mut self,
        request: &Request,
        policy: &SearchPolicy,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<Execution, Fault> {
        let context = self.checked(&request.context)?;
        let mut inputs = Vec::new();
        inputs
            .try_reserve_exact(request.inputs.len())
            .map_err(|_| Fault::allocation(AT))?;
        for root in &request.inputs {
            generation.work(1, AT)?;
            inputs.push(self.state.occurrence(self.checked(root)?)?.expression);
        }
        let first = inputs.first().copied().ok_or_else(invalid)?;
        match &request.operation {
            Operation::Simplify | Operation::Equal => {
                let target = if request.operation == Operation::Equal {
                    inputs.get(1).copied()
                } else {
                    None
                };
                let mut initial = self.fork(generation)?;
                let mut search = Search::new(
                    &mut initial,
                    context,
                    first,
                    target,
                    policy,
                    generation,
                    checker,
                )?;
                self.state = initial.state;
                let fault = search.explore(self, generation, checker).err();
                Ok(Execution {
                    result: search.result(checker),
                    fault,
                })
            }
            Operation::Expand => {
                let result = self.expansion_result(context, first, policy, generation, checker)?;
                Ok(Execution {
                    result: Some(result),
                    fault: None,
                })
            }
            Operation::Differentiate(_) | Operation::Jacobian(_) => {
                let result = self.calculus_result(
                    context,
                    &inputs,
                    &request.operation,
                    policy,
                    generation,
                    checker,
                )?;
                Ok(Execution {
                    result: Some(result),
                    fault: None,
                })
            }
            Operation::SquareFree(variable) => {
                let result =
                    self.square_free_result(context, first, variable, policy, generation, checker)?;
                Ok(Execution {
                    result: Some(result),
                    fault: None,
                })
            }
            _ => Ok(Execution {
                result: None,
                fault: None,
            }),
        }
    }

    /// Materialize the finite polynomial coefficient form and independently check that edge.
    fn expansion_result(
        &mut self,
        context: LogicalId,
        input: LogicalId,
        policy: &SearchPolicy,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<SearchResult, Fault> {
        let mut candidate = self.fork(generation)?;
        let output = crate::polynomial::normalize(&mut candidate, input, generation)?;
        let before = candidate.state.expression(input)?.unfolded;
        let after = candidate.state.expression(output)?.unfolded;
        let growth = after.saturating_sub(before);
        generation.check(BudgetKind::ExpansionCredits, growth, AT)?;
        let mut plan = crate::producer::Plan::default();
        plan.push(
            crate::producer::step(
                &candidate,
                crate::rule::Primitive::Jets,
                crate::proof_model::Relation::EqualOnDomain,
                input,
                output,
                Vec::new(),
            ),
            generation,
        )?;
        let witness = plan.verify(&mut candidate, context, checker)?;
        if !policy.allow_conditional && !witness.is_discharged() {
            return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
        }
        let trace = vec![witness.claim.left.clone(), witness.claim.right.clone()];
        let result = SearchResult {
            witness,
            cost: policy.costs.weight(crate::rule::Primitive::Jets)?,
            peak: before.max(after),
            expansion_credits: growth,
            primitive_steps: 1,
            trace,
            policy: policy.costs.clone(),
            statistics: SearchStatistics::default(),
            replay_usage: checker.usage(),
            response: None,
        };
        self.state = candidate.state;
        Ok(result)
    }

    /// Polynomial evidence and its displayed factors are published as one checked transaction.
    fn square_free_result(
        &mut self,
        context: LogicalId,
        input: LogicalId,
        variable: &str,
        policy: &SearchPolicy,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<SearchResult, Fault> {
        let mut candidate = self.fork(generation)?;
        let plan =
            crate::square_free::produce(&mut candidate, context, input, variable, generation)?;
        let proposal = plan.steps.last().ok_or_else(invalid)?;
        let output = candidate.checked(&proposal.right)?;
        let before = candidate.state.expression(input)?.unfolded;
        let after = candidate.state.expression(output)?.unfolded;
        let growth = after.saturating_sub(before);
        generation.check(BudgetKind::ExpansionCredits, growth, AT)?;
        let witness = plan.verify(&mut candidate, context, checker)?;
        if !policy.allow_conditional && !witness.is_discharged() {
            return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
        }
        let trace = vec![witness.claim.left.clone(), witness.claim.right.clone()];
        let result = SearchResult {
            witness,
            cost: policy.costs.weight(crate::rule::Primitive::SquareFree)?,
            peak: before.max(after),
            expansion_credits: growth,
            primitive_steps: 1,
            trace,
            policy: policy.costs.clone(),
            statistics: SearchStatistics::default(),
            replay_usage: checker.usage(),
            response: None,
        };
        self.state = candidate.state;
        Ok(result)
    }

    /// Derivative generation is one transaction; no partial output matrix is published.
    fn calculus_result(
        &mut self,
        context: LogicalId,
        inputs: &[LogicalId],
        operation: &Operation,
        policy: &SearchPolicy,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<SearchResult, Fault> {
        let mut candidate = self.fork(generation)?;
        let mut deriver = Deriver::default();
        let root = match operation {
            Operation::Differentiate(variable) => {
                let input = inputs.first().copied().ok_or_else(invalid)?;
                deriver
                    .derive(&mut candidate, input, variable, generation)?
                    .1
            }
            Operation::Jacobian(variables) => {
                deriver.jacobian(&mut candidate, context, inputs, variables, generation)?
            }
            _ => return Err(invalid()),
        };
        let plan = deriver.plan.selected(root, generation)?;
        let witness = plan.verify(&mut candidate, context, checker)?;
        if !policy.allow_conditional && !witness.is_discharged() {
            return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
        }
        let cost = policy
            .costs
            .weight(plan.steps.last().ok_or_else(invalid)?.primitive)?;
        let mut peak = 0_u64;
        for proposal in &plan.steps {
            generation.work(1, AT)?;
            for endpoint in [&proposal.left, &proposal.right] {
                peak = peak.max(
                    candidate
                        .state
                        .expression(candidate.checked(endpoint)?)?
                        .unfolded,
                );
            }
        }
        let trace = vec![witness.claim.left.clone(), witness.claim.right.clone()];
        let result = SearchResult {
            witness,
            cost,
            peak,
            expansion_credits: 0,
            primitive_steps: size(plan.steps.len(), AT)?,
            trace,
            policy: policy.costs.clone(),
            statistics: SearchStatistics::default(),
            replay_usage: checker.usage(),
            response: None,
        };
        self.state = candidate.state;
        Ok(result)
    }
}

/// Encode search metadata separately from the authoritative mathematical witness.
fn envelope(
    result: &SearchResult,
    limits: &Limits,
    generation_usage: BudgetUsage,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::output(budget, AT);
    out.raw(b"NRZRUN01")?;
    result.policy.encode(&mut out)?;
    for kind in crate::budget::ALL_KINDS {
        out.unsigned(limits.get(kind))?;
    }
    for value in [
        result.cost,
        result.peak,
        result.expansion_credits,
        result.primitive_steps,
        result.statistics.matches,
        result.statistics.transitions,
        result.statistics.dominated,
        result.statistics.visits,
        generation_usage.work,
        generation_usage.arithmetic_steps,
        generation_usage.condition_steps,
        result.replay_usage.work,
        result.replay_usage.arithmetic_steps,
        result.replay_usage.condition_steps,
    ] {
        out.unsigned(value)?;
    }
    out.count(result.trace.len())?;
    for endpoint in &result.trace {
        encode_id(endpoint.logical_id(), &mut out)?;
    }
    out.count(result.witness.bytes.len())?;
    out.raw(&result.witness.bytes)?;
    Ok(out.finish())
}

/// Classification never changes a spent ledger into a successful complete search claim.
fn outcome(
    record: Handle,
    execution: Execution,
    usage: BudgetUsage,
    replay_usage: BudgetUsage,
) -> Outcome {
    if let Some(fault) = execution.fault {
        return match fault.kind {
            FaultKind::Exhausted { .. } => Outcome::Exhausted {
                fault,
                usage,
                replay_usage,
                best: execution.result,
            },
            FaultKind::Invalid(InvalidCode::UnsupportedFragment | InvalidCode::RefutedDomain) => {
                Outcome::Unresolved {
                    request: record,
                    consistency: ContextConsistency::Unknown,
                    blocker: Some(fault),
                    usage,
                }
            }
            FaultKind::Invalid(
                code @ (InvalidCode::PolynomialFragment
                | InvalidCode::CoefficientField
                | InvalidCode::ZeroPolynomial),
            ) => {
                let reason = match code {
                    InvalidCode::CoefficientField => UnsupportedReason::CoefficientField,
                    InvalidCode::ZeroPolynomial => UnsupportedReason::ZeroPolynomialMultiplicity,
                    _ => UnsupportedReason::PolynomialFragment,
                };
                Outcome::Unsupported {
                    request: record,
                    reason,
                    consistency: ContextConsistency::Unknown,
                    usage,
                }
            }
            FaultKind::Invalid(_) => Outcome::Invalid { fault, usage },
        };
    }
    match execution.result {
        Some(result) if result.witness.is_discharged() => Outcome::Verified {
            request: record,
            result,
            usage,
        },
        Some(result) => Outcome::Conditional {
            request: record,
            result,
            usage,
        },
        None => Outcome::Unresolved {
            request: record,
            consistency: ContextConsistency::Unknown,
            blocker: None,
            usage,
        },
    }
}

/// Prepared input inconsistencies remain values.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
