// SPDX-License-Identifier: AGPL-3.0-only
//! Public formal-root fixtures with explicitly scoped, independently supplied cubic Vieta equations.

use super::Fixture;
use crate::test_api::domain::{
    AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, HypothesisRef, RegionDraft,
    RegionExpr,
};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::proof::{ApplicationDraft, Certificate, ConstructorLaw, Primitive, Statement};
use crate::test_api::solving::{
    AdjunctionMode, CoefficientMapData, IdentityMode, MapMode, MapTarget, PolynomialData,
    PolynomialTerm, RelationMultiple, RootRelationData, ScopedCoefficientRelation,
    SolveCertificate, SolveDraft,
};
use crate::test_api::syntax::{
    AtomicSort, Declaration, Draft, ExpressionDraft, Field, Operator, PredicateKind, Scope, Sort,
};
use crate::test_api::{Handle, Rational, Reference};
use crate::testing::common::TestResult;
use crate::testing::solve_graph::admit;

/// The three hypotheses describe t³-6t²+11t-6, without adjoining any of its roots.
#[derive(Debug)]
pub(crate) struct ScopedFixture {
    /// Owning graph and current local coefficient context.
    pub(crate) base: Fixture,
    /// Original context to which the formal hypotheses will be closed.
    pub(crate) parent: Handle,
    /// Explicit root terms in the intended polynomial slot order.
    pub(crate) roots: Vec<Handle>,
    /// Three independently displayed polynomial endpoint pairs.
    pub(crate) endpoints: Vec<(Handle, Handle)>,
    /// The corresponding formal differences e1-6, e2-11 and e3-6.
    pub(crate) relations: Vec<Handle>,
    /// Exact hypothesis predicates, retained for residual assertions.
    pub(crate) predicates: Vec<Handle>,
    /// Earlier checked-hypothesis applications, supplied as ordinary untrusted drafts.
    pub(crate) hypotheses: Vec<ApplicationDraft>,
    /// Canonical ground coefficient map on this exact local region.
    pub(crate) map: Handle,
    /// Scalar expression scope shared by every hypothesis endpoint.
    scope: Scope,
    /// Field of the formal root binder.
    field: Field,
    /// One independently selected proof mutation.
    mutation: ScopedMutation,
}

/// The third free declaration and the formal bound frame remain outside coefficient towers.
fn context_fixture(field: &Field, mutation: ScopedMutation) -> TestResult<(Fixture, Scope)> {
    let mut fixture = Fixture::new()?;
    fixture.header = admit(
        &mut fixture.cas,
        &Draft::Header(HeaderDraft::Extend {
            parent: Reference::Existing(fixture.header.clone()),
            declarations: vec![Declaration {
                key: "z".into(),
                display_name: "third root".into(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            }],
            spaces: Vec::new(),
        }),
    )?;
    let scope = vec![vec![AtomicSort::Scalar(field.clone()); 3]];
    fixture.region = admit(
        &mut fixture.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: scope.clone(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
    )?;
    let assumptions = parent_assumptions(&mut fixture, &scope, field, mutation)?;
    fixture.context = admit(
        &mut fixture.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header.clone()),
            admissible: Reference::Existing(fixture.region.clone()),
            assumptions,
        }),
    )?;
    Ok((fixture, scope))
}

