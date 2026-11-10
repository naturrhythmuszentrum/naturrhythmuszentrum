// SPDX-License-Identifier: AGPL-3.0-only
//! Request execution with separate monotone generation and replay ledgers.

use crate::budget::{Budget, BudgetKind, BudgetUsage, Limits};
use crate::calculus::Deriver;
use crate::encoding::Encoder;
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, encode_id, size};
use crate::model::Operation;
use crate::request_model::{ContextConsistency, Outcome, Request, UnsupportedReason, failure};
use crate::search::{Search, SearchPolicy, SearchResult, SearchStatistics};

/// Service location for output and envelope accounting.
const AT: Location = Location::new("request execution");

/// A fully checked prefix and an optional reason that further generation stopped.
pub(crate) struct Execution {
    /// Only a kernel-checked candidate is retained.
    pub(crate) result: Option<SearchResult>,
    /// A failed search/generation stage does not certify absence of another route.
    pub(crate) fault: Option<Fault>,
}

impl Cas {
    /// Execute with a complete pinned cost policy and explicit conditional-result permission.
    ///
    /// Generated proof steps are independently reconstructed and replayed. Search makes no
    /// shortest-path claim. Unsupported calculus children remain unresolved, with a blocker.
    pub fn run_with_policy(&mut self, request: &Request, policy: &SearchPolicy) -> Outcome {
        let resolved = match self.limits.resolve(&request.limits) {
            Ok(value) => value,
            Err(fault) => return failure(fault, &BudgetUsage::default()),
        };
        let mut generation = match Budget::new(resolved.clone()) {
            Ok(value) => value,
            Err(fault) => return failure(fault, &BudgetUsage::default()),
        };
        generation.restrict_storage(&self.limits);
        let mut replay_limits = generation.limits().clone();
        replay_limits.work = replay_limits.replay_work;
        let mut checker = match Budget::new(replay_limits) {
            Ok(value) => value,
            Err(fault) => return failure(fault, &generation.usage()),
        };
        let prepared = self.prepare_execution(request, policy, &resolved, &mut generation);
        let (record, unsupported) = match prepared {
            Ok(value) => value,
            Err(fault) => return failure(fault, &generation.usage()),
        };
        if unsupported {
            return Outcome::Unsupported {
                request: record,
                reason: UnsupportedReason::Field,
                consistency: ContextConsistency::Unknown,
                usage: generation.usage(),
            };
        }
        if matches!(request.task, crate::task::Task::Solve(_)) {
            return crate::solving::execute(
                self,
                request,
                &record,
                policy,
                &mut generation,
                &mut checker,
            );
        }
        let mut execution = self.execute(request, policy, &mut generation, &mut checker);
        if let Some(result) = &mut execution.result {
            result.replay_usage = checker.usage();
            result.response =
                envelope(result, &resolved, &generation.usage(), &mut generation).ok();
        }
        outcome(record, execution, &generation.usage(), &checker.usage())
    }

