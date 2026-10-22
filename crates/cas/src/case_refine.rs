// SPDX-License-Identifier: AGPL-3.0-only
//! Lazy common refinement with resumable pair cursors on both existing search queues.
//!
//! # Bounded refinement and calculus
//!
//! Native `Simplify` schedules common-refinement pairs lazily on the cost and progress queues. Empty pairs consume the shared refinement allowance, nested jobs share the same ledger, positive growth is charged, and each complete result retains a checked cover. Boundary calculus adds finite selector partitions, polynomial side jets, classical cell and point derivatives, witnessed nonexistence, and transparent named definitions. Distribution calculus adds checked regular embeddings, finite jump derivatives, delta actions and polynomial smooth multipliers in an exact declared space.

use crate::budget::{Budget, BudgetKind};
use crate::case_producer::Plan;
use crate::case_syntax::{self, invalid};
use crate::domain_model::CasesDraft;
use crate::error::{Fault, Location};
use crate::frontier::{Frontier, Metrics};
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, LogicalId, Reference, size};
use crate::model::Operator;
use crate::request_model::Request;
use std::collections::{BTreeMap, BTreeSet};

/// Refinement accounting location.
const AT: Location = Location::new("lazy common refinement");

/// One explicit association after applying the case's declared domain.
#[derive(Clone, Copy)]
struct Arm {
    /// Exact applicability region.
    region: LogicalId,
    /// Strict associated scalar value.
    value: LogicalId,
}

/// A suspended Cartesian cursor; no product-sized array is allocated.
struct Job {
    /// Original binary operation represented by this job.
    expression: LogicalId,
    /// Declared common region that the completed members must cover.
    domain: LogicalId,
    /// Canonical left association family.
    left: Vec<Arm>,
    /// Canonical right association family.
    right: Vec<Arm>,
    /// Next pair in row-major structural order.
    next: usize,
    /// Number of pairs, checked before multiplication.
    count: usize,
    /// Completed nonempty result associations.
    arms: Vec<Handle>,
    /// Their exact regions, retained for the explicit cover proof.
    members: Vec<Handle>,
    /// Suspended parent and the region receiving this nested result.
    parent: Option<(usize, LogicalId)>,
}

/// The whole request, including nested jobs, shares these exact resources.
struct Scheduler<'a, 'p> {
    /// Unpublished candidate graph.
    cas: &'a mut Cas,
    /// Primitive expansion accumulator.
    plan: &'a mut Plan<'p>,
    /// Exact parent request and coverage support.
    request: &'a Request,
    /// Both cost and progress scheduling views.
    frontier: Frontier,
    /// Bounded producer state indexed by queue labels.
    jobs: Vec<Job>,
    /// Every complete cover needed by the final congruence.
    covers: Vec<Handle>,
    /// Completed equivalent jobs may be reused only in the same exact local region.
    completed: BTreeMap<(LogicalId, LogicalId), LogicalId>,
    /// Request-wide pair and allocation ledger.
    generation: &'a mut Budget,
    /// Independent theorem checker ledger.
    checker: &'a mut Budget,
}