impl ScopedFixture {
    /// Every scalar endpoint carries the actual header and exact formal binder scope.
    pub(crate) fn expression(
        &mut self,
        operator: Operator,
        operands: &[Handle],
    ) -> TestResult<Handle> {
        admit(
            &mut self.base.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.base.header.clone()),
                operator,
                sort: Sort::Value(AtomicSort::Scalar(self.field.clone())),
                space: None,
                scope: self.scope.clone(),
                binding: None,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
            }),
        )
    }

    /// Rational constants are supplied directly through the public exact-number constructor.
    pub(crate) fn scalar(&mut self, value: i64) -> TestResult<Handle> {
        let number = Rational::from_ratio(value, 1, &mut Budget::new(Limits::default())?)?;
        self.expression(Operator::Rational(number), &[])
    }

    /// The displayed scalar elementary polynomials are built independently of sparse normalization.
    fn elementary(&mut self, index: usize) -> TestResult<Handle> {
        let [first, second, third] = self.roots.as_slice() else {
            return Err("missing formal roots".into());
        };
        let (first, second, third) = (first.clone(), second.clone(), third.clone());
        match index {
            0 => {
                let pair = self.expression(Operator::Add, &[first, second])?;
                self.expression(Operator::Add, &[pair, third])
            }
            1 => {
                let pair = self.expression(Operator::Mul, &[first.clone(), second.clone()])?;
                let outer = self.expression(Operator::Mul, &[first, third.clone()])?;
                let inner = self.expression(Operator::Mul, &[second, third])?;
                let partial = self.expression(Operator::Add, &[pair, outer])?;
                self.expression(Operator::Add, &[partial, inner])
            }
            2 => {
                let pair = self.expression(Operator::Mul, &[first, second])?;
                self.expression(Operator::Mul, &[pair, third])
            }
            _ => Err("wrong elementary degree".into()),
        }
    }

    /// Independent sparse monomials in decreasing graded-lex order.
    fn monomials(index: usize) -> TestResult<Vec<(Vec<u32>, i64)>> {
        Ok(match index {
            0 => vec![(vec![1, 0, 0], 1), (vec![0, 1, 0], 1), (vec![0, 0, 1], 1)],
            1 => vec![(vec![1, 1, 0], 1), (vec![1, 0, 1], 1), (vec![0, 1, 1], 1)],
            2 => vec![(vec![1, 1, 1], 1)],
            _ => return Err("wrong elementary degree".into()),
        })
    }

    /// Each equality retains both original scalar endpoints and two independent polynomial handles.
    fn equation(&mut self, index: usize, coefficient: i64) -> TestResult {
        let scalar_left = self.elementary(index)?;
        let scalar_right = if index == 0 && self.mutation == ScopedMutation::UndefinedEndpoint {
            let one = self.scalar(1)?;
            let zero = self.scalar(0)?;
            self.expression(Operator::Div, &[one, zero])?
        } else {
            self.scalar(coefficient)?
        };
        self.predicates.push(admit(
            &mut self.base.cas,
            &Draft::Condition {
                header: Reference::Existing(self.base.header.clone()),
                scope: self.scope.clone(),
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(scalar_left),
                    Reference::Existing(scalar_right),
                ],
            },
        )?);
        let mut terms = Self::monomials(index)?;
        let left = self.polynomial(&terms)?;
        let right = self.polynomial(&[(vec![0, 0, 0], coefficient)])?;
        self.endpoints.push((left, right));
        terms.push((vec![0, 0, 0], -coefficient));
        let relation = self.polynomial(&terms)?;
        self.relations.push(relation);
        Ok(())
    }

    /// Introduce the exact local hypotheses and reconstruct their qualified provenance slots.
    fn introduce(&mut self) -> TestResult {
        let assumptions = self
            .predicates
            .iter()
            .enumerate()
            .map(|(index, predicate)| AssumptionDraft {
                key: format!("vieta-{}", index + 1),
                condition: Reference::Existing(predicate.clone()),
            })
            .collect();
        self.base.context = admit(
            &mut self.base.cas,
            &Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(self.parent.clone()),
                region: Reference::Existing(self.base.region.clone()),
                hypotheses: assumptions,
            }),
        )?;
        self.base.region = self.base.cas.admissible_region(&self.base.context)?;
        for (index, predicate) in self.predicates.iter().enumerate() {
            self.hypotheses.push(ApplicationDraft {
                rule: Primitive::ContextHypothesis,
                context: self.base.context.clone(),
                conclusion: Statement::PredicateFact {
                    region: self.base.region.clone(),
                    predicate: predicate.clone(),
                },
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::Hypothesis(HypothesisRef {
                    context: self.base.context.clone(),
                    slot: u32::try_from(index + 1)?,
                }),
            });
        }
        self.map = admit(
            &mut self.base.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(self.base.rational.clone()),
                target: MapTarget::ScalarAmbient {
                    header: Reference::Existing(self.base.header.clone()),
                    field: Field::Complex,
                },
                mode: MapMode::Specialization,
                region: Reference::Existing(self.base.region.clone()),
                images: Vec::new(),
                denominators: Vec::new(),
            })),
        )?;
        Ok(())
    }

    /// Bound scalar roots retain an explicit ordered polynomial interpretation.
    fn new(mutation: ScopedMutation) -> TestResult<Self> {
        let field = if mutation == ScopedMutation::RealBinder {
            Field::Real
        } else {
            Field::Complex
        };
        let (base, scope) = context_fixture(&field, mutation)?;
        let parent = base.context.clone();
        let map = base.rational.clone();
        let mut result = Self {
            base,
            parent,
            roots: Vec::new(),
            endpoints: Vec::new(),
            relations: Vec::new(),
            predicates: Vec::new(),
            hypotheses: Vec::new(),
            map,
            scope,
            field,
            mutation,
        };
        for index in 0..3 {
            let root = result.expression(
                Operator::Bound {
                    depth: 0,
                    slot: index,
                },
                &[],
            )?;
            result.roots.push(root);
        }
        for (index, value) in [6, 11, 6].into_iter().enumerate() {
            result.equation(index, value)?;
        }
        if mutation == ScopedMutation::ExtraRelation {
            let root = result.roots.first().ok_or("missing root")?.clone();
            let one = result.scalar(1)?;
            result.predicates.push(admit(
                &mut result.base.cas,
                &Draft::Condition {
                    header: Reference::Existing(result.base.header.clone()),
                    scope: result.scope.clone(),
                    kind: PredicateKind::Equal,
                    arguments: vec![Reference::Existing(root), Reference::Existing(one)],
                },
            )?);
        }
        result.introduce()?;
        Ok(result)
    }

    /// Closure keeps the exact child region and leaves all undischarged hypotheses visible.
    pub(crate) fn closing_steps(
        &self,
        conclusion: &Statement,
        child: u32,
        construction: u32,
    ) -> [ApplicationDraft; 2] {
        [
            ApplicationDraft {
                rule: Primitive::RegionConstruct,
                context: self.parent.clone(),
                conclusion: Statement::Constructor {
                    region: self.base.region.clone(),
                    law: ConstructorLaw::Membership,
                },
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::None,
            },
            ApplicationDraft {
                rule: Primitive::ContextClose,
                context: self.parent.clone(),
                conclusion: conclusion.clone(),
                premises: vec![child, construction],
                substitutions: Vec::new(),
                certificate: Certificate::Close {
                    frame: self.base.context.clone(),
                },
            },
        ]
    }
}

