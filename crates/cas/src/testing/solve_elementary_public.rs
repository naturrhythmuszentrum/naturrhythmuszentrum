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
    /// Scalar ambient of this fixture.
    pub(crate) field: Field,
    /// Rational coefficient algebra for formal identities.
    pub(crate) algebra: Handle,
}

/// Setup permits a bounded degree-five tower; ordinary proofs retain their own smaller budgets.
pub(crate) fn admit(cas: &mut Cas, draft: &Draft) -> TestResult<Handle> {
    cas.admit(
        std::slice::from_ref(draft),
        &mut Budget::new(Limits {
            tower_degree: 32,
            ..Limits::blind_cubic()
        })?,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "missing admission".into())
}

impl Fixture {
    /// All display-independent declaration keys are fixed before any problem is built.
    pub(crate) fn new() -> TestResult<Self> {
        Self::in_field(&Field::Real)
    }

    /// Construct a separate ambient header; fields are never silently coerced.
    pub(crate) fn in_field(field: &Field) -> TestResult<Self> {
        Self::with_limits(field, Limits::blind_cubic())
    }

    /// Separate finite setup limits permit degree-generic fixtures beyond the pinned cubic profile.
    fn with_limits(field: &Field, limits: Limits) -> TestResult<Self> {
        let mut cas = Cas::new(limits)?;
        let declarations = ["a", "b", "c", "t", "x", "y"]
            .into_iter()
            .map(|key| Declaration {
                key: key.into(),
                display_name: key.into(),
                sort: Sort::Value(AtomicSort::Scalar(field.clone())),
            })
            .collect();
        let header = admit(
            &mut cas,
            &Draft::Header(HeaderDraft::Core(Environment {
                field: field.clone(),
                branch: if *field == Field::Complex {
                    Branch::PrincipalComplex
                } else {
                    Branch::StrictReal
                },
                declarations,
            })),
        )?;
        let coefficients = coordinate_product(
            &mut cas,
            &header,
            &["a", "b", "c"],
            CoordinateRole::Coefficient,
            field,
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
            field: field.clone(),
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
                    vec![vec![AtomicSort::Scalar(self.field.clone())]]
                } else {
                    Vec::new()
                },
                operator,
                sort: Sort::Value(AtomicSort::Scalar(self.field.clone())),
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
        let unknowns = coordinate_product(
            &mut self.cas,
            &self.header,
            &[key],
            CoordinateRole::Unknown,
            &self.field,
        )?;
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
                ambient: self.field.clone(),
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
        let family = coordinate_product(
            &mut self.cas,
            &self.header,
            &["t"],
            CoordinateRole::Family,
            &self.field,
        )?;
        let scope = vec![vec![AtomicSort::Scalar(self.field.clone())]];
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
    field: &Field,
) -> TestResult<Handle> {
    admit(
        cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(header.clone()),
            coordinates: keys
                .iter()
                .map(|key| Coordinate {
                    key: (*key).into(),
                    field: field.clone(),
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

impl Fixture {
    /// Ground graph coefficients are distinct from scalar expression literals.
    pub(crate) fn coefficient(&mut self, integer: i64) -> TestResult<Handle> {
        use crate::test_api::solving::{CoefficientValueData, CoefficientValueObject};
        let value = Rational::from_ratio(integer, 1, &mut Budget::new(Limits::blind_cubic())?)?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(self.algebra.clone()),
                value: CoefficientValueData::Rational { value },
            })),
        )
    }

    /// Supply exact canonical univariate terms in decreasing order.
    pub(crate) fn polynomial(
        &mut self,
        domain: &Handle,
        terms: &[(u32, Handle)],
    ) -> TestResult<Handle> {
        use crate::test_api::solving::{PolynomialData, PolynomialTerm};
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
                domain: Reference::Existing(domain.clone()),
                variables: vec![self.field.clone()],
                terms: terms
                    .iter()
                    .map(|(exponent, coefficient)| PolynomialTerm {
                        exponents: vec![*exponent],
                        coefficient: Reference::Existing(coefficient.clone()),
                    })
                    .collect(),
            })),
        )
    }

    /// Scalar maps explicitly preserve the ambient, region and every generator image.
    pub(crate) fn scalar_map(&mut self, domain: &Handle, images: &[Handle]) -> TestResult<Handle> {
        use crate::test_api::solving::{CoefficientMapData, MapMode, MapTarget};
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(domain.clone()),
                target: MapTarget::ScalarAmbient {
                    header: Reference::Existing(self.header.clone()),
                    field: self.field.clone(),
                },
                mode: MapMode::Specialization,
                region: Reference::Existing(self.region.clone()),
                images: images.iter().cloned().map(Reference::Existing).collect(),
                denominators: Vec::new(),
            })),
        )
    }

    /// A root relation and an unconstrained one-slot selection retain exact coefficient interpretation.
    pub(crate) fn root(
        &mut self,
        polynomial: &Handle,
        map: &Handle,
    ) -> TestResult<(Handle, Handle, Handle)> {
        use crate::test_api::solving::{RootRelationData, RootSelectionData, SelectionMode};
        let relation = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::RootRelation(RootRelationData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: self.field.clone(),
                polynomial: Reference::Existing(polynomial.clone()),
                binder: vec![AtomicSort::Scalar(self.field.clone())],
                map: Reference::Existing(map.clone()),
                region: Reference::Existing(self.region.clone()),
                multiplicity: None,
            })),
        )?;
        let bound = self.expression(Operator::Bound { depth: 0, slot: 0 }, &[], true)?;
        let predicate = admit(
            &mut self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: vec![vec![AtomicSort::Scalar(self.field.clone())]],
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(bound.clone()),
                    Reference::Existing(bound),
                ],
            },
        )?;
        let selection = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::RootSelection(RootSelectionData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                relations: vec![Reference::Existing(relation.clone())],
                predicate: Reference::Existing(predicate),
                mode: SelectionMode::Any,
                convention: None,
                interval: None,
            })),
        )?;
        let root = admit(
            &mut self.cas,
            &Draft::RootValue {
                header: Reference::Existing(self.header.clone()),
                selection: Reference::Existing(selection.clone()),
                slot: 0,
            },
        )?;
        Ok((relation, selection, root))
    }

    /// A monic quotient has no implicit field or square-free theorem.
    pub(crate) fn tower(&mut self, parent: &Handle, polynomial: &Handle) -> TestResult<Handle> {
        use crate::test_api::solving::{ExtensionLevelData, SquareFreeRequest};
        let level = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::ExtensionLevel(ExtensionLevelData {
                parent: Reference::Existing(parent.clone()),
                polynomial: Reference::Existing(polynomial.clone()),
                generator: vec![AtomicSort::Scalar(self.field.clone())],
                square_free: SquareFreeRequest::NotRequested,
            })),
        )?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Quotient {
                    level: Reference::Existing(level),
                },
            )),
        )
    }

    /// A reduced tower vector and its scalar literal use the same explicit map.
    pub(crate) fn tower_value(
        &mut self,
        domain: &Handle,
        coefficients: &[Handle],
    ) -> TestResult<Handle> {
        use crate::test_api::solving::{CoefficientValueData, CoefficientValueObject};
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(domain.clone()),
                value: CoefficientValueData::Tower {
                    coefficients: coefficients
                        .iter()
                        .cloned()
                        .map(Reference::Existing)
                        .collect(),
                },
            })),
        )
    }

    /// Exact coefficients are embedded only through an admitted map.
    pub(crate) fn literal(&mut self, value: &Handle, map: &Handle) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            &Draft::ExactCoefficient {
                header: Reference::Existing(self.header.clone()),
                value: Reference::Existing(value.clone()),
                map: Reference::Existing(map.clone()),
            },
        )
    }

    /// A complete one-unit certificate reconstructs 1*1+m*0=1 over its actual parent algebra.
    pub(crate) fn unit(
        &self,
        value: &Handle,
        domain: &Handle,
        modulus: &Handle,
        one: &Handle,
        zero: &Handle,
    ) -> ApplicationDraft {
        use crate::test_api::solving::{
            CertificateExtensionUnit, SolveCertificate, SolveSchema, SolveStatement, StatementUnit,
        };
        self.step(
            SolveSchema::ExtensionUnit,
            SolveStatement::Unit(StatementUnit {
                value: value.clone(),
                inverse: value.clone(),
                domain: domain.clone(),
                region: self.region.clone(),
            }),
            SolveCertificate::ExtensionUnit(CertificateExtensionUnit {
                element: value.clone(),
                modulus: modulus.clone(),
                gcd: one.clone(),
                quotients: vec![one.clone(), modulus.clone()],
                bezout: vec![one.clone(), zero.clone()],
                region: self.region.clone(),
            }),
        )
    }

    /// Named algebraic closure has the exact leading-unit premise.
    pub(crate) fn adjoin(
        &self,
        relation: &Handle,
        selection: &Handle,
        leading: u32,
    ) -> ApplicationDraft {
        use crate::test_api::solving::{
            AdjunctionMode, CertificateRootAdjunction, SolveCertificate, SolveSchema,
            SolveStatement, StatementRootExistence,
        };
        let mut step = self.step(
            SolveSchema::RootAdjunction,
            SolveStatement::RootExistence(StatementRootExistence {
                relation: selection.clone(),
                region: self.region.clone(),
            }),
            SolveCertificate::RootAdjunction(CertificateRootAdjunction {
                mode: AdjunctionMode::ComplexFTA,
                relation: relation.clone(),
                selection: Some(selection.clone()),
                leading_unit: 0,
                order: None,
                splitting_chain: Vec::new(),
            }),
        );
        step.premises = vec![leading];
        step
    }

    /// Normalize an independently supplied factor identity in the explicit coefficient tower.
    pub(crate) fn identity(&self, left: &Handle, right: &Handle, map: &Handle) -> ApplicationDraft {
        use crate::test_api::solving::{
            CertificateCoefficientIdentity, IdentityMode, SolveCertificate, SolveSchema,
            SolveStatement, StatementCoefficientEquality,
        };
        self.step(
            SolveSchema::CoefficientIdentity,
            SolveStatement::CoefficientEquality(StatementCoefficientEquality {
                left: left.clone(),
                right: right.clone(),
                map: map.clone(),
                region: self.region.clone(),
            }),
            SolveCertificate::CoefficientIdentity(CertificateCoefficientIdentity {
                mode: IdentityMode::Normalize,
                left: left.clone(),
                right: right.clone(),
                map: map.clone(),
                region: self.region.clone(),
                combinations: Vec::new(),
                denominators: Vec::new(),
            }),
        )
    }

    /// Finite solution proposals have no complete bit and include the exact original unknown domain.
    pub(crate) fn finite(&mut self, problem: &Handle, entries: &[Handle]) -> TestResult<Handle> {
        use crate::test_api::solving::{SolutionBranchData, SolutionBranchKind, SolutionSetData};
        let SolveView::SolveProblem(original) = self.cas.solve_view(problem)? else {
            return Err("missing problem".into());
        };
        let coordinates = original.unknowns.clone();
        let domain = original.domain.clone();
        let mut tuples = Vec::new();
        for entry in entries {
            tuples.push(Reference::Existing(admit(
                &mut self.cas,
                &Draft::Solve(SolveDraft::Tuple(TupleData {
                    header: Reference::Existing(self.header.clone()),
                    scope: Vec::new(),
                    coordinates: Reference::Existing(coordinates.clone()),
                    entries: vec![Reference::Existing(entry.clone())],
                })),
            )?));
        }
        let kind = if entries.is_empty() {
            SolutionBranchKind::Empty
        } else {
            SolutionBranchKind::Finite { tuples }
        };
        let branch = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::SolutionBranch(SolutionBranchData {
                problem: Reference::Existing(problem.clone()),
                region: Reference::Existing(self.region.clone()),
                domain: Reference::Existing(domain),
                kind,
            })),
        )?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::SolutionSet(SolutionSetData {
                problem: Reference::Existing(problem.clone()),
                branches: vec![Reference::Existing(branch)],
            })),
        )
    }
}

