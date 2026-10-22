// SPDX-License-Identifier: AGPL-3.0-only
//! Complete Real coefficient strata, using only unsealed proposals and the shared final replay.

use super::{
    construction::{Frame, admit},
    leaf::{Guard, Leaf},
};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{ApplicationDraft, Certificate, ConstructorLaw, Statement};
use crate::rule::Primitive;
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::{CoefficientFailure, Element, EvaluationScope, ZeroDecision},
    storage::{invalid, reserve, vector},
};
use crate::solving::radicals::construction::Construction;
use crate::solving::solve_search::presentation::{Close, Docket};
use crate::solving::towers::{dense::Dense, materialize};
use crate::solving::{
    CertificateTotalPredicateSplit, PredicateSplitMode, RealSignConclusion as Sign,
    SolutionSetData, SolveCertificate, SolveDraft, SolveReason, SolveSchema, SolveStatement,
    StatementCoverage,
};

/// A completed local set and its deferred presentation recipe.
struct Proposal {
    /// Complete proposed regional solution set.
    set: Handle,
    /// Postorder presentation recipe.
    plan: usize,
}

/// The original source and one shared request ledger govern every child context.
struct Search<'a> {
    /// Transaction candidate.
    cas: &'a mut Cas,
    /// Original Real semantic frame and shared proposal DAG.
    frame: Frame,
    /// Original problem with retained source and exclusions.
    problem: &'a Handle,
    /// Complete original coefficient vector before any degree specialization.
    original: Dense,
    /// Actual ancestor guard alternatives, in traversal order.
    guards: Vec<Guard>,
    /// Deferred representation proofs for the complete branch tree.
    presentations: Docket,
    /// Cost observations without mathematical authority.
    audit: &'a mut crate::solving::radicals::route::Audit,
    /// One generation ledger across every branch.
    budget: &'a mut Budget,
}

/// The caller independently checks the returned DAG before exposing any coverage or presentation.
pub(crate) fn propose(
    cas: &mut Cas,
    request: &crate::request_model::Request,
    audit: &mut crate::solving::radicals::route::Audit,
    budget: &mut Budget,
) -> Result<(Construction, Handle, Docket), CoefficientFailure> {
    let crate::task::Task::Solve(specification) = &request.task else {
        return Err(invalid().into());
    };
    if !request.premises.is_empty() {
        return Err(CoefficientFailure::Unsupported(
            SolveReason::CoveragePending,
        ));
    }
    let original = crate::solving::elementary::proof::problem(cas, &specification.problem)?;
    let polynomial = crate::solving::solve_search::read::polynomial(cas, original, budget)?;
    let frame = Frame {
        inner: Construction {
            header: original.header.clone(),
            context: request.context.clone(),
            region: original.region.clone(),
            steps: Vec::new(),
        },
    };
    let mut search = Search {
        cas,
        frame,
        problem: &specification.problem,
        original: polynomial,
        guards: Vec::new(),
        presentations: Docket::default(),
        audit,
        budget,
    };
    let proposal = search.degree()?;
    Ok((search.frame.inner, proposal.set, search.presentations))
}

