// SPDX-License-Identifier: AGPL-3.0-only
//! Public elementary proof fixtures with independent, explicitly supplied mathematics.

use super::common::TestResult;
use crate::test_api::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use crate::test_api::solving::{
    CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData, FamilyMapData,
    SolveDraft, SolveProblemData, SolveView, TupleData,
};
use crate::test_api::syntax::{
    AtomicSort, Branch, Declaration, Draft, Environment, ExpressionDraft, Field, Operator,
    PredicateKind, Sort,
};
use crate::test_api::{Cas, Handle, Rational, Reference};

/// Explicit graph setup for public theorem and replay tests.
pub(crate) struct Fixture {
    /// Owning graph store.
    pub(crate) cas: Cas,
    /// Fixed declaration header.
    pub(crate) header: Handle,
    /// Ordered coefficient product.
    pub(crate) coefficients: Handle,
    /// Coefficient region.
    pub(crate) region: Handle,
    /// Root context.
    pub(crate) context: Handle,
    /// Rational coefficient algebra for formal identities.
    pub(crate) algebra: Handle,
}

/// Use the same finite ceiling for each setup admission as for theorem replay.
pub(crate) fn admit(cas: &mut Cas, draft: &Draft) -> TestResult<Handle> {
    cas.admit(
        std::slice::from_ref(draft),
        &mut Budget::new(Limits::blind_cubic())?,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "missing admission".into())
}