/// Independent changes to one mathematical or scope premise of the existential proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScopedMutation {
    /// Complete satisfiable cubic system.
    Valid,
    /// All elementary equations admit every root ordering.
    SymmetricPermutation,
    /// One displayed polynomial endpoint is shifted with the other, preserving its difference.
    ChangedEndpoints,
    /// A scalar endpoint is undefined.
    UndefinedEndpoint,
    /// The scalar and formal variable namespaces are not interchangeable.
    SlotAtomSwap,
    /// A supplied predicate belongs to another local frame.
    ForeignContext,
    /// A supplied predicate names another region.
    ForeignRegion,
    /// The explicit root interpretation is incomplete.
    OmittedSlot,
    /// A required earlier equality is absent.
    MissingHypothesis,
    /// A complete local frame has an incomplete relation certificate.
    MissingRelation,
    /// The local frame contains an additional constraint.
    ExtraRelation,
    /// The anchored polynomial has a different constant coefficient.
    Polynomial,
    /// The anchored polynomial has another proved degree.
    Degree,
    /// The exact positive-degree theorem is absent.
    UnprovedDegree,
    /// The supplied leading-unit theorem concerns another coefficient.
    Leading,
    /// The desired conclusion still contains a formal root slot.
    RootConclusion,
    /// Algebraic closure does not assert real-root satisfiability.
    RealBinder,
    /// A scalar conclusion still refers to a bound root, even when reflexive.
    ScalarRootConclusion,
    /// A guard on the root variables invalidates unrestricted existential closure.
    RootRegion,
    /// Enclosing assumptions cannot constrain the formal root witness.
    ParentRootAssumption,
    /// Ordinary scoped consumption retains hypotheses at closure.
    Retained,
}

