// SPDX-License-Identifier: AGPL-3.0-only
//! Exact coefficient-region branching, with explicit local-context closure before cover elimination.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{ApplicationDraft, Certificate, ConstructorLaw, Statement};
use crate::rule::Primitive;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{
    CoefficientFailure, Element, EvaluationScope, ZeroDecision,
};
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::radicals::{
    construction::{Construction, admit},
    route::{Audit, Route},
};
use crate::solving::towers::{dense::Dense, materialize};
use crate::solving::{
    CertificateTotalPredicateSplit, PredicateSplitMode, SolutionSetData, SolveCertificate,
    SolveDraft, SolveReason, SolveSchema, SolveStatement, StatementCoverage,
};

/// A completed regional set and its deferred presentation node.
pub(crate) struct Proposal {
    /// Graph-owned exact solution set.
    pub(crate) set: Handle,
    /// Postorder presentation recipe index, with no theorem authority.
    plan: usize,
}

/// One request owns the complete unsealed branch DAG and one shared finite ledger.
pub(crate) struct Regional<'a> {
    /// Graph transaction candidate.
    pub(crate) cas: &'a mut Cas,
    /// Exact current frame and the global ordered proposal DAG.
    pub(crate) construction: &'a mut Construction,
    /// Original source request, including its sealed assumptions.
    pub(crate) request: &'a crate::request_model::Request,
    /// Original equation and denominator policy.
    pub(crate) problem: &'a Handle,
    /// Original dense coefficients, never overwritten by a specialization.
    pub(crate) original: &'a Dense,
    /// Enumeration and pending-obligation record.
    pub(crate) audit: &'a mut Audit,
    /// One generation ledger for every branch and retry.
    pub(crate) budget: &'a mut Budget,
    /// Previously requested exact guards prevent retrying an unsupported decision forever.
    pub(crate) guards: Vec<Element>,
    /// Computed formal invariants may be reused only within this original parameter request.
    pub(crate) cache: Option<crate::solving::radicals::cache::Cache>,
    /// Presentation is deferred until the full coverage proof has independently replayed.
    pub(crate) presentations: super::presentation::Docket,
}

