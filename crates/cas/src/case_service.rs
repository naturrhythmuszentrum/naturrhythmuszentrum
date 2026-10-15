// SPDX-License-Identifier: AGPL-3.0-only
//! Native request routing and atomic publication of independently replayed recipes.

use crate::budget::Budget;
use crate::case_producer::Plan;
use crate::case_syntax::{self, invalid};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId};
use crate::proof_model::{Certificate, DomainContract, Statement};
use crate::request_model::Request;
use crate::rule::Primitive;
use crate::search::{SearchPolicy, SearchResult};
use crate::task::{BoundaryFact, Task};

/// Native syntax cannot enter the legacy scalar projection by an unrecognized rule.
pub(crate) fn handles(
    cas: &Cas,
    task: &Task,
    inputs: &[LogicalId],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    match task {
        Task::ElaborateCases
        | Task::Select { .. }
        | Task::Merge { .. }
        | Task::Eliminate { .. }
        | Task::Coverage { .. }
        | Task::Boundary { .. }
        | Task::Equal {
            domain_contract: DomainContract::SameDefinedDomain,
            ..
        } => Ok(true),
        Task::Simplify | Task::Equal { .. } => {
            for id in case_syntax::order(cas, inputs, budget)? {
                if cas.state.expression(id)?.domain.is_some() {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// The entire recipe, including fresh-store replay, is one unpublished candidate.
pub(crate) fn execute(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    policy: &SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<SearchResult, Fault> {
    cas.transaction(generation, |candidate, generation| {
        let mut runner = Runner {
            cas: candidate,
            request,
            plan: Plan::new(policy),
            generation,
            checker,
        };
        let proof = match runner.run(inputs) {
            Ok(proof) => proof,
            Err(fault) => {
                return Err(classify_context(
                    runner.cas,
                    request,
                    fault,
                    runner.generation,
                )?);
            }
        };
        let mut trace: Vec<_> = inputs.iter().map(|id| runner.cas.handle(*id)).collect();
        if let Statement::Equality { right, .. } | Statement::Elaboration { result: right, .. } =
            &crate::proof_view::application(runner.cas, proof.logical_id(), runner.checker)?
                .judgment
                .statement
        {
            trace.push(right.clone());
        }
        let result = runner
            .plan
            .finish(runner.cas, &proof, trace, runner.checker)?;
        Ok(result)
    })
}

/// One recipe shares the request's generation ledger, checker ledger and positive cost ledger.
struct Runner<'a> {
    /// Unpublished graph candidate.
    cas: &'a mut Cas,
    /// Exact request, including original receipts and supplied checked premises.
    request: &'a Request,
    /// Checked primitive expansion and policy.
    plan: Plan<'a>,
    /// Monotone producer budget, including nested refinement.
    generation: &'a mut Budget,
    /// Independent replay budget.
    checker: &'a mut Budget,
}

impl Runner<'_> {
    /// Resolve only producers whose semantics exactly match the requested task.
    fn run(&mut self, inputs: &[LogicalId]) -> Result<Handle, Fault> {
        self.nonempty(inputs)?;
        if let Task::Eliminate {
            regions,
            conclusion,
        } = &self.request.task
        {
            return self.eliminate(regions, conclusion);
        }
        let input = self.cas.handle(*inputs.first().ok_or_else(invalid)?);
        match &self.request.task {
            Task::ElaborateCases => self.elaborate(&input),
            Task::Select { region } => self.select(&input, region),
            Task::Merge {
                regions,
                proposed_value,
            } => self.merge(&input, proposed_value, regions),
            Task::Coverage { domain } => self.coverage(&input, domain),
            Task::Boundary {
                point,
                requested_facts,
                ..
            } if requested_facts == &[BoundaryFact::Ownership] => self.ownership(&input, point),
            Task::Equal {
                domain,
                domain_contract,
            } => {
                let right = self.cas.handle(*inputs.get(1).ok_or_else(invalid)?);
                self.equal(&input, &right, domain, *domain_contract)
            }
            Task::Simplify => self.simplify(&input),
            _ => Err(pending()),
        }
    }

    /// Elimination uses exactly the declared cover and supplied closed member proofs.
    fn eliminate(&mut self, regions: &[Handle], conclusion: &Statement) -> Result<Handle, Fault> {
        let premises: Vec<_> = self
            .request
            .premises
            .iter()
            .map(|proof| proof.proof().clone())
            .collect();
        let first = self.request.premises.first().ok_or_else(pending)?;
        if !matches!(&first.judgment().statement, Statement::Cover { members, .. } if members == regions)
        {
            return Err(invalid());
        }
        self.push(Primitive::CaseEliminate, conclusion.clone(), &premises)
    }

    /// An explicit equality retains its complete domain contract and supplied evidence.
    fn equal(
        &mut self,
        left: &Handle,
        right: &Handle,
        domain: &Handle,
        contract: DomainContract,
    ) -> Result<Handle, Fault> {
        let premises: Vec<_> = self
            .request
            .premises
            .iter()
            .map(|proof| proof.proof().clone())
            .collect();
        self.push(
            Primitive::CaseCongruence,
            Statement::Equality {
                left: left.clone(),
                right: right.clone(),
                domain: domain.clone(),
                domain_contract: contract,
            },
            &premises,
        )
    }

    /// Expand merge under the same context and independent request ledgers.
    fn merge(
        &mut self,
        function: &Handle,
        proposed: &Handle,
        regions: &[Handle],
    ) -> Result<Handle, Fault> {
        crate::case_recipes::merge(
            &mut self.plan,
            self.cas,
            &self.request.context,
            function,
            proposed,
            regions,
            self.generation,
            self.checker,
        )
    }

    /// Expand function-indexed ownership with explicit exact domain evidence.
    fn ownership(&mut self, function: &Handle, point: &Handle) -> Result<Handle, Fault> {
        crate::case_recipes::ownership(
            &mut self.plan,
            self.cas,
            &self.request.context,
            function,
            point,
            self.generation,
            self.checker,
        )
    }

    /// Requested function theorems on a proved empty region are refusals, not vacuous successes.
    fn nonempty(&mut self, inputs: &[LogicalId]) -> Result<(), Fault> {
        let domain = match &self.request.task {
            Task::Equal { domain, .. }
            | Task::Coverage { domain }
            | Task::Select { region: domain } => Some(domain.logical_id()),
            Task::Eliminate { conclusion, .. } => {
                crate::context_check::domain(conclusion).map(Handle::logical_id)
            }
            Task::ElaborateCases => Some(
                self.cas
                    .state
                    .source_form(*inputs.first().ok_or_else(invalid)?)?
                    .domain,
            ),
            _ => None,
        };
        if let Some(domain) = domain {
            match crate::case_obligations::empty(self.cas, domain, self.generation) {
                Ok(true) => {
                    return Err(Fault::invalid(
                        InvalidCode::EmptyDomain,
                        Location::new("native request domain"),
                    ));
                }
                Err(fault) if !crate::case_obligations::unknown(&fault) => return Err(fault),
                _ => {}
            }
        }
        Ok(())
    }

    /// Execute a single registered case schema with no implicit premise borrowing.
    fn push(
        &mut self,
        rule: Primitive,
        conclusion: Statement,
        premises: &[Handle],
    ) -> Result<Handle, Fault> {
        self.plan.push(
            self.cas,
            &self.request.context,
            rule,
            conclusion,
            premises,
            Certificate::None,
            self.generation,
            self.checker,
        )
    }

    /// Ordered signatures retain the exact source descriptor and every guarded requirement.
    fn elaborate(&mut self, source: &Handle) -> Result<Handle, Fault> {
        let context = self.cas.checked(&self.request.context)?;
        let mut earlier = Vec::new();
        let mut premises = Vec::new();
        for witness in &self.request.premises {
            earlier.push(crate::proof_view::application(
                self.cas,
                witness.proof().logical_id(),
                self.generation,
            )?);
            premises.push(witness.proof().clone());
        }
        let built = crate::case_ordered::construct(
            self.cas,
            context,
            source.logical_id(),
            &earlier,
            self.generation,
        )?;
        let domain = self
            .cas
            .handle(self.cas.state.source_form(source.logical_id())?.domain);
        self.push(
            Primitive::CaseOrdered,
            Statement::Elaboration {
                source: source.clone(),
                result: built.result,
                domain,
            },
            &premises,
        )
    }

    /// Choose an actual covering association; unrelated global overlaps do not enter this theorem.
    fn select(&mut self, function: &Handle, region: &Handle) -> Result<Handle, Fault> {
        let arms = self
            .cas
            .state
            .expression(function.logical_id())?
            .arms
            .clone();
        for id in case_syntax::canonical(self.cas, &arms, self.generation)? {
            let arm = self.cas.state.arm(id)?.clone();
            match crate::case_check::subset(
                self.cas,
                region.logical_id(),
                arm.region,
                self.generation,
            ) {
                Ok(()) => {
                    let right = self.cas.handle(arm.value);
                    return self.push(
                        Primitive::CaseSelect,
                        Statement::Equality {
                            left: function.clone(),
                            right,
                            domain: region.clone(),
                            domain_contract: DomainContract::DefinedAndEqual,
                        },
                        &[],
                    );
                }
                Err(fault)
                    if matches!(
                        fault.kind,
                        crate::error::FaultKind::Invalid(InvalidCode::Rule)
                    ) => {}
                Err(fault) => return Err(fault),
            }
        }
        Err(pending())
    }

    /// Totality is a definedness fact on the requested region, with the complete cover recorded.
    fn coverage(&mut self, function: &Handle, region: &Handle) -> Result<Handle, Fault> {
        let (domain_proof, defined) = crate::case_producer::domain(
            &mut self.plan,
            self.cas,
            &self.request.context,
            function,
            self.generation,
            self.checker,
        )?;
        let cover = crate::case_recipes::cover(
            &mut self.plan,
            self.cas,
            &self.request.context,
            region,
            &[defined],
            self.generation,
            self.checker,
        )?;
        let predicate = case_syntax::predicate(
            self.cas,
            function.logical_id(),
            crate::model::PredicateKind::Defined,
            self.generation,
        )?;
        self.push(
            Primitive::CaseDomain,
            Statement::PredicateFact {
                region: region.clone(),
                predicate: self.cas.handle(predicate),
            },
            &[domain_proof, cover],
        )
    }

    /// Refinement is scheduled lazily; its complete cover remains an explicit premise.
    fn simplify(&mut self, function: &Handle) -> Result<Handle, Fault> {
        let (output, premises) = crate::case_refine::produce(
            &mut self.plan,
            self.cas,
            self.request,
            function.logical_id(),
            self.generation,
            self.checker,
        )?;
        let domain = self.cas.handle(
            self.cas
                .state
                .context(self.request.context.logical_id())?
                .admissible,
        );
        self.push(
            Primitive::CaseCongruence,
            Statement::Equality {
                left: function.clone(),
                right: self.cas.handle(output),
                domain,
                domain_contract: DomainContract::SameDefinedDomain,
            },
            &premises,
        )
    }
}

/// Undecided cover evidence and other missing premises stay distinct from mathematical refutations.
const fn classify(fault: Fault, task: &Task) -> Fault {
    if crate::case_obligations::unknown(&fault) {
        let code = if matches!(task, Task::Coverage { .. }) {
            InvalidCode::UnknownTotality
        } else {
            InvalidCode::PendingPremise
        };
        Fault::invalid(code, fault.location)
    } else {
        fault
    }
}

/// An unconditional countercell does not refute a request whose extra hypotheses remain undecided.
fn classify_context(
    cas: &Cas,
    request: &Request,
    fault: Fault,
    budget: &mut Budget,
) -> Result<Fault, Fault> {
    let refuted = matches!(
        fault.kind,
        crate::error::FaultKind::Invalid(
            InvalidCode::OverlapConflict | InvalidCode::RefutedDomain | InvalidCode::CoverageHole
        )
    );
    if refuted {
        if request
            .premises
            .iter()
            .any(|proof| !proof.is_discharged() || !proof.hypotheses().is_empty())
        {
            return Ok(pending());
        }
        let mut current = Some(cas.checked(&request.context)?);
        while let Some(id) = current {
            budget.condition(1, Location::new("case context classification"))?;
            let frame = cas.state.context(id)?;
            if !frame.assumptions.is_empty() || !frame.frame.is_empty() {
                return Ok(pending());
            }
            current = frame.parent;
        }
    }
    Ok(classify(fault, &request.task))
}

/// A missing producer or supplied premise never certifies absence.
const fn pending() -> Fault {
    Fault::invalid(InvalidCode::PendingPremise, Location::new("native request"))
}