impl ScopedFixture {
    /// Sparse equations use the exact same field frame as their scalar root interpretation.
    fn polynomial(&mut self, entries: &[(Vec<u32>, i64)]) -> TestResult<Handle> {
        let mut terms = Vec::new();
        for (exponents, coefficient) in entries {
            terms.push(PolynomialTerm {
                exponents: exponents.clone(),
                coefficient: Reference::Existing(self.base.integer(*coefficient)?),
            });
        }
        admit(
            &mut self.base.cas,
            &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
                domain: Reference::Existing(self.base.rational.clone()),
                variables: vec![self.field.clone(); 3],
                terms,
            })),
        )
    }

    /// This named original polynomial is separate from every displayed Vieta endpoint.
    fn anchor(&mut self) -> TestResult<Handle> {
        let degree = if self.mutation == ScopedMutation::Degree {
            4
        } else {
            3
        };
        let leading = self
            .base
            .integer(if self.mutation == ScopedMutation::Leading {
                2
            } else {
                1
            })?;
        let constant = self
            .base
            .integer(if self.mutation == ScopedMutation::Polynomial {
                -7
            } else {
                -6
            })?;
        let middle = self.base.integer(11)?;
        let subleading = self.base.integer(-6)?;
        let domain = self.base.rational.clone();
        let polynomial = self.base.univariate(
            &domain,
            &[
                (0, constant),
                (1, middle),
                (2, subleading),
                (degree, leading),
            ],
        )?;
        admit(
            &mut self.base.cas,
            &Draft::Solve(SolveDraft::RootRelation(RootRelationData {
                header: Reference::Existing(self.base.header.clone()),
                scope: self.scope.clone(),
                ambient: Field::Complex,
                polynomial: Reference::Existing(polynomial),
                binder: vec![AtomicSort::Scalar(Field::Complex)],
                map: Reference::Existing(self.map.clone()),
                region: Reference::Existing(self.base.region.clone()),
                multiplicity: None,
            })),
        )
    }

    /// A degree-generic syzygy uses all three supplied relations without adjoining any root.
    fn identity(&mut self, anchor: &Handle) -> TestResult<ApplicationDraft> {
        let left = self.scalar(36)?;
        let six = self.scalar(6)?;
        let square = self.expression(Operator::Mul, &[six.clone(), six])?;
        let mut step = self.base.identity(&left, &square, &self.map);
        step.premises = vec![1, 2, 3, 4];
        let middle = self.polynomial(&[
            (vec![1, 1, 1], 1),
            (vec![1, 0, 0], -1),
            (vec![0, 1, 0], -1),
            (vec![0, 0, 1], -1),
        ])?;
        let mut negative = Self::monomials(1)?;
        for (_, coefficient) in &mut negative {
            *coefficient = -*coefficient;
        }
        negative.push((vec![0, 0, 0], 11));
        let multipliers = [
            self.relations.get(1).ok_or("missing e2")?.clone(),
            middle,
            self.polynomial(&negative)?,
        ];
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("missing identity".into());
        };
        let SolveCertificate::CoefficientIdentity(identity) = certificate.as_mut() else {
            return Err("wrong identity".into());
        };
        identity.mode = IdentityMode::RelationCombination;
        for (index, multiplier) in multipliers.into_iter().enumerate() {
            let (left, right) = self.endpoints.get(index).ok_or("missing endpoints")?;
            identity.combinations.push(RelationMultiple {
                multiplier,
                relation: self.relations.get(index).ok_or("missing relation")?.clone(),
                hypothesis: Some(ScopedCoefficientRelation {
                    premise: u32::try_from(index + 1)?,
                    left: left.clone(),
                    right: right.clone(),
                    roots: self.roots.clone(),
                    vieta: Some(anchor.clone()),
                }),
            });
        }
        Ok(step)
    }

    /// Mutations retain valid graph syntax and alter independently checked proof evidence.
    fn mutate(&mut self, step: &mut ApplicationDraft) -> TestResult {
        if self.mutation == ScopedMutation::MissingHypothesis {
            step.premises.remove(1);
        }
        if self.mutation == ScopedMutation::UnprovedDegree {
            step.premises.remove(0);
        }
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("missing identity".into());
        };
        let SolveCertificate::CoefficientIdentity(identity) = certificate.as_mut() else {
            return Err("wrong identity".into());
        };
        if self.mutation == ScopedMutation::MissingRelation {
            identity.combinations.pop();
            let mut terms = Self::monomials(0)?;
            for (_, coefficient) in &mut terms {
                *coefficient = -*coefficient;
            }
            terms.push((vec![0, 0, 0], 6));
            identity
                .combinations
                .get_mut(1)
                .ok_or("missing second row")?
                .multiplier = self.polynomial(&terms)?;
        }
        if self.mutation == ScopedMutation::SlotAtomSwap {
            identity.combinations.truncate(1);
            identity
                .combinations
                .first_mut()
                .ok_or("missing first row")?
                .multiplier = self.polynomial(&[(vec![0, 0, 0], 1)])?;
        }
        for row in &mut identity.combinations {
            let evidence = row.hypothesis.as_mut().ok_or("missing evidence")?;
            if self.mutation == ScopedMutation::SymmetricPermutation {
                evidence.roots.swap(0, 2);
            }
            if matches!(
                self.mutation,
                ScopedMutation::Retained | ScopedMutation::SlotAtomSwap
            ) {
                evidence.vieta = None;
            }
            if self.mutation == ScopedMutation::UnprovedDegree {
                evidence.premise -= 1;
            }
        }
        let first = identity
            .combinations
            .first_mut()
            .ok_or("missing row")?
            .hypothesis
            .as_mut()
            .ok_or("missing evidence")?;
        if self.mutation == ScopedMutation::OmittedSlot {
            first.roots.pop();
        }
        if self.mutation == ScopedMutation::ChangedEndpoints {
            let mut terms = Self::monomials(0)?;
            terms.push((vec![0, 0, 0], 1));
            first.left = self.polynomial(&terms)?;
            first.right = self.polynomial(&[(vec![0, 0, 0], 7)])?;
        }
        Ok(())
    }

    /// Change only the desired endpoints, keeping certificate and conclusion in agreement.
    fn conclusion_mutation(&mut self, step: &mut ApplicationDraft) -> TestResult {
        let endpoints = match self.mutation {
            ScopedMutation::RootConclusion => {
                let root = self.polynomial(&[(vec![1, 0, 0], 1)])?;
                Some((root.clone(), root))
            }
            ScopedMutation::ScalarRootConclusion => {
                let root = self.roots.first().ok_or("missing root")?.clone();
                Some((root.clone(), root))
            }
            ScopedMutation::SlotAtomSwap => {
                let left = self.elementary(0)?;
                let right = self.scalar(6)?;
                Some((left, right))
            }
            _ => None,
        };
        if let Some((left, right)) = endpoints {
            step.conclusion = self.base.identity(&left, &right, &self.map).conclusion;
            let Certificate::Solve(certificate) = &mut step.certificate else {
                return Err("missing identity".into());
            };
            let SolveCertificate::CoefficientIdentity(identity) = certificate.as_mut() else {
                return Err("wrong identity".into());
            };
            identity.left = left;
            identity.right = right;
        }
        Ok(())
    }
}