impl Regional<'_> {
    /// Construct one complete leaf proposal; every mathematical claim is checked only at final sealing.
    fn prepare(&mut self) -> Result<Handle, CoefficientFailure> {
        let (context, region) = (
            self.construction.context.clone(),
            self.construction.region.clone(),
        );
        if self
            .cas
            .state
            .region(region.logical_id())?
            .lies_over
            .is_some()
        {
            crate::solving::family::construction::projection(
                self.cas,
                self.construction,
                self.problem,
                self.budget,
            )?;
        }
        let scope = EvaluationScope {
            context: &context,
            region: &region,
            premises: &self.request.premises,
            localization: &[],
        };
        let engine = Engine {
            state: &self.cas.state,
            domain: &self.original.domain,
        };
        let mut coefficients = vector(self.original.coefficients.len(), self.budget)?;
        for coefficient in &self.original.coefficients {
            coefficients.push(
                if matches!(
                    engine.zero_test(coefficient, &scope, self.budget)?,
                    ZeroDecision::Zero(_)
                ) {
                    engine.constant(false, self.budget)?
                } else {
                    engine.copy(coefficient, self.budget)?
                },
            );
        }
        let polynomial = Dense::new(&engine, coefficients, self.budget)?;
        if polynomial.coefficients.is_empty() {
            return self.free();
        }
        let images = super::coefficient_images(
            self.cas,
            self.construction,
            &polynomial.domain,
            self.budget,
        )?;
        let map = self
            .construction
            .mapping(self.cas, &polynomial.domain, &images, self.budget)?;
        let values = Route {
            cache: self.cache.as_mut(),
            units: Vec::new(),
            choices: crate::solving::radicals::route::Choices::default(),
            cas: self.cas,
            construction: self.construction,
            scope: &scope,
            audit: self.audit,
            budget: self.budget,
        }
        .derive(&polynomial, &map, 0)?;
        self.audit
            .mark("factor-coverage", &region, 0, self.budget)?;
        if self
            .cas
            .state
            .region(region.logical_id())?
            .lies_over
            .is_some()
        {
            return Ok(crate::solving::family::coverage::emit(
                self.cas,
                self.construction,
                self.problem,
                &polynomial,
                &values,
                self.budget,
            )?);
        }
        Ok(crate::solving::radicals::proof::coverage(
            self.cas,
            self.construction,
            self.problem,
            &polynomial,
            &values,
            self.budget,
        )?)
    }

    /// The full-coordinate identity family is offered only when no original exclusion is widened.
    fn free(&mut self) -> Result<Handle, CoefficientFailure> {
        let original = crate::solving::elementary::proof::problem(self.cas, self.problem)?;
        if !original.denominators.is_empty()
            || !matches!(
                self.cas
                    .state
                    .region(original.domain.logical_id())?
                    .expression,
                RegionExpr::Ambient
            )
        {
            return Err(CoefficientFailure::Unsupported(
                SolveReason::CoveragePending,
            ));
        }
        Ok(crate::solving::radicals::family::coverage(
            self.cas,
            self.construction,
            self.problem,
            self.budget,
        )?)
    }

    /// A missing regional decision may be split once; the same undecided guard on a child stays unresolved.
    pub(crate) fn solve(&mut self) -> Result<Proposal, CoefficientFailure> {
        self.budget
            .solving(BudgetKind::SolutionBranches, 1, super::AT)?;
        let checkpoint = self.construction.steps.len();
        self.audit.pending = None;
        self.audit.mark(
            "branch-prepare",
            &self.construction.region,
            u64::try_from(self.guards.len()).map_err(|_| invalid())?,
            self.budget,
        )?;
        let failure = match self.prepare() {
            Ok(set) => return self.record(set, Vec::new()),
            Err(error) => error,
        };
        let Some(pending) = self.audit.pending.take() else {
            return Err(failure);
        };
        if pending.domain != self.original.domain || !self.request.premises.is_empty() {
            return Err(failure);
        }
        let engine = Engine {
            state: &self.cas.state,
            domain: &pending.domain,
        };
        for previous in &self.guards {
            if engine.formal_equal(&pending, previous, self.budget)? {
                return Err(failure);
            }
        }
        self.construction.steps.truncate(checkpoint);
        let expression =
            super::guard::expression(self.cas, self.construction, &pending, self.budget)?;
        reserve(&mut self.guards, 1, self.budget)?;
        self.guards.push(pending);
        self.audit.mark(
            "branch-split",
            &self.construction.region,
            u64::try_from(self.guards.len()).map_err(|_| invalid())?,
            self.budget,
        )?;
        let result = self.split(&expression);
        self.guards.pop();
        result
    }

    /// Both exact zero and its defined complement remain explicit children of the same parent.
    fn children(&mut self, expression: &Handle) -> Result<Vec<Handle>, CoefficientFailure> {
        let zero = self.construction.expression(
            self.cas,
            Operator::Rational(crate::number::Rational::zero()),
            &[],
            false,
            self.budget,
        )?;
        let predicate = admit(
            self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.construction.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(expression.clone()),
                    Reference::Existing(zero),
                ],
            },
            self.budget,
        )?;
        let coefficients = crate::solving::elementary::proof::problem(self.cas, self.problem)?
            .coefficients
            .clone();
        let mut children = vector(2, self.budget)?;
        for truth in [true, false] {
            let (base, predicate) = (
                Reference::Existing(self.construction.region.clone()),
                Reference::Existing(predicate.clone()),
            );
            let expression = if truth {
                RegionExpr::GuardTrue { base, predicate }
            } else {
                RegionExpr::GuardFalse { base, predicate }
            };
            children.push(admit(
                self.cas,
                &Draft::Region(RegionDraft {
                    header: Reference::Existing(self.construction.header.clone()),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Product {
                        coordinates: coefficients.clone(),
                    },
                    expression,
                }),
                self.budget,
            )?);
        }
        Ok(children)
    }

    /// Parent coverage names exactly the union of both closed child solution sets.
    fn split(&mut self, expression: &Handle) -> Result<Proposal, CoefficientFailure> {
        if self
            .cas
            .state
            .region(self.construction.region.logical_id())?
            .lies_over
            .is_some()
        {
            crate::solving::family::construction::projection(
                self.cas,
                self.construction,
                self.problem,
                self.budget,
            )?;
        }
        let children = self.children(expression)?;
        let certificate_children =
            crate::solving::coefficients::storage::copied(&children, self.budget)?;
        let cover = self.construction.append(
            Primitive::Solve(SolveSchema::TotalPredicateSplit),
            Statement::Cover {
                domain: self.construction.region.clone(),
                members: certificate_children,
            },
            Certificate::Solve(Box::new(SolveCertificate::TotalPredicateSplit(
                CertificateTotalPredicateSplit {
                    mode: PredicateSplitMode::ZeroNonzero,
                    expression: expression.clone(),
                    region: self.construction.region.clone(),
                    children: crate::solving::coefficients::storage::copied(
                        &children,
                        self.budget,
                    )?,
                    branches: Vec::new(),
                    cover: None,
                },
            ))),
            self.budget,
        )?;
        let mut branches = vector(0, self.budget)?;
        let mut proofs = vector(children.len(), self.budget)?;
        let mut plans = vector(children.len(), self.budget)?;
        for child in &children {
            let (proposal, proof) = self.closed(child)?;
            plans.push(proposal.plan);
            let SolveDraft::SolutionSet(set) =
                crate::solving::graph::object(&self.cas.state, proposal.set.logical_id())?
            else {
                return Err(invalid().into());
            };
            reserve(&mut branches, set.branches.len(), self.budget)?;
            branches.extend(set.branches.iter().cloned().map(Reference::Existing));
            proofs.push(proof);
        }
        let set = materialize::admit(
            self.cas,
            SolveDraft::SolutionSet(SolutionSetData {
                problem: Reference::Existing(self.problem.clone()),
                branches,
            }),
            self.budget,
        )?;
        self.construction.push(
            SolveSchema::TotalPredicateSplit,
            SolveStatement::Coverage(StatementCoverage {
                problem: self.problem.clone(),
                set: set.clone(),
                region: self.construction.region.clone(),
            }),
            SolveCertificate::TotalPredicateSplit(CertificateTotalPredicateSplit {
                mode: PredicateSplitMode::EliminateCover,
                expression: expression.clone(),
                region: self.construction.region.clone(),
                children,
                branches: proofs,
                cover: Some(cover),
            }),
            self.budget,
        )?;
        self.record(set, plans)
    }

    /// Record representation work only after a complete regional proposal exists.
    fn record(
        &mut self,
        set: Handle,
        children: Vec<usize>,
    ) -> Result<Proposal, CoefficientFailure> {
        let plan = self
            .presentations
            .add(self.construction, &set, children, self.budget)?;
        Ok(Proposal { set, plan })
    }

    /// Close precisely the local membership frame, retaining its exact meet region and no sibling assumptions.
    fn closed(&mut self, child: &Handle) -> Result<(Proposal, u32), CoefficientFailure> {
        let (parent, domain) = (
            self.construction.context.clone(),
            self.construction.region.clone(),
        );
        let frame = admit(
            self.cas,
            &Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(parent.clone()),
                region: Reference::Existing(child.clone()),
                hypotheses: Vec::new(),
            }),
            self.budget,
        )?;
        let region = self
            .cas
            .handle(self.cas.state.context(frame.logical_id())?.admissible);
        self.construction.context.clone_from(&frame);
        self.construction.region.clone_from(&region);
        let result = self.solve();
        self.construction.context.clone_from(&parent);
        self.construction.region.clone_from(&domain);
        let proposal = result?;
        let covered = u32::try_from(
            self.construction
                .steps
                .len()
                .checked_sub(1)
                .ok_or_else(invalid)?,
        )
        .map_err(|_| invalid())?;
        let conclusion = self
            .construction
            .steps
            .last()
            .ok_or_else(invalid)?
            .conclusion
            .clone();
        let constructor = u32::try_from(self.construction.steps.len()).map_err(|_| invalid())?;
        reserve(&mut self.construction.steps, 2, self.budget)?;
        self.construction.steps.push(ApplicationDraft {
            rule: Primitive::RegionConstruct,
            context: self.construction.context.clone(),
            conclusion: Statement::Constructor {
                region,
                law: ConstructorLaw::Membership,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        });
        self.presentations.close(
            proposal.plan,
            super::presentation::Close {
                parent,
                region: domain,
                frame: frame.clone(),
                constructor,
            },
        )?;
        let slot = self
            .construction
            .steps
            .iter()
            .filter(|step| step.context == self.construction.context)
            .count();
        self.budget.work(
            crate::graph::size(self.construction.steps.len(), super::AT)?,
            super::AT,
        )?;
        self.construction.steps.push(ApplicationDraft {
            rule: Primitive::ContextClose,
            context: self.construction.context.clone(),
            conclusion,
            premises: vec![covered, constructor],
            substitutions: Vec::new(),
            certificate: Certificate::Close { frame },
        });
        Ok((proposal, u32::try_from(slot).map_err(|_| invalid())?))
    }
}