/// A full supplied binomial lifecycle, including exact order, tower product and both cover rules.
pub(crate) struct BinomialFixture {
    /// Owning elementary setup.
    pub(crate) base: Fixture,
    /// All reachable applications, ending in complete binomial coverage.
    pub(crate) steps: Vec<ApplicationDraft>,
}

/// Root-of-unity setup remains independent of any solving recipe under test.
struct UnityFixture {
    domain: Handle,
    map: Handle,
    selection: Handle,
    one: Handle,
    zero: Handle,
    generator: Handle,
    unit: ApplicationDraft,
    steps: Vec<ApplicationDraft>,
}

impl Fixture {
    /// Prime orders use the independently known geometric polynomial 1+X+...+X^(n-1).
    fn unity(&mut self, order: u32) -> TestResult<UnityFixture> {
        use crate::test_api::solving::{
            BinomialMode, CertificateBinomialCoverage, PrimePower, SolveCertificate, SolveSchema,
            SolveStatement, StatementRootOrder,
        };
        let algebra = self.algebra.clone();
        let one = self.coefficient(1)?;
        let zero = self.coefficient(0)?;
        let constant = self.polynomial(&algebra, &[(0, one.clone())])?;
        let empty = self.polynomial(&algebra, &[])?;
        let modulus = self.polynomial(&algebra, &[(1, one.clone())])?;
        let unit = self.unit(&one, &algebra, &modulus, &constant, &empty);
        let terms: Vec<_> = (0..order)
            .rev()
            .map(|exponent| (exponent, one.clone()))
            .collect();
        let cyclotomic = self.polynomial(&algebra, &terms)?;
        let ground = self.scalar_map(&algebra, &[])?;
        let (relation, selection, root) = self.root(&cyclotomic, &ground)?;
        let existence = self.adjoin(&relation, &selection, 0);
        let mut exact_order = self.step(
            SolveSchema::BinomialCoverage,
            SolveStatement::RootOrder(StatementRootOrder {
                value: root.clone(),
                order,
                region: self.region.clone(),
            }),
            SolveCertificate::BinomialCoverage(CertificateBinomialCoverage {
                mode: BinomialMode::Order,
                order,
                unity: Some(selection.clone()),
                selection: None,
                factors: vec![PrimePower {
                    prime: order,
                    exponent: 1,
                }],
                exclusions: Vec::new(),
                product: None,
            }),
        );
        exact_order.premises = vec![1];
        let domain = self.tower(&algebra, &cyclotomic)?;
        let unit_value = self.tower_value(&domain, std::slice::from_ref(&one))?;
        let zero_value = self.tower_value(&domain, &[])?;
        let generator = if order == 2 {
            let negative = self.coefficient(-1)?;
            self.tower_value(&domain, &[negative])?
        } else {
            self.tower_value(&domain, &[zero, one])?
        };
        let unit_certificate = self.unit(&unit_value, &domain, &cyclotomic, &constant, &empty);
        let map = self.scalar_map(&domain, &[root])?;
        Ok(UnityFixture {
            domain,
            map,
            selection,
            one: unit_value,
            zero: zero_value,
            generator,
            unit: unit_certificate,
            steps: vec![unit, existence, exact_order],
        })
    }