/// Generation and replay consume the same untrusted public applications and exact cubic anchor.
pub(crate) fn scoped_identity(
    mutation: ScopedMutation,
) -> TestResult<(Fixture, Vec<ApplicationDraft>)> {
    let mut fixture = ScopedFixture::new(mutation)?;
    let anchor = fixture.anchor()?;
    let unit = fixture.base.ground_unit()?;
    let existence = fixture
        .base
        .adjunction(&anchor, None, AdjunctionMode::ComplexFTA, None);
    let mut step = fixture.identity(&anchor)?;
    fixture.mutate(&mut step)?;
    fixture.conclusion_mutation(&mut step)?;
    if mutation == ScopedMutation::ForeignRegion {
        let other = admit(
            &mut fixture.base.cas,
            &Draft::Region(RegionDraft {
                header: Reference::Existing(fixture.base.header.clone()),
                scope: fixture.scope.clone(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Empty,
            }),
        )?;
        if let Statement::PredicateFact { region, .. } = &mut fixture
            .hypotheses
            .first_mut()
            .ok_or("missing hypothesis")?
            .conclusion
        {
            *region = other;
        }
    }
    if mutation == ScopedMutation::ForeignContext {
        fixture.foreign_context()?;
    }
    let mut steps = vec![unit, existence];
    steps.extend(fixture.hypotheses.iter().take(3).cloned());
    let close = fixture.closing_steps(&step.conclusion, 5, 6);
    steps.push(step);
    steps.extend(close);
    fixture.base.context = fixture.parent;
    Ok((fixture.base, steps))
}

impl ScopedFixture {
    /// A sibling frame has a different qualified provenance even with the same mathematical predicate.
    fn foreign_context(&mut self) -> TestResult {
        let predicate = self.predicates.first().ok_or("missing predicate")?;
        let sibling = admit(
            &mut self.base.cas,
            &Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(self.parent.clone()),
                region: Reference::Existing(self.base.region.clone()),
                hypotheses: vec![AssumptionDraft {
                    key: "foreign".into(),
                    condition: Reference::Existing(predicate.clone()),
                }],
            }),
        )?;
        let premise = self.hypotheses.first_mut().ok_or("missing hypothesis")?;
        premise.context = sibling.clone();
        premise.certificate = Certificate::Hypothesis(HypothesisRef {
            context: sibling.clone(),
            slot: 1,
        });
        if let Statement::PredicateFact { region, .. } = &mut premise.conclusion {
            *region = self.base.cas.admissible_region(&sibling)?;
        }
        Ok(())
    }
}