impl Search<'_> {
    /// Decide the actual leading coefficient before every degree-dependent division.
    fn degree(&mut self) -> Result<Proposal, CoefficientFailure> {
        self.audit.mark(
            "real-degree",
            &self.frame.region,
            u64::try_from(self.guards.len()).map_err(|_| invalid())?,
            self.budget,
        )?;
        self.budget
            .solving(BudgetKind::SolutionBranches, 1, super::AT)?;
        let engine = Engine {
            state: &self.cas.state,
            domain: &self.original.domain,
        };
        let scope = EvaluationScope {
            context: &self.frame.context,
            region: &self.frame.region,
            premises: &[],
            localization: &[],
        };
        let mut count = self.original.coefficients.len();
        while count > 0 {
            let coefficient = self
                .original
                .coefficients
                .get(count - 1)
                .ok_or_else(invalid)?;
            match engine.zero_test(coefficient, &scope, self.budget)? {
                ZeroDecision::Zero(_) => count -= 1,
                ZeroDecision::Nonzero(_) => break,
                _ => {
                    let value = engine.copy(coefficient, self.budget)?;
                    return self.split_value(
                        &value,
                        PredicateSplitMode::ZeroNonzero,
                        |search, _| search.degree(),
                    );
                }
            }
        }
        let polynomial = self
            .original
            .shortened(count, &self.cas.state, self.budget)?
            .map_or_else(|| self.original.copy(&self.cas.state, self.budget), Ok)?;
        if count > 4 {
            return Err(CoefficientFailure::Unsupported(
                SolveReason::RealIsolationUnavailable,
            ));
        }
        if count <= 2 {
            let set = self.leaf()?.lower(&polynomial)?;
            return self.record(set, Vec::new());
        }
        let data = super::derive::depress(self.cas, &self.frame, &polynomial, self.budget)?;
        if count == 3 {
            return self.split_value(
                &data.radicand,
                PredicateSplitMode::RealSigns,
                |search, sign| {
                    let set = search.leaf()?.quadratic(&data, sign)?;
                    search.record(set, Vec::new())
                },
            );
        }
        let cubic = super::derive::cubic(self.cas, &self.frame, data, self.budget)?;
        self.split_value(
            &cubic.resolvent.radicand,
            PredicateSplitMode::RealSigns,
            |search, sign| search.cubic(&cubic, sign),
        )
    }

    /// Only the actual cubic discriminant and the necessary Sturm/gcd leading coefficient split further.
    fn cubic(
        &mut self,
        data: &super::derive::Cubic,
        sign: usize,
    ) -> Result<Proposal, CoefficientFailure> {
        match sign {
            0 => {
                let set = self.leaf()?.three(data)?;
                self.record(set, Vec::new())
            }
            1 => self.split_value(
                &data.linear,
                PredicateSplitMode::ZeroNonzero,
                |search, zero| {
                    let set = search.leaf()?.repeated(data, zero == 0)?;
                    search.record(set, Vec::new())
                },
            ),
            2 => self.split_value(
                &data.linear,
                PredicateSplitMode::RealSigns,
                |search, linear| {
                    let set = search.leaf()?.single(data, linear)?;
                    search.record(set, Vec::new())
                },
            ),
            _ => Err(invalid().into()),
        }
    }

    /// A local map and one-unit theorem are rebuilt in the exact child frame.
    fn leaf(&mut self) -> Result<Leaf<'_>, CoefficientFailure> {
        Leaf::new(
            self.cas,
            &mut self.frame,
            self.problem,
            &self.original,
            &self.guards,
            self.audit,
            self.budget,
        )
    }

    /// Constant alternatives require no synthetic branch; symbolic alternatives retain their total numerator guard.
    fn split_value(
        &mut self,
        value: &Element,
        mode: PredicateSplitMode,
        branch: impl Fn(&mut Self, usize) -> Result<Proposal, CoefficientFailure>,
    ) -> Result<Proposal, CoefficientFailure> {
        let constant = match super::render::rational(value, self.budget) {
            Ok(number) => Some(number),
            Err(CoefficientFailure::Unsupported(_)) => None,
            Err(error) => return Err(error),
        };
        if let Some(number) = constant {
            let order = number.compare(&crate::number::Rational::zero(), self.budget)?;
            let index = if mode == PredicateSplitMode::ZeroNonzero {
                usize::from(order != std::cmp::Ordering::Equal)
            } else {
                match order {
                    std::cmp::Ordering::Less => 0,
                    std::cmp::Ordering::Equal => 1,
                    std::cmp::Ordering::Greater => 2,
                }
            };
            return branch(self, index);
        }
        let guard = super::render::numerator(value, self.budget)?;
        let expression = super::render::expression(self.cas, &self.frame, &guard, self.budget)?;
        self.split(&guard, &expression, mode, branch)
    }

    /// Exact zero/nonzero or `RealSigns` guards are checked by the existing total-partition primitive.
    fn children(
        &mut self,
        expression: &Handle,
        mode: PredicateSplitMode,
    ) -> Result<Vec<Handle>, CoefficientFailure> {
        let zero = self.frame.expression(
            self.cas,
            Operator::Rational(crate::number::Rational::zero()),
            &[],
            false,
            self.budget,
        )?;
        let negative = self.frame.expression(
            self.cas,
            Operator::Neg,
            std::slice::from_ref(expression),
            false,
            self.budget,
        )?;
        let original = crate::solving::elementary::proof::problem(self.cas, self.problem)?;
        let coefficients = original.coefficients.clone();
        let count = if mode == PredicateSplitMode::ZeroNonzero {
            2
        } else {
            3
        };
        let mut children = vector(count, self.budget)?;
        for index in 0..count {
            let equality = mode == PredicateSplitMode::ZeroNonzero || index == 1;
            let arguments = if equality {
                vec![
                    Reference::Existing(expression.clone()),
                    Reference::Existing(zero.clone()),
                ]
            } else {
                vec![Reference::Existing(if index == 0 {
                    negative.clone()
                } else {
                    expression.clone()
                })]
            };
            let predicate = admit(
                self.cas,
                &Draft::Condition {
                    header: Reference::Existing(self.frame.header.clone()),
                    scope: Vec::new(),
                    kind: if equality {
                        PredicateKind::Equal
                    } else {
                        PredicateKind::Positive
                    },
                    arguments,
                },
                self.budget,
            )?;
            let base = Reference::Existing(self.frame.region.clone());
            let predicate = Reference::Existing(predicate);
            let expression = if mode == PredicateSplitMode::ZeroNonzero && index == 1 {
                RegionExpr::GuardFalse { base, predicate }
            } else {
                RegionExpr::GuardTrue { base, predicate }
            };
            children.push(admit(
                self.cas,
                &Draft::Region(RegionDraft {
                    header: Reference::Existing(self.frame.header.clone()),
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

    /// The total partition is a separate checked premise before any branch is entered.
    fn cover(
        &mut self,
        expression: &Handle,
        children: &[Handle],
        mode: PredicateSplitMode,
    ) -> Result<u32, CoefficientFailure> {
        let region = self.frame.region.clone();
        Ok(self.frame.append(
            Primitive::Solve(SolveSchema::TotalPredicateSplit),
            Statement::Cover {
                domain: region.clone(),
                members: crate::solving::coefficients::storage::copied(children, self.budget)?,
            },
            Certificate::Solve(Box::new(SolveCertificate::TotalPredicateSplit(
                CertificateTotalPredicateSplit {
                    mode,
                    expression: expression.clone(),
                    region,
                    children: crate::solving::coefficients::storage::copied(children, self.budget)?,
                    branches: Vec::new(),
                    cover: None,
                },
            ))),
            self.budget,
        )?)
    }

    /// Each child keeps the actual coefficient guard and its exact ordered-field alternative.
    fn guard(
        &mut self,
        guard: &Element,
        expression: &Handle,
        mode: PredicateSplitMode,
        index: usize,
    ) -> Result<(), CoefficientFailure> {
        let sign = if mode == PredicateSplitMode::ZeroNonzero {
            if index == 0 {
                Sign::Zero
            } else {
                Sign::NonZero
            }
        } else {
            match index {
                0 => Sign::Negative,
                1 => Sign::Zero,
                _ => Sign::Positive,
            }
        };
        let engine = Engine {
            state: &self.cas.state,
            domain: &guard.domain,
        };
        reserve(&mut self.guards, 1, self.budget)?;
        self.guards.push(Guard {
            value: engine.copy(guard, self.budget)?,
            expression: expression.clone(),
            sign,
            proof: None,
        });
        Ok(())
    }

    /// Both the complete child list and every closed branch proof remain explicit in the cover elimination.
    fn split(
        &mut self,
        guard: &Element,
        expression: &Handle,
        mode: PredicateSplitMode,
        branch: impl Fn(&mut Self, usize) -> Result<Proposal, CoefficientFailure>,
    ) -> Result<Proposal, CoefficientFailure> {
        let children = self.children(expression, mode)?;
        let cover = self.cover(expression, &children, mode)?;
        let mut branches = vector(0, self.budget)?;
        let mut proofs = vector(children.len(), self.budget)?;
        let mut plans = vector(children.len(), self.budget)?;
        for (index, child) in children.iter().enumerate() {
            self.guard(guard, expression, mode, index)?;
            let (proposal, proof) = self.closed(child, index, &branch)?;
            self.guards.pop();
            let SolveDraft::SolutionSet(set) =
                crate::solving::graph::object(&self.cas.state, proposal.set.logical_id())?
            else {
                return Err(invalid().into());
            };
            reserve(&mut branches, set.branches.len(), self.budget)?;
            branches.extend(set.branches.iter().cloned().map(Reference::Existing));
            proofs.push(proof);
            plans.push(proposal.plan);
        }
        let set = materialize::admit(
            self.cas,
            SolveDraft::SolutionSet(SolutionSetData {
                problem: Reference::Existing(self.problem.clone()),
                branches,
            }),
            self.budget,
        )?;
        let region = self.frame.region.clone();
        self.frame.push(
            SolveSchema::TotalPredicateSplit,
            SolveStatement::Coverage(StatementCoverage {
                problem: self.problem.clone(),
                set: set.clone(),
                region: region.clone(),
            }),
            SolveCertificate::TotalPredicateSplit(CertificateTotalPredicateSplit {
                mode: PredicateSplitMode::EliminateCover,
                expression: expression.clone(),
                region,
                children,
                branches: proofs,
                cover: Some(cover),
            }),
            self.budget,
        )?;
        self.record(set, plans)
    }

    /// Presentation is deferred until the entire coverage DAG has independently replayed.
    fn record(
        &mut self,
        set: Handle,
        children: Vec<usize>,
    ) -> Result<Proposal, CoefficientFailure> {
        let plan = self
            .presentations
            .add(&self.frame, &set, children, self.budget)?;
        Ok(Proposal { set, plan })
    }

    /// Close the actual local membership frame before a parent can consume its result.
    fn closed(
        &mut self,
        child: &Handle,
        index: usize,
        branch: &impl Fn(&mut Self, usize) -> Result<Proposal, CoefficientFailure>,
    ) -> Result<(Proposal, u32), CoefficientFailure> {
        let parent = self.frame.context.clone();
        let domain = self.frame.region.clone();
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
        self.frame.context.clone_from(&frame);
        self.frame.region.clone_from(&region);
        let result = branch(self, index);
        self.frame.context.clone_from(&parent);
        self.frame.region.clone_from(&domain);
        let proposal = result?;
        let covered = u32::try_from(self.frame.steps.len().checked_sub(1).ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        let conclusion = self
            .frame
            .steps
            .last()
            .ok_or_else(invalid)?
            .conclusion
            .clone();
        let constructor = u32::try_from(self.frame.steps.len()).map_err(|_| invalid())?;
        reserve(&mut self.frame.steps, 2, self.budget)?;
        self.frame.steps.push(ApplicationDraft {
            rule: Primitive::RegionConstruct,
            context: parent.clone(),
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
            Close {
                parent: parent.clone(),
                region: domain,
                frame: frame.clone(),
                constructor,
            },
        )?;
        self.budget.work(
            crate::graph::size(self.frame.steps.len(), super::AT)?,
            super::AT,
        )?;
        let slot = self
            .frame
            .steps
            .iter()
            .filter(|step| step.context == parent)
            .count();
        self.frame.steps.push(ApplicationDraft {
            rule: Primitive::ContextClose,
            context: parent,
            conclusion,
            premises: vec![covered, constructor],
            substitutions: Vec::new(),
            certificate: Certificate::Close { frame },
        });
        Ok((proposal, u32::try_from(slot).map_err(|_| invalid())?))
    }
}
