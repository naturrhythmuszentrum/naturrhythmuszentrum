// SPDX-License-Identifier: AGPL-3.0-only
//! The cover recipe emits ordinary checked applications with exact ordered inventories.

use super::{FamilyCoverRequest, coverage};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, Reference};
use crate::proof_model::{Certificate, Statement, VerifiedWitness};
use crate::solving::family::{invalid, region};
use crate::solving::{
    CertificateTotalPredicateSplit, PredicateSplitMode, SolutionSetData, SolveCertificate,
    SolveDraft, SolveSchema, SolveStatement, StatementCoverage,
};

/// Match every original base member to exactly one total-space local group.
pub(super) fn inventory(
    cas: &Cas,
    request: &FamilyCoverRequest,
    witness: &VerifiedWitness,
    budget: &mut Budget,
) -> Result<Option<Vec<usize>>, Fault> {
    cas.checked(witness.proof())?;
    if !witness.is_discharged() {
        return Err(invalid());
    }
    let Statement::Cover { domain, members } = &witness.judgment().statement else {
        return Err(invalid());
    };
    let parent = cas.state.context(cas.checked(&request.context)?)?;
    let total = cas.state.region(parent.admissible)?;
    let over = total.lies_over.as_ref().ok_or_else(invalid)?;
    let family =
        crate::solving::family::mapping::family_data(&cas.state, &cas.handle(over.family))?;
    let base = crate::solving::family::mapping::base_data(&cas.state, &family.base)?;
    if *domain != base.domain || over.stratum != base.domain.logical_id() {
        return Err(invalid());
    }
    if members.len() != request.groups.len() {
        return Ok(None);
    }
    let mut order = crate::solving::coefficients::storage::vector(members.len(), budget)?;
    for member in members {
        let mut found = None;
        for (index, group) in request.groups.iter().enumerate() {
            budget.work(1, crate::solving::family::AT)?;
            let frame = cas.state.context(cas.checked(&group.context)?)?;
            let shape = cas.state.region(frame.admissible)?;
            if shape.lies_over.as_ref().is_some_and(|value| {
                value.family == over.family && value.stratum == member.logical_id()
            }) {
                if found.is_some() || order.contains(&index) {
                    return Err(invalid());
                }
                found = Some(index);
            }
        }
        let Some(found) = found else {
            return Ok(None);
        };
        order.push(found);
    }
    Ok(Some(order))
}

/// One recipe owns its original context, plan and cumulative independent ledgers.
struct Runner<'a, 'p> {
    /// Transactional graph for expanded objects.
    cas: &'a mut Cas,
    /// Existing primitive application plan.
    plan: crate::case_producer::Plan<'p>,
    /// One cumulative construction ledger.
    generation: &'a mut Budget,
    /// Independent cumulative replay ledger.
    checker: &'a mut Budget,
}

impl Runner<'_, '_> {
    /// Close only real ancestor frames, retaining the exact piece theorem region.
    fn close(&mut self, witness: &VerifiedWitness, target: &Handle) -> Result<Handle, Fault> {
        let mut proof = witness.proof().clone();
        let mut context = witness.judgment().context.clone();
        while context != *target {
            let frame = self.cas.state.context(context.logical_id())?;
            let parent = self.cas.handle(frame.parent.ok_or_else(invalid)?);
            let region = self.cas.handle(frame.admissible);
            let constructor = self.plan.push(
                self.cas,
                &parent,
                crate::rule::Primitive::RegionConstruct,
                Statement::Constructor {
                    region,
                    law: crate::proof_model::ConstructorLaw::Membership,
                },
                &[],
                Certificate::None,
                self.generation,
                self.checker,
            )?;
            proof = self.plan.push(
                self.cas,
                &parent,
                crate::rule::Primitive::ContextClose,
                witness.judgment().statement.clone(),
                &[proof, constructor],
                Certificate::Close { frame: context },
                self.generation,
                self.checker,
            )?;
            context = parent;
        }
        Ok(proof)
    }

    /// Every combined set is exactly the union of the supplied checked branch inventories.
    fn set(&mut self, problem: &Handle, proofs: &[Handle]) -> Result<Handle, Fault> {
        let mut branches = Vec::new();
        for proof in proofs {
            let application =
                crate::proof_view::application(self.cas, proof.logical_id(), self.generation)?;
            let Statement::Solve(statement) = &application.judgment.statement else {
                return Err(invalid());
            };
            let SolveStatement::Coverage(claim) = statement.as_ref() else {
                return Err(invalid());
            };
            if claim.problem != *problem {
                return Err(invalid());
            }
            let SolveDraft::SolutionSet(set) =
                crate::solving::graph::object(&self.cas.state, claim.set.logical_id())?
            else {
                return Err(invalid());
            };
            for branch in &set.branches {
                let reference = Reference::Existing(branch.clone());
                if !branches.contains(&reference) {
                    branches.push(reference);
                }
            }
        }
        crate::solving::towers::materialize::admit(
            self.cas,
            SolveDraft::SolutionSet(SolutionSetData {
                problem: Reference::Existing(problem.clone()),
                branches,
            }),
            self.generation,
        )
    }