/// An asymmetric equation exposes an incorrect order that symmetric Vieta equations cannot distinguish.
pub(crate) fn asymmetric_relation(permuted: bool) -> TestResult<(Fixture, Vec<ApplicationDraft>)> {
    let mut fixture = ScopedFixture::new(ScopedMutation::Valid)?;
    asymmetric_hypothesis(&mut fixture)?;
    let left = fixture.polynomial(&[(vec![1, 0, 0], 1), (vec![0, 1, 0], 2)])?;
    let right = fixture.polynomial(&[(vec![0, 0, 0], 6)])?;
    let relation =
        fixture.polynomial(&[(vec![1, 0, 0], 1), (vec![0, 1, 0], 2), (vec![0, 0, 0], -6)])?;
    let multiplier = fixture.polynomial(&[(vec![0, 0, 0], 1)])?;
    let mut step = fixture.base.identity(&left, &right, &fixture.map);
    step.premises = vec![0];
    let mut roots = fixture.roots.clone();
    if permuted {
        roots.swap(0, 1);
    }
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("missing identity".into());
    };
    let SolveCertificate::CoefficientIdentity(identity) = certificate.as_mut() else {
        return Err("wrong identity".into());
    };
    identity.mode = IdentityMode::RelationCombination;
    identity.combinations = vec![RelationMultiple {
        multiplier,
        relation,
        hypothesis: Some(ScopedCoefficientRelation {
            premise: 0,
            left,
            right,
            roots,
            vieta: None,
        }),
    }];
    let close = fixture.closing_steps(&step.conclusion, 1, 2);
    let mut steps = fixture.hypotheses;
    steps.push(step);
    steps.extend(close);
    fixture.base.context = fixture.parent;
    Ok((fixture.base, steps))
}

/// Independently supplied outer restrictions distinguish existential roots from arbitrary assignments.
fn bound_predicate(fixture: &mut Fixture, scope: &Scope, field: &Field) -> TestResult<Handle> {
    let mut arguments = Vec::new();
    for operator in [
        Operator::Bound { depth: 0, slot: 0 },
        Operator::Rational(Rational::zero()),
    ] {
        let expression = admit(
            &mut fixture.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(fixture.header.clone()),
                operator,
                sort: Sort::Value(AtomicSort::Scalar(field.clone())),
                space: None,
                scope: scope.clone(),
                binding: None,
                operands: Vec::new(),
            }),
        )?;
        arguments.push(Reference::Existing(expression));
    }
    admit(
        &mut fixture.cas,
        &Draft::Condition {
            header: Reference::Existing(fixture.header.clone()),
            scope: scope.clone(),
            kind: PredicateKind::Equal,
            arguments,
        },
    )
}