/// Refine strict binary operations after processing their nested value dependencies.
pub(crate) fn produce(
    plan: &mut Plan<'_>,
    cas: &mut Cas,
    request: &Request,
    root: LogicalId,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<(LogicalId, Vec<Handle>), Fault> {
    let order = case_syntax::order(cas, &[root], generation)?;
    let mut scheduler = Scheduler {
        cas,
        plan,
        request,
        frontier: Frontier::default(),
        jobs: Vec::new(),
        covers: Vec::new(),
        completed: BTreeMap::new(),
        generation,
        checker,
    };
    let mut mapped = BTreeMap::new();
    for id in order {
        let rebuilt = scheduler.rebuild(id, &mapped)?;
        let output = if eligible(scheduler.cas, rebuilt)? {
            scheduler.run(rebuilt)?
        } else {
            rebuilt
        };
        mapped.insert(id, output);
    }
    Ok((*mapped.get(&root).ok_or_else(invalid)?, scheduler.covers))
}

impl Scheduler<'_, '_> {
    /// Preserve native syntax and apply only already completed child transformations.
    fn rebuild(
        &mut self,
        id: LogicalId,
        mapped: &BTreeMap<LogicalId, LogicalId>,
    ) -> Result<LogicalId, Fault> {
        let value = self.cas.state.expression(id)?.clone();
        if let Some(domain) = value.domain {
            let mut arms = Vec::new();
            for arm in &value.arms {
                let arm = self.cas.state.arm(*arm)?.clone();
                let child = *mapped.get(&arm.value).ok_or_else(invalid)?;
                arms.push(Reference::Existing(crate::case_ordered::arm(
                    self.cas,
                    value.header,
                    &value.scope,
                    arm.region,
                    child,
                    self.generation,
                )?));
            }
            let draft = Draft::Cases(CasesDraft {
                header: Reference::Existing(self.cas.handle(value.header)),
                scope: value.scope,
                domain: Reference::Existing(self.cas.handle(domain)),
                arms,
            });
            return Ok(case_syntax::admit(self.cas, draft, self.generation)?.logical_id());
        }
        if value.operands.is_empty() {
            return Ok(id);
        }
        let operands = value
            .operands
            .iter()
            .map(|child| {
                mapped
                    .get(child)
                    .map(|id| Reference::Existing(self.cas.handle(*id)))
                    .ok_or_else(invalid)
            })
            .collect::<Result<_, _>>()?;
        let draft = Draft::Expression(ExpressionDraft {
            space: None,
            header: Reference::Existing(self.cas.handle(value.header)),
            scope: value.scope,
            sort: value.sort,
            operator: value.operator,
            binding: value.binding,
            operands,
        });
        Ok(case_syntax::admit(self.cas, draft, self.generation)?.logical_id())
    }

    /// Resume one pair per scheduler visit, with nested cursors joining the same queues.
    fn run(&mut self, expression: LogicalId) -> Result<LogicalId, Fault> {
        let within = self
            .cas
            .state
            .context(self.request.context.logical_id())?
            .admissible;
        if let Some(output) = self.completed.get(&(expression, within)) {
            return Ok(*output);
        }
        let root = self.start(expression, None)?;
        while let Some(id) = self.frontier.pop(self.generation)? {
            self.plan.visit()?;
            let complete = self.jobs.get(id).ok_or_else(invalid)?.next
                == self.jobs.get(id).ok_or_else(invalid)?.count;
            if complete {
                let output = self.finish(id)?;
                if id == root {
                    return Ok(output);
                }
                let (parent, region) = self
                    .jobs
                    .get(id)
                    .ok_or_else(invalid)?
                    .parent
                    .ok_or_else(invalid)?;
                self.retain(parent, region, output)?;
                self.frontier.resume(parent, self.generation)?;
            } else {
                self.advance(id)?;
            }
        }
        Err(invalid())
    }

    /// Construct only the two input families and one suspended cursor.
    fn start(
        &mut self,
        expression: LogicalId,
        parent: Option<(usize, LogicalId)>,
    ) -> Result<usize, Fault> {
        let within = parent.map_or(
            self.cas
                .state
                .context(self.request.context.logical_id())?
                .admissible,
            |(_, region)| region,
        );
        let value = self.cas.state.expression(expression)?.clone();
        let left = family(
            self.cas,
            *value.operands.first().ok_or_else(invalid)?,
            within,
            self.generation,
        )?;
        let right = family(
            self.cas,
            *value.operands.get(1).ok_or_else(invalid)?,
            within,
            self.generation,
        )?;
        let count = left.len().checked_mul(right.len()).ok_or_else(invalid)?;
        let a = case_syntax::union(
            self.cas,
            within,
            &left.iter().map(|arm| arm.region).collect::<Vec<_>>(),
            self.generation,
        )?;
        let b = case_syntax::union(
            self.cas,
            within,
            &right.iter().map(|arm| arm.region).collect::<Vec<_>>(),
            self.generation,
        )?;
        let domain = case_syntax::meet(self.cas, within, &[within, a, b], self.generation)?;
        let id = self.jobs.len();
        self.generation.check(
            BudgetKind::FrontierLabels,
            size(id.saturating_add(1), AT)?,
            AT,
        )?;
        self.jobs
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.jobs.push(Job {
            expression,
            domain,
            left,
            right,
            next: 0,
            count,
            arms: Vec::new(),
            members: Vec::new(),
            parent,
        });
        self.enqueue(id)?;
        Ok(id)
    }

    /// Full context, task, syntax and receipt support prevent cross-obligation dominance.
    fn enqueue(&mut self, id: usize) -> Result<(), Fault> {
        let job = self.jobs.get(id).ok_or_else(invalid)?;
        let mut key = crate::wire_build::syntax_key(
            self.cas,
            self.request.context.logical_id(),
            self.generation,
        )?;
        key.extend(crate::wire_build::syntax_key(
            self.cas,
            job.expression,
            self.generation,
        )?);
        key.extend(crate::wire_build::syntax_key(
            self.cas,
            job.domain,
            self.generation,
        )?);
        let mut encoded = crate::encoding::Encoder::new(self.generation, AT);
        crate::task::encode(&self.request.task, &mut encoded)?;
        key.extend(encoded.finish());
        let mut support: BTreeSet<_> = self
            .request
            .inputs
            .iter()
            .map(|root| root.root.logical_id())
            .collect();
        support.extend(
            self.request
                .premises
                .iter()
                .map(|proof| proof.proof().logical_id()),
        );
        let value = self.cas.state.expression(job.expression)?;
        let metrics = Metrics {
            cost: self.plan.refinement_cost()?,
            peak: value.unfolded,
            credits: self.plan.credits(),
            depth: value.depth,
            bytes: size(key.len(), AT)?,
            support,
            encoding: key.clone(),
        };
        if !self.frontier.insert(
            id,
            key,
            [size(job.count, AT)?, value.unfolded, value.depth, 0],
            metrics,
            self.generation,
        )? {
            return Err(invalid());
        }
        Ok(())
    }

    /// Charge every attempted pair before even asking whether its exact intersection is empty.
    fn advance(&mut self, id: usize) -> Result<(), Fault> {
        self.generation
            .specialized(BudgetKind::RefinementPairs, 1, true, AT)?;
        self.plan.attempt(self.generation)?;
        let job = self.jobs.get_mut(id).ok_or_else(invalid)?;
        let width = job.right.len();
        let left = *job
            .left
            .get(job.next.checked_div(width).ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let right = *job
            .right
            .get(job.next.checked_rem(width).ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        job.next = job.next.checked_add(1).ok_or_else(invalid)?;
        let domain = job.domain;
        let expression = job.expression;
        let region = case_syntax::meet(
            self.cas,
            domain,
            &[domain, left.region, right.region],
            self.generation,
        )?;
        if crate::case_obligations::empty(self.cas, region, self.generation)? {
            return self.frontier.resume(id, self.generation);
        }
        let value = binary(
            self.cas,
            expression,
            left.value,
            right.value,
            self.generation,
        )?;
        if eligible(self.cas, value)? {
            if let Some(output) = self.completed.get(&(value, region)).copied() {
                self.retain(id, region, output)?;
                self.frontier.resume(id, self.generation)?;
            } else {
                self.start(value, Some((id, region)))?;
            }
        } else {
            self.retain(id, region, value)?;
            self.frontier.resume(id, self.generation)?;
        }
        Ok(())
    }

    /// Append a completed nonempty pair without hiding any of its definedness requirements.
    fn retain(&mut self, id: usize, region: LogicalId, value: LogicalId) -> Result<(), Fault> {
        let job = self.jobs.get(id).ok_or_else(invalid)?;
        let syntax = self.cas.state.expression(job.expression)?.clone();
        let arm = crate::case_ordered::arm(
            self.cas,
            syntax.header,
            &syntax.scope,
            region,
            value,
            self.generation,
        )?;
        let job = self.jobs.get_mut(id).ok_or_else(invalid)?;
        crate::case_producer::family(job.arms.len().saturating_add(1), self.generation)?;
        job.arms.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        job.members
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        job.arms.push(arm);
        job.members.push(self.cas.handle(region));
        Ok(())
    }

    /// A completed product requires a checked cover before the native result is retained.
    fn finish(&mut self, id: usize) -> Result<LogicalId, Fault> {
        let job = self.jobs.get(id).ok_or_else(invalid)?;
        let domain = self.cas.handle(job.domain);
        let member_ids: Vec<_> = job.members.iter().map(Handle::logical_id).collect();
        let members: Vec<_> = case_syntax::canonical(self.cas, &member_ids, self.generation)?
            .into_iter()
            .map(|id| self.cas.handle(id))
            .collect();
        let cover = crate::case_recipes::cover(
            self.plan,
            self.cas,
            &self.request.context,
            &domain,
            &members,
            self.generation,
            self.checker,
        )?;
        self.covers
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.covers.push(cover);
        if job.arms.is_empty() {
            let within = job.parent.map_or(
                self.cas
                    .state
                    .context(self.request.context.logical_id())?
                    .admissible,
                |(_, region)| region,
            );
            self.completed
                .insert((job.expression, within), job.expression);
            return Ok(job.expression);
        }
        let value = self.cas.state.expression(job.expression)?;
        let draft = Draft::Cases(CasesDraft {
            header: Reference::Existing(self.cas.handle(value.header)),
            scope: value.scope.clone(),
            domain: Reference::Existing(domain),
            arms: job.arms.iter().cloned().map(Reference::Existing).collect(),
        });
        let expression = job.expression;
        let within = job.parent.map_or(
            self.cas
                .state
                .context(self.request.context.logical_id())?
                .admissible,
            |(_, region)| region,
        );
        let output = case_syntax::admit(self.cas, draft, self.generation)?.logical_id();
        self.plan.expansion(
            self.cas.state.expression(expression)?.unfolded,
            self.cas.state.expression(output)?.unfolded,
            self.generation,
        )?;
        self.completed.insert((expression, within), output);
        Ok(output)
    }
}

/// Only strict binary arithmetic has a common-refinement producer in this lane.
fn eligible(cas: &Cas, id: LogicalId) -> Result<bool, Fault> {
    let value = cas.state.expression(id)?;
    if !matches!(
        value.operator,
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div
    ) {
        return Ok(false);
    }
    for operand in &value.operands {
        if cas.state.expression(*operand)?.domain.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A scalar participates as one association; native cases use canonical structural arm order.
fn family(
    cas: &mut Cas,
    id: LogicalId,
    within: LogicalId,
    budget: &mut Budget,
) -> Result<Vec<Arm>, Fault> {
    let value = cas.state.expression(id)?.clone();
    let Some(domain) = value.domain else {
        return Ok(vec![Arm {
            region: within,
            value: id,
        }]);
    };
    let mut result = Vec::new();
    for id in case_syntax::canonical(cas, &value.arms, budget)? {
        let arm = cas.state.arm(id)?.clone();
        let region = case_syntax::meet(cas, within, &[within, domain, arm.region], budget)?;
        result.push(Arm {
            region,
            value: arm.value,
        });
    }
    Ok(result)
}

/// Retain the original strict binary operator and exact semantic signature.
fn binary(
    cas: &mut Cas,
    id: LogicalId,
    left: LogicalId,
    right: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let value = cas.state.expression(id)?;
    let draft = Draft::Expression(ExpressionDraft {
        space: None,
        header: Reference::Existing(cas.handle(value.header)),
        scope: value.scope.clone(),
        sort: value.sort.clone(),
        operator: value.operator.clone(),
        binding: value.binding.clone(),
        operands: vec![
            Reference::Existing(cas.handle(left)),
            Reference::Existing(cas.handle(right)),
        ],
    });
    Ok(case_syntax::admit(cas, draft, budget)?.logical_id())
}