    /// Validate policy and original receipts before publishing any operation work.
    fn prepare_execution(
        &mut self,
        request: &Request,
        policy: &SearchPolicy,
        limits: &Limits,
        generation: &mut Budget,
    ) -> Result<(Handle, bool), Fault> {
        policy.costs.validate(generation)?;
        self.transaction(generation, |candidate, generation| {
            candidate.pin_policy(&policy.costs, generation)?;
            candidate.prepare_request(request, limits.clone(), generation)
        })
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

    /// Native typed routes share request coverage while keeping their statements out of scalar search.
    fn native_execution(
        &mut self,
        request: &Request,
        inputs: &[LogicalId],
        policy: &SearchPolicy,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<Option<Execution>, Fault> {
        let context = self.checked(&request.context)?;
        if matches!(&request.task, crate::task::Task::Pullback { .. }) {
            self.check_nonempty_request(context, generation)?;
            let result = crate::pullback_producer::execute(
                self, request, inputs, policy, generation, checker,
            )?;
            return Ok(Some(Execution {
                result: Some(result),
                fault: None,
            }));
        }
        if matches!(
            &request.task,
            crate::task::Task::Limit(_) | crate::task::Task::Series(_)
        ) {
            self.check_nonempty_request(context, generation)?;
            let result =
                crate::germ_producer::execute(self, request, inputs, policy, generation, checker)?;
            return Ok(Some(Execution {
                result: Some(result),
                fault: None,
            }));
        }
        if matches!(
            &request.task,
            crate::task::Task::Integrate(_) | crate::task::Task::Sum(_)
        ) {
            self.check_nonempty_request(context, generation)?;
            let result =
                crate::split_producer::execute(self, request, inputs, policy, generation, checker)?;
            return Ok(Some(Execution {
                result: Some(result),
                fault: None,
            }));
        }
        if crate::distribution_search::handles(self, &request.task, inputs)? {
            self.check_nonempty_request(context, generation)?;
            return crate::distribution_search::execute(
                self, request, inputs, policy, generation, checker,
            )
            .map(Some);
        }
        if crate::boundary_runner::handles(self, &request.task, inputs, generation)? {
            self.check_nonempty_request(context, generation)?;
            return crate::boundary_runner::execute(
                self, request, inputs, policy, generation, checker,
            )
            .map(Some);
        }
        if crate::case_service::handles(self, &request.task, inputs, generation)? {
            self.check_nonempty_request(context, generation)?;
            let result =
                crate::case_service::execute(self, request, inputs, policy, generation, checker)?;
            return Ok(Some(Execution {
                result: Some(result),
                fault: None,
            }));
        }
        Ok(None)
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
            inputs.push(self.state.occurrence(self.checked(&root.root)?)?.expression);
        }
        if let Some(result) =
            self.native_execution(request, &inputs, policy, generation, checker)?
        {
            return Ok(result);
        }
        let Some(operation) = request.task.scalar() else {
            return Ok(Execution {
                result: None,
                fault: None,
            });
        };
        self.check_nonempty_request(context, generation)?;
        let first = inputs.first().copied().ok_or_else(invalid)?;
        match &operation {
            Operation::Simplify | Operation::Equal => {
                let target = if operation == Operation::Equal {
                    inputs.get(1).copied()
                } else {
                    None
                };
                let mut search = self.transaction(generation, |initial, generation| {
                    Search::new(initial, context, first, target, policy, generation, checker)
                })?;
                let fault = search.explore(self, generation, checker).err();
                Ok(Execution {
                    result: search.result(self, checker)?,
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
                let result = self
                    .calculus_result(context, &inputs, &operation, policy, generation, checker)?;
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
            Operation::Spectral => Ok(Execution {
                result: None,
                fault: None,
            }),
        }
    }

    /// Reject exactly proved empty function domains without claiming opaque domains are inhabited.
    fn check_nonempty_request(
        &self,
        context: LogicalId,
        generation: &mut Budget,
    ) -> Result<(), Fault> {
        let admissible = self.state.context(context)?.admissible;
        match crate::region_cache::partition(self, &[admissible], generation) {
            Ok(cells) => {
                if cells
                    .membership
                    .first()
                    .is_some_and(|row| row.iter().all(|value| !value))
                {
                    return Err(Fault::invalid(InvalidCode::EmptyDomain, AT));
                }
            }
            Err(Fault {
                kind: FaultKind::Invalid(InvalidCode::UnsupportedFragment),
                ..
            }) => {}
            Err(fault) => return Err(fault),
        }
        Ok(())
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
        self.transaction(generation, |candidate, generation| {
            let output = crate::polynomial::normalize(candidate, input, generation)?;
            let before = candidate.state.expression(input)?.unfolded;
            let after = candidate.state.expression(output)?.unfolded;
            let growth = after.saturating_sub(before);
            generation.check(BudgetKind::ExpansionCredits, growth, AT)?;
            let mut plan = crate::producer::Plan::default();
            plan.push(
                crate::producer::step(
                    candidate,
                    crate::rule::Primitive::Jets,
                    crate::scalar_model::Relation::EqualOnDomain,
                    input,
                    output,
                    Vec::new(),
                ),
                generation,
            )?;
            let witness = plan.verify(candidate, context, checker)?;
            if !policy.allow_conditional && !witness.is_discharged() {
                return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
            }
            let trace = vec![witness.claim.left.clone(), witness.claim.right.clone()];
            let witness = crate::proof_view::native_witness(candidate, witness, checker)?;
            let result = SearchResult {
                payload: None,
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
            Ok(result)
        })
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
        self.transaction(generation, |candidate, generation| {
            let plan =
                crate::square_free::produce(candidate, context, input, variable, generation)?;
            let proposal = plan.steps.last().ok_or_else(invalid)?;
            let output = candidate.checked(&proposal.right)?;
            let before = candidate.state.expression(input)?.unfolded;
            let after = candidate.state.expression(output)?.unfolded;
            let growth = after.saturating_sub(before);
            generation.check(BudgetKind::ExpansionCredits, growth, AT)?;
            let witness = plan.verify(candidate, context, checker)?;
            if !policy.allow_conditional && !witness.is_discharged() {
                return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
            }
            let trace = vec![witness.claim.left.clone(), witness.claim.right.clone()];
            let witness = crate::proof_view::native_witness(candidate, witness, checker)?;
            let result = SearchResult {
                payload: None,
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
            Ok(result)
        })
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
        self.transaction(generation, |candidate, generation| {
            let mut deriver = Deriver::default();
            let root = match operation {
                Operation::Differentiate(variable) => {
                    let input = inputs.first().copied().ok_or_else(invalid)?;
                    deriver
                        .derive(candidate, context, input, variable, generation)?
                        .1
                }
                Operation::Jacobian(variables) => {
                    deriver.jacobian(candidate, context, inputs, variables, generation)?
                }
                _ => return Err(invalid()),
            };
            let plan = deriver.plan.selected(root, generation)?;
            let witness = plan.verify(candidate, context, checker)?;
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
            let witness = crate::proof_view::native_witness(candidate, witness, checker)?;
            let result = SearchResult {
                payload: None,
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
            Ok(result)
        })
    }
}

/// Encode search metadata separately from the authoritative mathematical witness.
fn envelope(
    result: &SearchResult,
    limits: &Limits,
    generation_usage: &BudgetUsage,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::output(budget, AT);
    out.raw(b"NRZRUN04")?;
    result.policy.encode(&mut out)?;
    crate::wire::index(crate::solving::active_schema_bitmap(), &mut out)?;
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
    ] {
        out.unsigned(value)?;
    }
    generation_usage.encode(&mut out)?;
    result.replay_usage.encode(&mut out)?;
    out.count(result.trace.len())?;
    for endpoint in &result.trace {
        encode_id(endpoint.logical_id(), &mut out)?;
    }
    out.count(result.witness.bytes.len())?;
    out.raw(&result.witness.bytes)?;
    out.byte(u8::from(result.payload.is_some()))?;
    if let Some(payload) = &result.payload {
        payload.encode(&mut out)?;
    }
    Ok(out.finish())
}

/// Classification never changes a spent ledger into a successful complete search claim.
fn outcome(
    record: Handle,
    execution: Execution,
    usage: &BudgetUsage,
    replay_usage: &BudgetUsage,
) -> Outcome {
    if let Some(fault) = execution.fault {
        return match fault.kind {
            FaultKind::Invalid(
                InvalidCode::DistributionPointValue
                | InvalidCode::DistributionProductUndefined
                | InvalidCode::TestSpaceMismatch
                | InvalidCode::SeriesNonunitConstant
                | InvalidCode::SeriesNonzeroConstant
                | InvalidCode::RefutedDomain
                | InvalidCode::EmptyDomain
                | InvalidCode::OverlapConflict
                | InvalidCode::CoverageHole
                | InvalidCode::AnalyticDisagreement
                | InvalidCode::PullbackMultipleZero
                | InvalidCode::PullbackIncompleteZeros
                | InvalidCode::PullbackMapMismatch,
            ) => Outcome::Refused {
                reason: crate::distribution_search::refusal(&fault),
                request: record,
                fault,
                usage: *usage,
            },
            FaultKind::Exhausted { .. } => Outcome::Exhausted {
                solve_best: None,
                fault,
                usage: *usage,
                replay_usage: *replay_usage,
                best: execution.result.map(Box::new),
            },
            FaultKind::Invalid(InvalidCode::UnknownTotality) => Outcome::UnknownTotality {
                partial: execution.result.map(Box::new),
                request: record,
                blocker: fault,
                usage: *usage,
            },
            FaultKind::Invalid(InvalidCode::PendingPremise) => Outcome::PendingPremise {
                partial: execution.result.map(Box::new),
                request: record,
                blocker: fault,
                usage: *usage,
            },
            FaultKind::Invalid(
                InvalidCode::UnsupportedFragment
                | InvalidCode::ScalarTransportUnsupported
                | InvalidCode::SeriesUnsupported,
            ) => Outcome::Unresolved {
                solve_reason: None,
                request: record,
                consistency: ContextConsistency::Unknown,
                blocker: Some(fault),
                usage: *usage,
            },
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
                    usage: *usage,
                }
            }
            FaultKind::Invalid(_) => Outcome::Invalid {
                fault,
                usage: *usage,
            },
        };
    }
    completed_outcome(record, execution.result, usage)
}

/// Completion classification uses every retained component, including partial maps.
fn completed_outcome(record: Handle, result: Option<SearchResult>, usage: &BudgetUsage) -> Outcome {
    match result {
        Some(result)
            if result
                .payload
                .as_ref()
                .is_some_and(|value| value.partial() && value.discharged()) =>
        {
            Outcome::Partial {
                request: record,
                result,
                usage: *usage,
            }
        }
        Some(result)
            if result
                .payload
                .as_ref()
                .is_some_and(|value| value.nonexistence() && value.discharged()) =>
        {
            Outcome::Nonexistent {
                request: record,
                result,
                usage: *usage,
            }
        }
        Some(result)
            if result.witness.is_discharged()
                && result
                    .payload
                    .as_ref()
                    .is_none_or(|value| value.discharged()) =>
        {
            Outcome::Proved {
                request: record,
                result,
                usage: *usage,
            }
        }
        Some(result) => Outcome::Conditional {
            request: record,
            result,
            usage: *usage,
        },
        None => Outcome::Unresolved {
            solve_reason: None,
            request: record,
            consistency: ContextConsistency::Unknown,
            blocker: None,
            usage: *usage,
        },
    }
}

#[cfg(test)]
mod compose_outcomes {
    use super::*;
    use crate::graph::{Draft, Reference};
    use crate::proof_model::{ApplicationDraft, Certificate, Statement};
    use crate::request_model::CoveredInput;
    use crate::series::{
        SeriesCertificate as C, SeriesObject, SeriesOperation as O, SeriesProductRow,
        SeriesStatement as S,
    };
    use crate::task::Task;
    use crate::testing::common::TestResult;
    use crate::testing::native::{Fixture, admit, budget};

    /// Actual nonunit evidence remains owned when classified as a mathematical refusal.
    /// ´test:unit:reciprocal-public-outcome-evidence´
    #[test]
    fn reciprocal_public_outcome_evidence() -> TestResult {
        let mut f = crate::testing::series_residual::Fixture::new(&["0", "1"], "0", false, false)?;
        let input = f.object(&[crate::Rational::zero(), crate::Rational::one()])?;
        let fault = f
            .cas
            .compute_series(&O::Reciprocal { input }, &[], &mut budget()?)
            .expect_err("nonunit");
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::SeriesNonunitConstant)
        );
        assert_eq!(
            fault.series_constant().ok_or("owned zero")?.value,
            crate::Rational::zero()
        );
        let record = f.context.clone();
        let classified = outcome(
            record,
            Execution {
                result: None,
                fault: Some(fault.clone()),
            },
            &BudgetUsage::default(),
            &BudgetUsage::default(),
        );
        let Outcome::Refused {
            reason,
            fault: retained,
            ..
        } = classified
        else {
            return Err("refusal classification".into());
        };
        assert_eq!(
            reason,
            crate::request_model::RefusalReason::SeriesNonunitConstant
        );
        assert_eq!(retained, fault);
        Ok(())
    }

    /// Actual producer and supplied-candidate faults preserve the public mathematical split.
    /// ´test:unit:compose-public-outcome-evidence´
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "Each actual fault retains its request, evidence and rollback classification together"
    )]
    fn compose_public_outcome_evidence() -> TestResult {
        for production in [false, true] {
            for kind in 0..4 {
                let mut f = Fixture::new()?;
                let field = admit(
                    &mut f.cas,
                    Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
                        crate::solving::CoefficientDomainData::Rational,
                    )),
                )?;
                let mut objects = Vec::new();
                for coefficient in [&f.one, if kind == 0 { &f.one } else { &f.zero }] {
                    objects.push(admit(
                        &mut f.cas,
                        Draft::SeriesObject(SeriesObject {
                            field: Reference::Existing(field.clone()),
                            centre: Reference::Existing(f.zero.clone()),
                            variable: Reference::Existing(f.x.clone()),
                            scope: Vec::new(),
                            context: Reference::Existing(f.context.clone()),
                            order: 1,
                            coefficients: vec![Reference::Existing(coefficient.clone())],
                        }),
                    )?);
                }
                let left = objects[0].clone();
                let right = objects[1].clone();
                let prior_field = admit(
                    &mut f.cas,
                    Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
                        crate::solving::CoefficientDomainData::Rational,
                    )),
                )?;
                let prior_input = admit(
                    &mut f.cas,
                    Draft::SeriesObject(SeriesObject {
                        field: Reference::Existing(prior_field),
                        centre: Reference::Existing(f.zero.clone()),
                        variable: Reference::Existing(f.x.clone()),
                        scope: Vec::new(),
                        context: Reference::Existing(f.context.clone()),
                        order: 1,
                        coefficients: vec![Reference::Existing(f.one.clone())],
                    }),
                )?;
                let prior = f.cas.compute_series(
                    &O::Truncate {
                        input: prior_input,
                        order: 1,
                    },
                    &[],
                    &mut budget()?,
                )?;
                let unavailable_field = admit(
                    &mut f.cas,
                    Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
                        crate::solving::CoefficientDomainData::Gaussian,
                    )),
                )?;
                let mut ledger = budget()?;
                ledger.work(37, Location::new("composition outcome prior spend"))?;
                let root = f.cas.materialize_derived(&f.one, &f.one, &mut budget()?)?;
                let receipt = f.cas.walk_derived(&root, &mut budget()?)?.receipt;
                let request = Request {
                    task: Task::Simplify,
                    context: f.context.clone(),
                    inputs: vec![CoveredInput { root, receipt }],
                    premises: Vec::new(),
                    limits: crate::budget::LimitsOverride::default(),
                };
                let limits = ledger.limits().clone();
                let (record, unsupported) = f.cas.prepare_execution(
                    &request,
                    &SearchPolicy::default(),
                    &limits,
                    &mut ledger,
                )?;
                assert!(!unsupported);
                let before = f.cas.canonical_snapshot(&mut budget()?)?;
                let order = if kind == 3 { 2 } else { 1 };
                let certificate = C::Compose {
                    form: 0,
                    order,
                    default_order: false,
                    zero: if kind == 1 {
                        crate::Rational::one()
                    } else {
                        crate::Rational::zero()
                    },
                    seed: vec![crate::Rational::one()],
                    powers: Vec::new(),
                    sums: vec![SeriesProductRow {
                        degree: 0,
                        left_index: 0,
                        right_index: 0,
                        left: crate::Rational::one(),
                        right: crate::Rational::one(),
                        product: crate::Rational::one(),
                        sum: crate::Rational::one(),
                    }],
                };
                let statement = S::Compose {
                    left: left.clone(),
                    right: right.clone(),
                    output: left.clone(),
                    order,
                    default_order: false,
                };
                let draft = ApplicationDraft {
                    rule: statement.primitive(),
                    context: f.context.clone(),
                    conclusion: Statement::Series(Box::new(statement)),
                    premises: Vec::new(),
                    substitutions: Vec::new(),
                    certificate: Certificate::Series(Box::new(certificate)),
                };
                // Production owns its zero row; its malformed-evidence control supplies an extra
                // restriction instead. The supplied-candidate path mutates the zero witness.
                let proofs = if kind == 1 {
                    vec![prior.proof().clone()]
                } else {
                    Vec::new()
                };
                let fault = if kind == 2 {
                    // Both entry routes first admit their input objects. Unsupported fields
                    // stop at that shared boundary; no unavailable object can enter the store.
                    f.cas
                        .admit(
                            &[Draft::SeriesObject(SeriesObject {
                                field: Reference::Existing(unavailable_field),
                                centre: Reference::Existing(f.zero.clone()),
                                variable: Reference::Existing(f.x.clone()),
                                scope: Vec::new(),
                                context: Reference::Existing(f.context.clone()),
                                order: 1,
                                coefficients: vec![Reference::Existing(f.zero.clone())],
                            })],
                            &mut ledger,
                        )
                        .map(|_| ())
                } else if production {
                    f.cas
                        .compute_series(
                            &O::Compose {
                                left,
                                right,
                                order: Some(order),
                            },
                            &proofs,
                            &mut ledger,
                        )
                        .map(|_| ())
                } else {
                    f.cas
                        .check_proof(&f.context, &[draft], &mut ledger)
                        .map(|_| ())
                }
                .expect_err("composition refusal");
                let code = match kind {
                    0 => InvalidCode::SeriesNonzeroConstant,
                    1 => InvalidCode::Rule,
                    _ => InvalidCode::SeriesUnsupported,
                };
                assert_eq!(fault.kind, FaultKind::Invalid(code));
                assert_eq!(before, f.cas.canonical_snapshot(&mut budget()?)?);
                assert!(ledger.usage().work > 37);
                if kind == 0 {
                    assert_eq!(
                        fault
                            .series_constant()
                            .as_ref()
                            .ok_or("exact coefficient")?
                            .value,
                        crate::Rational::one()
                    );
                }
                if kind == 3 {
                    assert_eq!(fault.series_precision(), Some([1, 1, 2]));
                }
                let result = outcome(
                    record,
                    Execution {
                        result: None,
                        fault: Some(fault.clone()),
                    },
                    &ledger.usage(),
                    &BudgetUsage::default(),
                );
                match result {
                    Outcome::Refused {
                        reason,
                        fault: actual,
                        ..
                    } if kind == 0 => {
                        assert_eq!(
                            reason,
                            crate::request_model::RefusalReason::SeriesNonzeroConstant
                        );
                        assert_eq!(actual, fault);
                    }
                    Outcome::Invalid { fault: actual, .. } if kind == 1 => {
                        assert_eq!(actual, fault);
                    }
                    Outcome::Unresolved {
                        blocker: Some(actual),
                        ..
                    } if kind >= 2 => assert_eq!(actual, fault),
                    other => return Err(format!("composition classification: {other:?}").into()),
                }
            }
        }
        Ok(())
    }
}