impl Fixture {
    /// All display-independent declaration keys are fixed before any problem is built.
    pub(crate) fn new() -> TestResult<Self> {
        let mut cas = Cas::new(Limits::blind_cubic())?;
        let declarations = ["a", "b", "c", "t", "x", "y"]
            .into_iter()
            .map(|key| Declaration {
                key: key.into(),
                display_name: key.into(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            })
            .collect();
        let header = admit(
            &mut cas,
            &Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                branch: Branch::StrictReal,
                declarations,
            })),
        )?;
        let coefficients = coordinate_product(
            &mut cas,
            &header,
            &["a", "b", "c"],
            CoordinateRole::Coefficient,
        )?;
        let region = product(&mut cas, &header, &coefficients, Vec::new())?;
        let context = admit(
            &mut cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let algebra = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            )),
        )?;
        Ok(Self {
            cas,
            header,
            coefficients,
            region,
            context,
            algebra,
        })
    }

    /// Build scalar syntax with an explicit binder scope and ordered children.
    pub(crate) fn expression(
        &mut self,
        operator: Operator,
        children: &[Handle],
        bound: bool,
    ) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: if bound {
                    vec![vec![AtomicSort::Scalar(Field::Real)]]
                } else {
                    Vec::new()
                },
                operator,
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                operands: children.iter().cloned().map(Reference::Existing).collect(),
                space: None,
                binding: None,
            }),
        )
    }

    /// Exact small integer with no external arithmetic dependency.
    pub(crate) fn integer(&mut self, value: i64, bound: bool) -> TestResult<Handle> {
        let value = Rational::from_ratio(value, 1, &mut Budget::new(Limits::blind_cubic())?)?;
        self.expression(Operator::Rational(value), &[], bound)
    }

    /// One original scalar declaration.
    pub(crate) fn free(&mut self, key: &str) -> TestResult<Handle> {
        self.expression(Operator::Free(key.into()), &[], false)
    }

    /// An original equation is retained as a condition, separate from its eventual solutions.
    pub(crate) fn equation(&mut self, left: &Handle, right: &Handle) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(left.clone()),
                    Reference::Existing(right.clone()),
                ],
            },
        )
    }

    /// Construct one original univariate problem without discarding any denominator.
    pub(crate) fn problem(
        &mut self,
        key: &str,
        predicate: &Handle,
        denominators: &[Handle],
    ) -> TestResult<Handle> {
        let unknowns =
            coordinate_product(&mut self.cas, &self.header, &[key], CoordinateRole::Unknown)?;
        let domain = product(&mut self.cas, &self.header, &unknowns, Vec::new())?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                predicate: Reference::Existing(predicate.clone()),
                coefficients: Reference::Existing(self.coefficients.clone()),
                unknowns: Reference::Existing(unknowns),
                region: Reference::Existing(self.region.clone()),
                domain: Reference::Existing(domain),
                coefficient_domain: Reference::Existing(self.algebra.clone()),
                ambient: Field::Real,
                denominators: denominators
                    .iter()
                    .cloned()
                    .map(Reference::Existing)
                    .collect(),
            })),
        )
    }

    /// Supplied membership records the exact simultaneous substitution in unknown order.
    pub(crate) fn member(
        &mut self,
        problem: &Handle,
        entry: &Handle,
    ) -> TestResult<ApplicationDraft> {
        use crate::test_api::solving::{
            CertificateMembership, SolveCertificate, SolveSchema, SolveStatement,
            SolveSubstitution, StatementMember,
        };
        let SolveView::SolveProblem(original) = self.cas.solve_view(problem)? else {
            return Err("missing problem".into());
        };
        let coordinates = original.unknowns.clone();
        let SolveView::CoordinateSpace(space) = self.cas.solve_view(&coordinates)? else {
            return Err("missing unknowns".into());
        };
        let key = space
            .coordinates
            .first()
            .ok_or("missing unknown")?
            .key
            .clone();
        let tuple = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::Tuple(TupleData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                coordinates: Reference::Existing(coordinates),
                entries: vec![Reference::Existing(entry.clone())],
            })),
        )?;
        Ok(self.step(
            SolveSchema::Membership,
            SolveStatement::Member(StatementMember {
                problem: problem.clone(),
                tuple: tuple.clone(),
                region: self.region.clone(),
            }),
            SolveCertificate::Membership(CertificateMembership {
                problem: problem.clone(),
                tuple,
                region: self.region.clone(),
                substitutions: vec![SolveSubstitution {
                    key,
                    expression: entry.clone(),
                }],
                definedness: Vec::new(),
            }),
        ))
    }

    /// A single application has no hidden premises or privileged producer path.
    pub(crate) fn step(
        &self,
        schema: crate::test_api::solving::SolveSchema,
        conclusion: crate::test_api::solving::SolveStatement,
        certificate: crate::test_api::solving::SolveCertificate,
    ) -> ApplicationDraft {
        ApplicationDraft {
            rule: Primitive::Solve(schema),
            context: self.context.clone(),
            conclusion: Statement::Solve(Box::new(conclusion)),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(certificate)),
        }
    }

    /// An affine map binds its source coordinate explicitly and targets the exact unknown product.
    pub(crate) fn affine(&mut self, target: &Handle, offset: i64) -> TestResult<Handle> {
        let SolveView::SolveProblem(problem) = self.cas.solve_view(target)? else {
            return Err("missing target".into());
        };
        let target = problem.unknowns.clone();
        let family =
            coordinate_product(&mut self.cas, &self.header, &["t"], CoordinateRole::Family)?;
        let scope = vec![vec![AtomicSort::Scalar(Field::Real)]];
        let region = product(&mut self.cas, &self.header, &family, scope.clone())?;
        let parameter = self.expression(Operator::Bound { depth: 0, slot: 0 }, &[], true)?;
        let constant = self.integer(offset, true)?;
        let entry = self.expression(Operator::Add, &[parameter, constant], true)?;
        let body = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::Tuple(TupleData {
                header: Reference::Existing(self.header.clone()),
                scope,
                coordinates: Reference::Existing(target.clone()),
                entries: vec![Reference::Existing(entry)],
            })),
        )?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::FamilyMap(FamilyMapData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                coefficients: Reference::Existing(self.coefficients.clone()),
                family: Reference::Existing(family),
                region: Reference::Existing(region),
                target: Reference::Existing(target),
                body: Reference::Existing(body),
            })),
        )
    }

    /// Build exact zero/nonzero child regions and the corresponding cover certificate.
    pub(crate) fn split(&mut self, expression: &Handle) -> TestResult<ApplicationDraft> {
        use crate::test_api::solving::{
            CertificateTotalPredicateSplit, PredicateSplitMode, SolveCertificate, SolveSchema,
        };
        let zero = self.integer(0, false)?;
        let predicate = self.equation(expression, &zero)?;
        let mut children = Vec::new();
        for truth in [true, false] {
            let base = Reference::Existing(self.region.clone());
            let predicate = Reference::Existing(predicate.clone());
            let expression = if truth {
                RegionExpr::GuardTrue { base, predicate }
            } else {
                RegionExpr::GuardFalse { base, predicate }
            };
            children.push(admit(
                &mut self.cas,
                &Draft::Region(RegionDraft {
                    header: Reference::Existing(self.header.clone()),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Product {
                        coordinates: self.coefficients.clone(),
                    },
                    expression,
                }),
            )?);
        }
        Ok(ApplicationDraft {
            rule: Primitive::Solve(SolveSchema::TotalPredicateSplit),
            context: self.context.clone(),
            conclusion: Statement::Cover {
                domain: self.region.clone(),
                members: children.clone(),
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(SolveCertificate::TotalPredicateSplit(
                CertificateTotalPredicateSplit {
                    mode: PredicateSplitMode::ZeroNonzero,
                    expression: expression.clone(),
                    region: self.region.clone(),
                    children,
                    branches: Vec::new(),
                    cover: None,
                },
            ))),
        })
    }
}