    /// The degree-generic factor list is constructed from one shared primitive root and one root choice.
    fn binomial_factors(
        &mut self,
        order: u32,
        unity: &Handle,
        root: &Handle,
        unknown: &Handle,
    ) -> TestResult<(Vec<Handle>, Handle)> {
        let mut entries = Vec::new();
        let mut product = self.integer(1, false)?;
        for exponent in 0..order {
            let value = if exponent == 0 {
                root.clone()
            } else {
                let power = self.expression(
                    Operator::IntegerPower(i64::from(exponent)),
                    std::slice::from_ref(unity),
                    false,
                )?;
                self.expression(Operator::Mul, &[power, root.clone()], false)?
            };
            let factor =
                self.expression(Operator::Sub, &[unknown.clone(), value.clone()], false)?;
            product = self.expression(Operator::Mul, &[product, factor], false)?;
            entries.push(value);
        }
        Ok((entries, product))
    }
}

impl BinomialFixture {
    /// No reference quadratic formula is supplied: only the binomial relation and cyclic product.
    pub(crate) fn new(order: u32, radicand: i64) -> TestResult<Self> {
        let mut base = Fixture::with_limits(
            &Field::Complex,
            Limits {
                tower_degree: 32,
                ..Limits::blind_cubic()
            },
        )?;
        let mut unity = base.unity(order)?;
        let negative = base.coefficient(-radicand)?;
        let constant = base.tower_value(&unity.domain, &[negative])?;
        let polynomial =
            base.polynomial(&unity.domain, &[(order, unity.one.clone()), (0, constant)])?;
        let (relation, selected, root) = base.root(&polynomial, &unity.map)?;
        unity.steps.push(unity.unit);
        let mut existence = base.adjoin(&relation, &selected, 3);
        existence.premises.push(1);
        unity.steps.push(existence);
        let domain = base.tower(&unity.domain, &polynomial)?;
        let generator = base.tower_value(&domain, &[unity.zero, unity.one])?;
        let embedded_unity = base.tower_value(&domain, &[unity.generator])?;
        let map = base.scalar_map(&domain, &[root])?;
        let radical = base.literal(&generator, &map)?;
        let primitive = base.literal(&embedded_unity, &map)?;
        let unknown = base.free("x")?;
        let (entries, product) = base.binomial_factors(order, &primitive, &radical, &unknown)?;
        let power = base.expression(Operator::IntegerPower(i64::from(order)), &[unknown], false)?;
        let constant = base.integer(radicand, false)?;
        let input = base.expression(Operator::Sub, &[power, constant], false)?;
        let zero = base.integer(0, false)?;
        let predicate = base.equation(&input, &zero)?;
        let problem = base.problem("x", &predicate, &[])?;
        let set = base.finite(&problem, &entries)?;
        let mut identity = base.identity(&input, &product, &map);
        identity.premises = vec![1, 4];
        unity.steps.push(identity);
        Self::cover(
            base,
            unity.steps,
            order,
            [&unity.selection, &selected, &problem, &set],
            &entries,
        )
    }

