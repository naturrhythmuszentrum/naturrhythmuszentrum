// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded boundary dispatch retains every independently replayed partial fact.

use crate::boundary_derivative::DerivativeJob;
use crate::boundary_producer::{Runner, pending};
use crate::budget::{Budget, BudgetKind};
use crate::case_producer::Plan;
use crate::case_syntax::invalid;
use crate::derivative_model::{BoundaryFacts, ResultPayload};
use crate::domain_model::{AmbientDomain, Layer, Side};
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle, LogicalId};
use crate::proof_model::Statement;
use crate::request_model::Request;
use crate::search::SearchPolicy;
use crate::service::Execution;
use crate::task::{BoundaryFact, Task};

/// Choose the finite real case route without widening existing assignment-domain scalar semantics.
pub(crate) fn handles(
    cas: &Cas,
    task: &Task,
    inputs: &[LogicalId],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    match task {
        Task::Boundary { .. } => Ok(true),
        Task::Differentiate {
            domain,
            order,
            layer: Layer::Pointwise,
            ..
        } => {
            if *order > 1
                || matches!(
                    cas.state.region(domain.logical_id())?.ambient,
                    AmbientDomain::Real(_)
                )
            {
                return Ok(true);
            }
            for id in crate::case_syntax::order(cas, inputs, budget)? {
                if cas.state.expression(id)?.domain.is_some() {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// A request publishes only complete syntax batches and independently replayed facts.
pub(crate) fn execute(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    policy: &SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<Execution, Fault> {
    let mut candidate = cas.fork(generation)?;
    let function = candidate.handle(*inputs.first().ok_or_else(invalid)?);
    let mut runner = Runner {
        cas: &mut candidate,
        request,
        plan: Plan::new(policy),
        generation,
        checker,
        facts: Vec::new(),
        deferred: None,
    };
    let (payload, mut fault) = runner.run(&function);
    if !policy.allow_conditional && !payload.discharged() {
        fault = Some(pending());
    }
    let result = payload.facts().last().cloned().map(|witness| {
        let mut trace = vec![function];
        match &witness.judgment().statement {
            Statement::CaseDerivative(value) => trace.push(value.derivative.clone()),
            Statement::DerivativeAt(value) => trace.push(value.value.clone()),
            _ => {}
        }
        let mut result = runner
            .plan
            .finish_retained(witness, trace, runner.checker.usage());
        result.payload = Some(Box::new(payload));
        result
    });
    cas.state = candidate.state;
    Ok(Execution { result, fault })
}

impl Runner<'_> {
    /// Convert failed further work into a partial collection without inventing an obstruction.
    fn run(&mut self, function: &Handle) -> (ResultPayload, Option<Fault>) {
        let result = self.task(function);
        match result {
            Ok((mut payload, fault)) => {
                let facts = std::mem::take(&mut self.facts);
                match &mut payload {
                    ResultPayload::DerivativeMap(value) => value.facts = facts,
                    ResultPayload::Boundary(value) => value.facts = facts,
                    ResultPayload::Facts(values)
                    | ResultPayload::Distribution(values)
                    | ResultPayload::PairingScalar(values) => *values = facts,
                }
                (payload, fault)
            }
            Err(fault) => (
                ResultPayload::Facts(std::mem::take(&mut self.facts)),
                Some(crate::boundary_map::classify(fault)),
            ),
        }
    }

    /// Validate coordinates and orders before any potentially large calculus allocation.
    fn task(&mut self, function: &Handle) -> Result<(ResultPayload, Option<Fault>), Fault> {
        match &self.request.task {
            Task::Differentiate {
                coordinate,
                domain,
                order,
                layer: Layer::Pointwise,
            } => {
                self.order(*order, true)?;
                crate::boundary_roles::region(self.cas, function, domain, coordinate)?;
                if crate::case_obligations::empty(self.cas, domain.logical_id(), self.generation)? {
                    return Err(Fault::invalid(
                        crate::error::InvalidCode::EmptyDomain,
                        Location::new("requested derivative domain"),
                    ));
                }
                let job = DerivativeJob {
                    function: function.clone(),
                    domain: domain.clone(),
                    coordinate: coordinate.clone(),
                };
                let (map, fault) = self.map(&job, *order)?;
                Ok((ResultPayload::DerivativeMap(Box::new(map)), fault))
            }
            Task::Boundary {
                point,
                coordinate,
                requested_facts,
            } => {
                crate::boundary_roles::point(self.cas, function, point, coordinate)?;
                let domain = self.cas.handle(
                    self.cas
                        .state
                        .context(self.request.context.logical_id())?
                        .admissible,
                );
                let job = DerivativeJob {
                    function: function.clone(),
                    domain,
                    coordinate: coordinate.clone(),
                };
                if requested_facts.is_empty() {
                    return Err(invalid());
                }
                for fact in requested_facts {
                    self.boundary_fact(&job, point, *fact)?;
                }
                Ok((
                    ResultPayload::Boundary(Box::new(BoundaryFacts {
                        function: function.clone(),
                        boundary: point.clone(),
                        facts: Vec::new(),
                    })),
                    None,
                ))
            }
            _ => Err(invalid()),
        }
    }

    /// Requested fact classes retain their explicit mathematical orders.
    fn boundary_fact(
        &mut self,
        job: &DerivativeJob,
        point: &Handle,
        fact: BoundaryFact,
    ) -> Result<(), Fault> {
        match fact {
            BoundaryFact::Ownership => {
                self.ownership(&job.function, point)?;
            }
            BoundaryFact::Jets { order } => {
                self.order(order, false)?;
                for side in [Side::Below, Side::Above] {
                    let region =
                        self.incident(&job.function, &job.domain, point, side, &job.coordinate)?;
                    self.jet(&job.function, &region, point, side, order, &job.coordinate)?;
                }
            }
            BoundaryFact::Jump { order } => {
                self.order(order, false)?;
                self.jump(job, point, order)?;
            }
            BoundaryFact::Derivative { order } => {
                self.order(order, true)?;
                let prefix = if order > 1 {
                    let (map, fault) = self.map(job, order - 1)?;
                    if let Some(fault) = fault {
                        return Err(fault);
                    }
                    Some(map.complete.ok_or_else(pending)?)
                } else {
                    None
                };
                let current = match prefix.as_ref().map(|value| &value.judgment().statement) {
                    Some(Statement::CaseDerivative(value)) => value.derivative.clone(),
                    None => job.function.clone(),
                    _ => return Err(invalid()),
                };
                self.point(
                    job,
                    &current,
                    point,
                    order,
                    prefix
                        .as_ref()
                        .map(super::proof_model::VerifiedWitness::proof),
                )?;
            }
        }
        Ok(())
    }

    /// Positive derivative order differs from a valid zeroth trace or jump.
    fn order(&mut self, order: u32, positive: bool) -> Result<(), Fault> {
        if positive && order == 0 {
            return Err(invalid());
        }
        self.generation.specialized(
            BudgetKind::JetOrder,
            u64::from(order),
            false,
            Location::new("requested boundary order"),
        )
    }
}