    /// Existing total-cover elimination checks each closed piece against its exact covering member.
    fn combine(
        &mut self,
        context: &Handle,
        problem: &Handle,
        cover: &Handle,
        proofs: &[Handle],
    ) -> Result<Handle, Fault> {
        let application =
            crate::proof_view::application(self.cas, cover.logical_id(), self.generation)?;
        let Statement::Cover { domain, members } = application.judgment.statement else {
            return Err(invalid());
        };
        if members.len() != proofs.len() {
            return Err(invalid());
        }
        let ordered = self.order(&members, proofs)?;
        let set = self.set(problem, &ordered)?;
        let header = self
            .cas
            .handle(self.cas.state.region(domain.logical_id())?.header);
        let expression = crate::solving::family::build::expression(
            self.cas,
            &header,
            &Vec::new(),
            crate::model::Operator::Rational(crate::number::Rational::zero()),
            crate::model::Field::Complex,
            &[],
            self.generation,
        )?;
        let mut premises = vec![cover.clone()];
        premises.extend(ordered);
        self.plan.push(
            self.cas,
            context,
            crate::rule::Primitive::Solve(SolveSchema::TotalPredicateSplit),
            Statement::Solve(Box::new(SolveStatement::Coverage(StatementCoverage {
                problem: problem.clone(),
                set,
                region: domain.clone(),
            }))),
            &premises,
            Certificate::Solve(Box::new(SolveCertificate::TotalPredicateSplit(
                CertificateTotalPredicateSplit {
                    mode: PredicateSplitMode::EliminateCover,
                    expression,
                    region: domain,
                    children: members,
                    branches: (1..premises.len())
                        .map(|index| u32::try_from(index).map_err(|_| invalid()))
                        .collect::<Result<_, _>>()?,
                    cover: Some(0),
                },
            ))),
            self.generation,
            self.checker,
        )
    }

    /// Match closed coverage to the cover's original child inventory without sampling.
    fn order(&mut self, members: &[Handle], proofs: &[Handle]) -> Result<Vec<Handle>, Fault> {
        let mut result = Vec::new();
        for member in members {
            let mut found = None;
            for proof in proofs {
                let application =
                    crate::proof_view::application(self.cas, proof.logical_id(), self.generation)?;
                let Statement::Solve(statement) = application.judgment.statement else {
                    return Err(invalid());
                };
                let SolveStatement::Coverage(claim) = statement.as_ref() else {
                    return Err(invalid());
                };
                if region::subset(
                    self.cas,
                    member.logical_id(),
                    claim.region.logical_id(),
                    self.generation,
                )? && region::subset(
                    self.cas,
                    claim.region.logical_id(),
                    member.logical_id(),
                    self.generation,
                )? {
                    if found.is_some() || result.contains(proof) {
                        return Err(invalid());
                    }
                    found = Some(proof.clone());
                }
            }
            result.push(found.ok_or_else(invalid)?);
        }
        Ok(result)
    }

    /// Base cover transport explicitly names every exact lifted member.
    fn lift(
        &mut self,
        request: &FamilyCoverRequest,
        base: &VerifiedWitness,
        members: Vec<Handle>,
    ) -> Result<Handle, Fault> {
        let total = self.cas.handle(
            self.cas
                .state
                .context(request.context.logical_id())?
                .admissible,
        );
        let lifted = crate::solving::family::recipes::pullback(self.cas, &total, self.generation)?;
        let subset = self.plan.push(
            self.cas,
            &request.context,
            crate::rule::Primitive::RegionConstruct,
            Statement::Subset {
                subset: total.clone(),
                superset: lifted,
            },
            &[],
            Certificate::None,
            self.generation,
            self.checker,
        )?;
        self.plan.push(
            self.cas,
            &request.context,
            crate::rule::Primitive::ContextWeaken,
            Statement::Cover {
                domain: total,
                members,
            },
            &[base.proof().clone(), subset],
            Certificate::None,
            self.generation,
            self.checker,
        )
    }
}

/// Emit fibre elimination, close each group, then eliminate the exact lifted base cover.
pub(super) fn run(
    cas: &mut Cas,
    request: &FamilyCoverRequest,
    base: &VerifiedWitness,
    order: &[usize],
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<VerifiedWitness, Fault> {
    let policy = crate::search::SearchPolicy::default();
    let mut runner = Runner {
        cas,
        plan: crate::case_producer::Plan::new(&policy),
        generation,
        checker,
    };
    let mut groups = Vec::new();
    let mut members = Vec::new();
    for index in order {
        let group = request.groups.get(*index).ok_or_else(invalid)?;
        let frame = runner.cas.state.context(group.context.logical_id())?;
        if frame.parent != Some(request.context.logical_id()) {
            return Err(invalid());
        }
        members.push(runner.cas.handle(frame.restriction.ok_or_else(invalid)?));
        let cover = group.cover.as_ref().ok_or_else(invalid)?;
        if cover.judgment().context != group.context || !cover.is_discharged() {
            return Err(invalid());
        }
        let mut pieces = Vec::new();
        for piece in &group.pieces {
            coverage(piece)?;
            pieces.push(runner.close(piece, &group.context)?);
        }
        let proof = runner.combine(&group.context, &request.problem, cover.proof(), &pieces)?;
        let witness = runner.cas.seal_scoped(&proof, runner.checker)?;
        groups.push(runner.close(&witness, &request.context)?);
    }
    let base = runner.lift(request, base, members)?;
    let proof = runner.combine(&request.context, &request.problem, &base, &groups)?;
    Ok(runner
        .plan
        .finish(runner.cas, &proof, Vec::new(), runner.checker)?
        .witness)
}