    /// Both general product completeness and the binomial specialization are replayed.
    fn cover(
        mut base: Fixture,
        mut steps: Vec<ApplicationDraft>,
        order: u32,
        objects: [&Handle; 4],
        entries: &[Handle],
    ) -> TestResult<Self> {
        use crate::test_api::solving::{
            BinomialMode, CertificateBinomialCoverage, CertificateProductCompleteness, PrimePower,
            RootFactor, SolveCertificate, SolveSchema, SolveStatement, StatementCoverage,
        };
        let [unity, selection, problem, set] = objects;
        let scalar = base.integer(1, false)?;
        let conclusion = SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: base.region.clone(),
        });
        let mut complete = base.step(
            SolveSchema::ProductCompleteness,
            conclusion.clone(),
            SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
                problem: problem.clone(),
                set: set.clone(),
                scalar,
                factors: entries
                    .iter()
                    .map(|root| RootFactor {
                        root: root.clone(),
                        multiplicity: 1,
                    })
                    .collect(),
                product: 0,
                existence: vec![1, 2],
                region: base.region.clone(),
            }),
        );
        complete.premises = vec![5, 1, 4];
        steps.push(complete);
        let mut binomial = base.step(
            SolveSchema::BinomialCoverage,
            conclusion,
            SolveCertificate::BinomialCoverage(CertificateBinomialCoverage {
                mode: BinomialMode::Coverage,
                order,
                unity: Some(unity.clone()),
                selection: Some(selection.clone()),
                factors: vec![PrimePower {
                    prime: order,
                    exponent: 1,
                }],
                exclusions: Vec::new(),
                product: Some(0),
            }),
        );
        binomial.premises = vec![5, 2, 1, 4, 6];
        steps.push(binomial);
        Ok(Self { base, steps })
    }
}