/// Root-dependent surrounding conditions remain distinct from the local Vieta equations.
fn parent_assumptions(
    fixture: &mut Fixture,
    scope: &Scope,
    field: &Field,
    mutation: ScopedMutation,
) -> TestResult<Vec<AssumptionDraft>> {
    let mut assumptions = Vec::new();
    if matches!(
        mutation,
        ScopedMutation::RootRegion | ScopedMutation::ParentRootAssumption
    ) {
        let predicate = bound_predicate(fixture, scope, field)?;
        if mutation == ScopedMutation::RootRegion {
            fixture.region = admit(
                &mut fixture.cas,
                &Draft::Region(RegionDraft {
                    header: Reference::Existing(fixture.header.clone()),
                    scope: scope.clone(),
                    ambient: AmbientDomain::Assignments,
                    expression: RegionExpr::GuardTrue {
                        base: Reference::Existing(fixture.region.clone()),
                        predicate: Reference::Existing(predicate),
                    },
                }),
            )?;
        } else {
            assumptions.push(AssumptionDraft {
                key: "restricted-root".into(),
                condition: Reference::Existing(predicate),
            });
        }
    }
    Ok(assumptions)
}

/// Supply the asymmetric scalar premise without reusing the displayed sparse polynomial.
fn asymmetric_hypothesis(fixture: &mut ScopedFixture) -> TestResult {
    let first = fixture.roots.first().ok_or("missing first root")?.clone();
    let second = fixture.roots.get(1).ok_or("missing second root")?.clone();
    let two = fixture.scalar(2)?;
    let product = fixture.expression(Operator::Mul, &[two, second])?;
    let scalar_left = fixture.expression(Operator::Add, &[first, product])?;
    let scalar_right = fixture.scalar(6)?;
    let predicate = admit(
        &mut fixture.base.cas,
        &Draft::Condition {
            header: Reference::Existing(fixture.base.header.clone()),
            scope: fixture.scope.clone(),
            kind: PredicateKind::Equal,
            arguments: vec![
                Reference::Existing(scalar_left),
                Reference::Existing(scalar_right),
            ],
        },
    )?;
    fixture.predicates = vec![predicate];
    fixture.hypotheses.clear();
    fixture.introduce()?;
    Ok(())
}