/// Ordered products are graph objects; roles never come from display labels.
fn coordinate_product(
    cas: &mut Cas,
    header: &Handle,
    keys: &[&str],
    role: CoordinateRole,
) -> TestResult<Handle> {
    admit(
        cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(header.clone()),
            coordinates: keys
                .iter()
                .map(|key| Coordinate {
                    key: (*key).into(),
                    field: Field::Real,
                    role,
                })
                .collect(),
        })),
    )
}

/// Complete products have explicit scope, including a family binder when requested.
fn product(
    cas: &mut Cas,
    header: &Handle,
    coordinates: &Handle,
    scope: Vec<Vec<AtomicSort>>,
) -> TestResult<Handle> {
    admit(
        cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope,
            ambient: AmbientDomain::Product {
                coordinates: coordinates.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
    )
}

/// Independent original polynomial x²-1 with its retained x-1 hole.
pub(crate) fn hole() -> TestResult<(Fixture, Handle)> {
    let mut fixture = Fixture::new()?;
    let x = fixture.free("x")?;
    let one = fixture.integer(1, false)?;
    let zero = fixture.integer(0, false)?;
    let square = fixture.expression(Operator::IntegerPower(2), std::slice::from_ref(&x), false)?;
    let numerator = fixture.expression(Operator::Sub, &[square, one.clone()], false)?;
    let denominator = fixture.expression(Operator::Sub, &[x, one], false)?;
    let equation = fixture.equation(&numerator, &zero)?;
    let problem = fixture.problem("x", &equation, &[denominator])?;
    Ok((fixture, problem))
}

/// Translation is supplied as two explicit maps; changing either inverse must fail.
pub(crate) fn translation(offset: i64) -> TestResult<(Fixture, ApplicationDraft)> {
    use crate::test_api::solving::{
        CertificateSolutionTransport, SolveCertificate, SolveSchema, SolveStatement,
        StatementEquivalent,
    };
    let mut fixture = Fixture::new()?;
    let x = fixture.free("x")?;
    let y = fixture.free("y")?;
    let one = fixture.integer(1, false)?;
    let zero = fixture.integer(0, false)?;
    let shifted = fixture.expression(Operator::Add, &[x, one], false)?;
    let original = fixture.equation(&shifted, &zero)?;
    let reduced = fixture.equation(&y, &zero)?;
    let source = fixture.problem("x", &original, &[])?;
    let target = fixture.problem("y", &reduced, &[])?;
    let forward = fixture.affine(&target, 1)?;
    let backward = fixture.affine(&source, offset)?;
    let conclusion = SolveStatement::Equivalent(StatementEquivalent {
        source: source.clone(),
        target: target.clone(),
        forward: forward.clone(),
        backward: backward.clone(),
        region: fixture.region.clone(),
    });
    let certificate = SolveCertificate::SolutionTransport(CertificateSolutionTransport {
        source,
        target,
        forward,
        backward,
        region: fixture.region.clone(),
        definedness: Vec::new(),
        equations: Vec::new(),
        forward_inverse: Vec::new(),
        backward_inverse: Vec::new(),
    });
    let step = fixture.step(SolveSchema::SolutionTransport, conclusion, certificate);
    Ok((fixture, step))
}