/// An ordinary nonzero constant is a complete empty solution set, not an unresolved request.
pub(crate) fn constant_cover() -> TestResult<(Fixture, Vec<ApplicationDraft>)> {
    use crate::test_api::solving::{
        CertificateProductCompleteness, SolveCertificate, SolveSchema, SolveStatement,
        StatementCoverage,
    };
    let mut base = Fixture::new()?;
    let constant = base.integer(3, false)?;
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&constant, &zero)?;
    let problem = base.problem("x", &predicate, &[])?;
    let set = base.finite(&problem, &[])?;
    let algebra = base.algebra.clone();
    let map = base.scalar_map(&algebra, &[])?;
    let identity = base.identity(&constant, &constant, &map);
    let mut coverage = base.step(
        SolveSchema::ProductCompleteness,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
            problem,
            set,
            scalar: constant,
            factors: Vec::new(),
            product: 0,
            existence: Vec::new(),
            region: base.region.clone(),
        }),
    );
    coverage.premises = vec![0];
    Ok((base, vec![identity, coverage]))
}

/// Constructive root refinement is universal exact arithmetic, with no numerical root sampling.
pub(crate) fn root_refinement(
    witness: i64,
    unique: bool,
) -> TestResult<(Fixture, ApplicationDraft)> {
    use crate::test_api::solving::{
        CertificateRootSelectionRefinement, RefinementMode, RefinementTargetKind, SolveCertificate,
        SolveSchema, SolveStatement, StatementRootExistence, StatementSelectionUnique,
    };
    let mut base = Fixture::new()?;
    let algebra = base.algebra.clone();
    let one = base.coefficient(1)?;
    let negative = base.coefficient(-2)?;
    let polynomial = base.polynomial(&algebra, &[(1, one), (0, negative)])?;
    let map = base.scalar_map(&algebra, &[])?;
    let (_, selection, _) = base.root(&polynomial, &map)?;
    let value = base.integer(witness, false)?;
    let statement = if unique {
        SolveStatement::SelectionUnique(StatementSelectionUnique {
            selection: selection.clone(),
            region: base.region.clone(),
        })
    } else {
        SolveStatement::RootExistence(StatementRootExistence {
            relation: selection.clone(),
            region: base.region.clone(),
        })
    };
    let step = base.step(
        SolveSchema::RootSelectionRefinement,
        statement,
        SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
            mode: if unique {
                RefinementMode::Unique
            } else {
                RefinementMode::Existence
            },
            target_kind: RefinementTargetKind::Selection,
            target: selection,
            witnesses: vec![value],
            compatibility: Vec::new(),
            form: None,
        }),
    );
    Ok((base, step))
}