/// The same closed scalar theorem is valid in an empty context and refused in a bound context.
pub(crate) fn scoped_scalar(scoped: bool) -> TestResult<(Fixture, ApplicationDraft)> {
    use crate::test_api::proof::DomainContract;
    let mut fixture = if scoped {
        context_fixture(&Field::Real, ScopedMutation::Valid)?.0
    } else {
        Fixture::new()?
    };
    let scalar = fixture.guard_expression(
        Operator::Rational(Rational::from_ratio(
            6,
            1,
            &mut Budget::new(Limits::default())?,
        )?),
        &[],
    )?;
    let step = ApplicationDraft {
        rule: Primitive::Reflexivity,
        context: fixture.context.clone(),
        conclusion: Statement::Equality {
            left: scalar.clone(),
            right: scalar,
            domain: fixture.region.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    Ok((fixture, step))
}

/// A fully walked scalar request reaches the shared producer boundary with explicit context scope.
pub(crate) fn scoped_request(
    scoped: bool,
) -> TestResult<(Fixture, crate::test_api::request::Request)> {
    use crate::test_api::limits::LimitsOverride;
    use crate::test_api::request::{CoveredInput, Request, Task};
    let (mut fixture, step) = scoped_scalar(scoped)?;
    let Statement::Equality { left, .. } = step.conclusion else {
        return Err("missing scalar input".into());
    };
    let source = admit(
        &mut fixture.cas,
        &Draft::Source {
            key: "scoped-scalar".into(),
            text: "6".into(),
        },
    )?;
    let root =
        fixture
            .cas
            .materialize_original(&left, &source, &mut Budget::new(Limits::default())?)?;
    let walk = fixture
        .cas
        .walk_original(&root, &mut Budget::new(Limits::default())?)?;
    let request = Request {
        task: Task::Simplify,
        context: fixture.context.clone(),
        inputs: vec![CoveredInput {
            root,
            receipt: walk.receipt,
        }],
        premises: Vec::new(),
        limits: LimitsOverride::default(),
    };
    Ok((fixture, request))
}

/// A closed predicate cannot be silently installed under an unrelated bound frame.
pub(crate) fn scoped_context_mismatch(
    local: bool,
    restriction: bool,
) -> TestResult<(Fixture, Draft)> {
    let (mut fixture, _) = context_fixture(&Field::Real, ScopedMutation::Valid)?;
    let scalar = fixture.guard_expression(
        Operator::Rational(Rational::from_ratio(
            1,
            1,
            &mut Budget::new(Limits::default())?,
        )?),
        &[],
    )?;
    let predicate = admit(
        &mut fixture.cas,
        &Draft::Condition {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::NonZero,
            arguments: vec![Reference::Existing(scalar)],
        },
    )?;
    let mut assumptions = vec![AssumptionDraft {
        key: "wrong-scope".into(),
        condition: Reference::Existing(predicate),
    }];
    let region = if restriction {
        assumptions.clear();
        admit(
            &mut fixture.cas,
            &Draft::Region(RegionDraft {
                header: Reference::Existing(fixture.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
        )?
    } else {
        fixture.region.clone()
    };
    let draft = if local {
        ContextDraft::Local {
            parent: Reference::Existing(fixture.context.clone()),
            region: Reference::Existing(region),
            hypotheses: assumptions,
        }
    } else {
        ContextDraft::Root {
            header: Reference::Existing(fixture.header.clone()),
            admissible: Reference::Existing(region),
            assumptions,
        }
    };
    Ok((fixture, Draft::Context(draft)))
}

/// A supporting root-existence conclusion must not refer to an outer formal-root restriction.
pub(crate) fn scoped_dependent_adjunction() -> TestResult<(Fixture, Vec<ApplicationDraft>)> {
    let mut fixture = ScopedFixture::new(ScopedMutation::Valid)?;
    let unit = fixture.base.ground_unit()?;
    let predicate = bound_predicate(&mut fixture.base, &fixture.scope, &fixture.field)?;
    fixture.base.region = admit(
        &mut fixture.base.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.base.header.clone()),
            scope: fixture.scope.clone(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(fixture.base.region.clone()),
                predicate: Reference::Existing(predicate),
            },
        }),
    )?;
    fixture.map = admit(
        &mut fixture.base.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(fixture.base.rational.clone()),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(fixture.base.header.clone()),
                field: Field::Complex,
            },
            mode: MapMode::Specialization,
            region: Reference::Existing(fixture.base.region.clone()),
            images: Vec::new(),
            denominators: Vec::new(),
        })),
    )?;
    let anchor = fixture.anchor()?;
    let step = fixture
        .base
        .adjunction(&anchor, None, AdjunctionMode::ComplexFTA, None);
    Ok((fixture.base, vec![unit, step]))
}

/// A receiving closed context cannot hide the scope of a supplied verified premise.
pub(crate) fn scoped_premise_fixture() -> TestResult<(
    Fixture,
    Vec<ApplicationDraft>,
    crate::test_api::request::Request,
)> {
    use crate::test_api::limits::LimitsOverride;
    use crate::test_api::request::{CoveredInput, Request, Task};
    let (mut fixture, steps) = scoped_identity(ScopedMutation::Valid)?;
    let region = admit(
        &mut fixture.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
    )?;
    let context = admit(
        &mut fixture.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header.clone()),
            admissible: Reference::Existing(region),
            assumptions: Vec::new(),
        }),
    )?;
    let scalar = fixture.guard_expression(
        Operator::Rational(Rational::from_ratio(
            6,
            1,
            &mut Budget::new(Limits::default())?,
        )?),
        &[],
    )?;
    let source = admit(
        &mut fixture.cas,
        &Draft::Source {
            key: "closed-consumer".into(),
            text: "6".into(),
        },
    )?;
    let root =
        fixture
            .cas
            .materialize_original(&scalar, &source, &mut Budget::new(Limits::default())?)?;
    let walk = fixture
        .cas
        .walk_original(&root, &mut Budget::new(Limits::default())?)?;
    let request = Request {
        task: Task::Simplify,
        context,
        inputs: vec![CoveredInput {
            root,
            receipt: walk.receipt,
        }],
        premises: Vec::new(),
        limits: LimitsOverride::default(),
    };
    Ok((fixture, steps, request))
}