/// Prepared input inconsistencies remain values.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_model::{
        AmbientDomain, ContextDraft, HeaderDraft, Layer, RegionDraft, RegionExpr,
    };
    use crate::graph::{Draft, ExpressionDraft, Reference};
    use crate::model::{AtomicSort, Environment, Field, Operator, PredicateKind, Sort};
    use crate::number::Rational;
    use crate::proof_model::{DomainContract, GuardedObligation, Judgment, Statement};
    use crate::request_model::CoveredInput;
    use crate::task::Task;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn budget() -> Result<Budget, Fault> {
        Budget::new(Limits {
            work: 2_000_000_000,
            ..Limits::default()
        })
    }

    fn real() -> Sort {
        Sort::Value(AtomicSort::Scalar(Field::Real))
    }

    fn region(expression: RegionExpr) -> Draft {
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: vec![],
            ambient: AmbientDomain::Assignments,
            expression,
        })
    }

    fn fixture() -> TestResult<(Cas, Vec<Handle>)> {
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let nodes = cas.admit(
            &[
                Draft::Header(HeaderDraft::Core(Environment {
                    declarations: vec![crate::model::Declaration {
                        key: "series-coordinate".into(),
                        display_name: "x".into(),
                        sort: real(),
                    }],
                    ..Environment::default()
                })),
                region(RegionExpr::Ambient),
                Draft::Context(ContextDraft::Root {
                    header: Reference::Local(0),
                    admissible: Reference::Local(1),
                    assumptions: vec![],
                }),
                Draft::Expression(ExpressionDraft {
                    header: Reference::Local(0),
                    scope: vec![],
                    space: None,
                    sort: real(),
                    operator: Operator::Rational(Rational::one()),
                    binding: None,
                    operands: vec![],
                }),
                Draft::Condition {
                    header: Reference::Local(0),
                    scope: vec![],
                    kind: PredicateKind::NonZero,
                    arguments: vec![Reference::Local(3)],
                },
                region(RegionExpr::GuardTrue {
                    base: Reference::Local(1),
                    predicate: Reference::Local(4),
                }),
                Draft::Context(ContextDraft::Local {
                    parent: Reference::Local(2),
                    region: Reference::Local(5),
                    hypotheses: vec![],
                }),
            ],
            &mut budget()?,
        )?;
        Ok((cas, nodes))
    }

    fn claim(cas: &mut Cas, nodes: &[Handle], kind: u8) -> TestResult<Judgment> {
        let m = cas.admissible_region(&nodes[6])?;
        let expression = if kind == 0 {
            RegionExpr::Meet(vec![Reference::Existing(nodes[1].clone())])
        } else {
            RegionExpr::Meet(vec![
                Reference::Existing(m.clone()),
                Reference::Existing(nodes[5].clone()),
            ])
        };
        let transported = cas
            .admit(
                &[Draft::Region(RegionDraft {
                    header: Reference::Existing(nodes[0].clone()),
                    scope: vec![],
                    ambient: AmbientDomain::Assignments,
                    expression,
                })],
                &mut budget()?,
            )?
            .remove(0);
        Ok(Judgment {
            context: nodes[6].clone(),
            layer: Layer::Pointwise,
            space: None,
            statement: Statement::Equality {
                left: nodes[3].clone(),
                right: nodes[3].clone(),
                domain: m,
                domain_contract: DomainContract::DefinedAndEqual,
            },
            residuals: vec![GuardedObligation {
                home: nodes[if kind == 2 { 2 } else { 6 }].clone(),
                scope: vec![],
                layer: Layer::Pointwise,
                space: None,
                region: if kind == 3 {
                    nodes[3].clone()
                } else {
                    transported
                },
                predicate: nodes[4].clone(),
            }],
        })
    }

    /// Actual transport refusals preserve request classification and candidate rollback.
    /// ´test:unit:scalar-transport-request-outcome´
    #[test]
    fn scalar_transport_request_outcome() -> TestResult {
        for kind in 0..5 {
            let (mut cas, nodes) = fixture()?;
            let claim = claim(&mut cas, &nodes, kind)?;
            let root = cas.materialize_derived(&nodes[3], &nodes[3], &mut budget()?)?;
            let receipt = cas.walk_derived(&root, &mut budget()?)?.receipt;
            let request = Request {
                task: Task::Simplify,
                context: nodes[6].clone(),
                inputs: vec![CoveredInput { root, receipt }],
                premises: vec![],
                limits: crate::budget::LimitsOverride::default(),
            };
            let mut ledger = budget()?;
            let limits = ledger.limits().clone();
            let (record, unsupported) =
                cas.prepare_execution(&request, &SearchPolicy::default(), &limits, &mut ledger)?;
            assert!(!unsupported);
            let before = cas.canonical_snapshot(&mut budget()?)?;
            let header = cas.checked(&nodes[0])?;
            // Feed the completion classifier an actual conversion fault, not a fabricated code.
            // The candidate allocation makes rollback observable independently of classification.
            let result = cas.transaction(&mut ledger, |cas, ledger| {
                cas.generated(
                    header,
                    Operator::Rational(Rational::from_ratio(97, 1, ledger)?),
                    real(),
                    &[],
                    ledger,
                )?;
                if kind == 4 {
                    let variable = cas.generated(
                        header,
                        Operator::Free("series-coordinate".into()),
                        real(),
                        &[],
                        ledger,
                    )?;
                    let field = cas
                        .admit(
                            &[crate::graph::Draft::Solve(
                                crate::solving::SolveDraft::CoefficientDomain(
                                    crate::solving::CoefficientDomainData::Gaussian,
                                ),
                            )],
                            ledger,
                        )?
                        .pop()
                        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?;
                    cas.admit(
                        &[crate::graph::Draft::SeriesObject(
                            crate::series::SeriesObject {
                                field: crate::graph::Reference::Existing(field),
                                centre: crate::graph::Reference::Existing(nodes[3].clone()),
                                variable: crate::graph::Reference::Existing(cas.handle(variable)),
                                scope: Vec::new(),
                                context: crate::graph::Reference::Existing(nodes[6].clone()),
                                order: 1,
                                coefficients: vec![crate::graph::Reference::Existing(
                                    nodes[3].clone(),
                                )],
                            },
                        )],
                        ledger,
                    )?;
                }
                crate::proof_view::scalar_claim(cas, &claim, ledger)
            });
            let fault = result.err().ok_or("unsupported scalar view accepted")?;
            let code = match kind {
                0 | 1 => InvalidCode::ScalarTransportUnsupported,
                2 => InvalidCode::UnsupportedFragment,
                4 => InvalidCode::SeriesUnsupported,
                _ => InvalidCode::Reference,
            };
            assert_eq!(fault.kind, FaultKind::Invalid(code));
            let actual = outcome(
                record.clone(),
                Execution {
                    result: None,
                    fault: Some(fault.clone()),
                },
                &ledger.usage(),
                &BudgetUsage::default(),
            );
            match actual {
                Outcome::Unresolved {
                    request, blocker, ..
                } if kind != 3 => {
                    assert_eq!(request, record);
                    assert_eq!(blocker, Some(fault));
                }
                Outcome::Invalid { fault: actual, .. } if kind == 3 => assert_eq!(actual, fault),
                other => return Err(format!("changed scalar capability outcome: {other:?}").into()),
            }
            assert_eq!(cas.canonical_snapshot(&mut budget()?)?, before);
        }
        Ok(())
    }

    /// Actual calculus producer faults preserve unavailable capability versus malformed scope.
    /// ´test:unit:series-calculus-request-outcomes´
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "Each complete mutation retains its setup, precise fault and rollback evidence together"
    )]
    fn series_calculus_request_outcomes() -> TestResult {
        use crate::series::{SeriesObject, SeriesOperation};
        use crate::testing::native::{Fixture, admit, term};
        for kind in 0..4 {
            let mut f = Fixture::new()?;
            let field = admit(
                &mut f.cas,
                Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
                    crate::solving::CoefficientDomainData::Rational,
                )),
            )?;
            let input = admit(
                &mut f.cas,
                Draft::SeriesObject(SeriesObject {
                    field: Reference::Existing(field),
                    centre: Reference::Existing(f.zero.clone()),
                    variable: Reference::Existing(f.x.clone()),
                    scope: Vec::new(),
                    context: Reference::Existing(f.context.clone()),
                    order: 1,
                    coefficients: vec![Reference::Existing(f.one.clone())],
                }),
            )?;
            let mut proofs = Vec::new();
            let operation = if kind == 0 {
                SeriesOperation::Derive { input }
            } else {
                let constant = match kind {
                    1 => term(
                        &mut f.cas,
                        &f.header,
                        Operator::Constant(crate::model::Constant::Pi),
                        &[],
                    )?,
                    2 => f.x.clone(),
                    _ => term(
                        &mut f.cas,
                        &f.header,
                        Operator::Mul,
                        &[f.zero.clone(), f.x.clone()],
                    )?,
                };
                let predicate = admit(
                    &mut f.cas,
                    Draft::Condition {
                        header: Reference::Existing(f.header.clone()),
                        scope: Vec::new(),
                        kind: PredicateKind::Defined,
                        arguments: vec![Reference::Existing(constant.clone())],
                    },
                )?;
                let mut draft = crate::proof_model::ApplicationDraft {
                    rule: crate::rule::Primitive::CaseDomain,
                    context: f.context.clone(),
                    conclusion: Statement::PredicateFact {
                        region: f.domain.clone(),
                        predicate,
                    },
                    premises: Vec::new(),
                    substitutions: Vec::new(),
                    certificate: crate::proof_model::Certificate::None,
                };
                let (certificate, _) =
                    crate::case_check::reconstruct(&mut f.cas, &draft, &[], &mut budget()?)?;
                draft.certificate = crate::proof_model::Certificate::Cases(Box::new(certificate));
                proofs.push(
                    f.cas
                        .check_proof(&f.context, &[draft], &mut budget()?)?
                        .proof()
                        .clone(),
                );
                SeriesOperation::Integrate { input, constant }
            };
            let root = f.cas.materialize_derived(&f.one, &f.one, &mut budget()?)?;
            let receipt = f.cas.walk_derived(&root, &mut budget()?)?.receipt;
            let request = Request {
                task: Task::Simplify,
                context: f.context.clone(),
                inputs: vec![CoveredInput { root, receipt }],
                premises: Vec::new(),
                limits: crate::budget::LimitsOverride::default(),
            };
            let mut ledger = budget()?;
            let limits = ledger.limits().clone();
            let (record, unsupported) = f.cas.prepare_execution(
                &request,
                &SearchPolicy::default(),
                &limits,
                &mut ledger,
            )?;
            assert!(!unsupported);
            let before = f.cas.canonical_snapshot(&mut budget()?)?;
            let fault = f
                .cas
                .compute_series(&operation, &proofs, &mut ledger)
                .expect_err("calculus capability");
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(if kind < 2 {
                    InvalidCode::SeriesUnsupported
                } else {
                    InvalidCode::Scope
                })
            );
            let actual = outcome(
                record.clone(),
                Execution {
                    result: None,
                    fault: Some(fault.clone()),
                },
                &ledger.usage(),
                &BudgetUsage::default(),
            );
            match actual {
                Outcome::Unresolved {
                    request, blocker, ..
                } if kind < 2 => {
                    assert_eq!(request, record);
                    assert_eq!(blocker, Some(fault));
                }
                Outcome::Invalid { fault: actual, .. } if kind >= 2 => assert_eq!(actual, fault),
                other => return Err(format!("changed calculus outcome: {other:?}").into()),
            }
            assert_eq!(f.cas.canonical_snapshot(&mut budget()?)?, before);
        }
        Ok(())
    }

    /// Actual source entry faults retain the public capability and invalid-evidence distinction.
    /// ´test:unit:series-source-request-outcomes´
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "Actual entry faults and request completion share their complete source setup"
    )]
    fn series_source_request_outcomes() -> TestResult {
        use crate::series::{SourceClause as C, SourceRequest};
        use crate::testing::native::{Fixture, admit};
        let mut f = Fixture::new()?;
        let field = admit(
            &mut f.cas,
            Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
                crate::solving::CoefficientDomainData::Rational,
            )),
        )?;
        let root = f.cas.materialize_derived(&f.one, &f.one, &mut budget()?)?;
        let receipt = f.cas.walk_derived(&root, &mut budget()?)?.receipt;
        let request = Request {
            task: Task::Simplify,
            context: f.context.clone(),
            inputs: vec![CoveredInput { root, receipt }],
            premises: Vec::new(),
            limits: crate::budget::LimitsOverride::default(),
        };
        for clause in [
            C::SineOrigin,
            C::CosineOrigin,
            C::ArctangentOrigin,
            C::PolynomialSeed,
            C::Truncate,
            C::Add,
            C::Sub,
            C::ScaleLeft,
            C::ScaleRight,
            C::Neg,
            C::Mul,
        ] {
            let mut ledger = budget()?;
            let limits = ledger.limits().clone();
            let (record, unsupported) = f.cas.prepare_execution(
                &request,
                &SearchPolicy::default(),
                &limits,
                &mut ledger,
            )?;
            assert!(!unsupported);
            let source = SourceRequest {
                clause,
                source: f.x.clone(),
                domain: f.domain.clone(),
                field: field.clone(),
                centre: f.zero.clone(),
                variable: f.x.clone(),
                scope: Vec::new(),
                context: f.context.clone(),
                order: 1,
            };
            let before = f.cas.canonical_snapshot(&mut budget()?)?;
            let fault = f
                .cas
                .expand_series_source(&source, &[], &mut ledger)
                .expect_err("missing typed premise");
            assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
            assert!(
                matches!(outcome(record,Execution{result:None,fault:Some(fault.clone())},&ledger.usage(),&BudgetUsage::default()),Outcome::Invalid{fault:actual,..} if actual==fault)
            );
            assert_eq!(f.cas.canonical_snapshot(&mut budget()?)?, before);
        }
        let predicate = admit(
            &mut f.cas,
            Draft::Condition {
                header: Reference::Existing(f.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(f.one.clone())],
            },
        )?;
        let mut draft = crate::proof_model::ApplicationDraft {
            rule: crate::rule::Primitive::CaseDomain,
            context: f.context.clone(),
            conclusion: Statement::PredicateFact {
                region: f.domain.clone(),
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: crate::proof_model::Certificate::None,
        };
        let (certificate, _) =
            crate::case_check::reconstruct(&mut f.cas, &draft, &[], &mut budget()?)?;
        draft.certificate = crate::proof_model::Certificate::Cases(Box::new(certificate));
        let ordinary = f.cas.check_proof(&f.context, &[draft], &mut budget()?)?;
        let pi = crate::testing::native::term(
            &mut f.cas,
            &f.header,
            Operator::Constant(crate::model::Constant::Pi),
            &[],
        )?;
        for clause in [
            C::SineOrigin,
            C::CosineOrigin,
            C::ArctangentOrigin,
            C::PolynomialSeed,
        ] {
            let mut ledger = budget()?;
            let limits = ledger.limits().clone();
            let (record, unsupported) = f.cas.prepare_execution(
                &request,
                &SearchPolicy::default(),
                &limits,
                &mut ledger,
            )?;
            assert!(!unsupported);
            let source = SourceRequest {
                clause,
                source: f.one.clone(),
                domain: f.domain.clone(),
                field: field.clone(),
                centre: pi.clone(),
                variable: f.x.clone(),
                scope: Vec::new(),
                context: f.context.clone(),
                order: 1,
            };
            let before = f.cas.canonical_snapshot(&mut budget()?)?;
            let fault = f
                .cas
                .expand_series_source(&source, &[ordinary.proof().clone()], &mut ledger)
                .expect_err("unavailable coefficient centre");
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::SeriesUnsupported)
            );
            assert!(
                matches!(outcome(record.clone(),Execution{result:None,fault:Some(fault.clone())},&ledger.usage(),&BudgetUsage::default()),Outcome::Unresolved{request,blocker,..} if request==record && blocker==Some(fault))
            );
            assert_eq!(f.cas.canonical_snapshot(&mut budget()?)?, before);
        }
        Ok(())
    }
}