impl BinomialFixture {
    /// The zero radicand has one value and retains the full positive factor multiplicity.
    pub(crate) fn zero(order: u32) -> TestResult<Self> {
        use crate::test_api::solving::{
            BinomialMode, CertificateBinomialCoverage, CertificateProductCompleteness, RootFactor,
            SolveCertificate, SolveSchema, SolveStatement, StatementCoverage,
        };
        let mut base = Fixture::in_field(&Field::Complex)?;
        let algebra = base.algebra.clone();
        let one = base.coefficient(1)?;
        let modulus = base.polynomial(&algebra, &[(1, one.clone())])?;
        let constant = base.polynomial(&algebra, &[(0, one.clone())])?;
        let empty = base.polynomial(&algebra, &[])?;
        let unit = base.unit(&one, &algebra, &modulus, &constant, &empty);
        let polynomial = base.polynomial(&algebra, &[(order, one)])?;
        let map = base.scalar_map(&algebra, &[])?;
        let (relation, selection, _) = base.root(&polynomial, &map)?;
        let existence = base.adjoin(&relation, &selection, 0);
        let unknown = base.free("x")?;
        let power = base.expression(Operator::IntegerPower(i64::from(order)), &[unknown], false)?;
        let zero = base.integer(0, false)?;
        let scalar = base.integer(1, false)?;
        let predicate = base.equation(&power, &zero)?;
        let problem = base.problem("x", &predicate, &[])?;
        let set = base.finite(&problem, std::slice::from_ref(&zero))?;
        let identity = base.identity(&power, &power, &map);
        let conclusion = SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: base.region.clone(),
        });
        let mut product = base.step(
            SolveSchema::ProductCompleteness,
            conclusion.clone(),
            SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
                problem,
                set,
                scalar,
                factors: vec![RootFactor {
                    root: zero,
                    multiplicity: order,
                }],
                product: 0,
                existence: Vec::new(),
                region: base.region.clone(),
            }),
        );
        product.premises = vec![2];
        let mut binomial = base.step(
            SolveSchema::BinomialCoverage,
            conclusion,
            SolveCertificate::BinomialCoverage(CertificateBinomialCoverage {
                mode: BinomialMode::ZeroRadicand,
                order,
                unity: None,
                selection: Some(selection),
                factors: Vec::new(),
                exclusions: Vec::new(),
                product: Some(2),
            }),
        );
        binomial.premises = vec![3, 1, 2];
        Ok(Self {
            base,
            steps: vec![unit, existence, identity, product, binomial],
        })
    }

    /// Transport the derived binomial back through a supplied translation, reconstructing y²+2y-3.
    pub(crate) fn translated() -> TestResult<Self> {
        use crate::test_api::solving::{
            CertificateSolutionTransport, SolveCertificate, SolveSchema, SolveStatement,
            StatementCoverage,
        };
        let mut fixture = Self::new(2, 4)?;
        let Statement::Solve(statement) =
            &fixture.steps.last().ok_or("missing coverage")?.conclusion
        else {
            return Err("wrong statement".into());
        };
        let SolveStatement::Coverage(coverage) = statement.as_ref() else {
            return Err("missing binomial cover".into());
        };
        let target = coverage.problem.clone();
        let old_set = coverage.set.clone();
        let y = fixture.base.free("y")?;
        let squared =
            fixture
                .base
                .expression(Operator::IntegerPower(2), std::slice::from_ref(&y), false)?;
        let two = fixture.base.integer(2, false)?;
        let linear = fixture.base.expression(Operator::Mul, &[two, y], false)?;
        let sum = fixture
            .base
            .expression(Operator::Add, &[squared, linear], false)?;
        let three = fixture.base.integer(3, false)?;
        let input = fixture
            .base
            .expression(Operator::Sub, &[sum, three], false)?;
        let zero = fixture.base.integer(0, false)?;
        let predicate = fixture.base.equation(&input, &zero)?;
        let source = fixture.base.problem("y", &predicate, &[])?;
        let forward = fixture.base.affine(&target, 1)?;
        let backward = fixture.base.affine(&source, -1)?;
        let entries = finite_entries(&fixture.base.cas, &old_set)?;
        let one = fixture.base.integer(1, false)?;
        let mut translated = Vec::new();
        for entry in &entries {
            translated.push(fixture.base.expression(
                Operator::Sub,
                &[entry.clone(), one.clone()],
                false,
            )?);
        }
        let set = fixture.base.finite(&source, &translated)?;
        let mut step = fixture.base.step(
            SolveSchema::SolutionTransport,
            SolveStatement::Coverage(StatementCoverage {
                problem: source.clone(),
                set,
                region: fixture.base.region.clone(),
            }),
            SolveCertificate::SolutionTransport(CertificateSolutionTransport {
                source,
                target,
                forward,
                backward,
                region: fixture.base.region.clone(),
                definedness: Vec::new(),
                equations: Vec::new(),
                forward_inverse: Vec::new(),
                backward_inverse: Vec::new(),
            }),
        );
        step.premises = vec![7, 1, 4];
        fixture.steps.push(step);
        Ok(fixture)
    }
}

/// Public read-only views expose the exact values of a finite proposal.
fn finite_entries(cas: &Cas, set: &Handle) -> TestResult<Vec<Handle>> {
    let SolveView::SolutionSet(set) = cas.solve_view(set)? else {
        return Err("missing set".into());
    };
    let mut entries = Vec::new();
    for branch in &set.branches {
        let SolveView::SolutionBranch(branch) = cas.solve_view(branch)? else {
            return Err("missing branch".into());
        };
        let crate::test_api::solving::SolutionBranchKind::Finite { tuples } = &branch.kind else {
            return Err("wrong branch".into());
        };
        for tuple in tuples {
            let SolveView::Tuple(tuple) = cas.solve_view(tuple)? else {
                return Err("missing tuple".into());
            };
            entries.push(tuple.entries.first().ok_or("missing value")?.clone());
        }
    }
    Ok(entries)
}
