// SPDX-License-Identifier: AGPL-3.0-only
//! Public elementary proof fixtures with independent, explicitly supplied mathematics.

#[path = "solve_elementary_restriction_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "public test fixtures stay inside the test target"
)]
mod restriction;
pub(crate) use restriction::{
    assumed_restriction, degree_restriction, mixed_restriction, symbolic_restriction,
};

#[path = "solve_elementary_pivots_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "public test fixtures stay inside the test target"
)]
mod pivots;
pub(crate) use pivots::{
    embedded_fraction, embedded_tower_one, inverse_pivot, inverse_pivot_mutation,
    inverse_pivot_scan,
};

#[path = "solve_elementary_presentations_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "public test fixtures stay inside the test target"
)]
mod presentations;
pub(crate) use presentations::{
    closed_presentation_mutation, closed_presentations, presentation_closure,
};

#[path = "solve_elementary_realness_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "public test fixtures stay inside the test target"
)]
mod realness;
pub(crate) use realness::{
    choice_wrong_ambient, cubic_product_reference, fraction_realness, linked_cubic,
    linked_cubic_restriction, partial_power_realness, referenced_realness,
};

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
        self.affine_named(target, offset, "t")
    }

    /// A distinct family coordinate product remains distinct even with the same scalar field.
    pub(crate) fn affine_named(
        &mut self,
        target: &Handle,
        offset: i64,
        key: &str,
    ) -> TestResult<Handle> {
        let SolveView::SolveProblem(problem) = self.cas.solve_view(target)? else {
            return Err("missing target".into());
        };
        let target = problem.unknowns.clone();
        let family = coordinate_product(
            &mut self.cas,
            &self.header,
            &[key],
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

impl Fixture {
    /// A reverse map binds the exact unknown product and returns the original family product.
    pub(crate) fn recovery(&mut self, family: &Handle, offset: i64) -> TestResult<Handle> {
        let SolveView::FamilyMap(map) = self.cas.solve_view(family)? else {
            return Err("missing family".into());
        };
        let source = map.target.clone();
        let target = map.family.clone();
        let scope = vec![vec![AtomicSort::Scalar(self.field.clone())]];
        let region = product(&mut self.cas, &self.header, &source, scope.clone())?;
        let bound = self.expression(Operator::Bound { depth: 0, slot: 0 }, &[], true)?;
        let constant = self.integer(offset, true)?;
        let value = self.expression(Operator::Add, &[bound, constant], true)?;
        let body = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::Tuple(TupleData {
                header: Reference::Existing(self.header.clone()),
                scope,
                coordinates: Reference::Existing(target.clone()),
                entries: vec![Reference::Existing(value)],
            })),
        )?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::FamilyMap(FamilyMapData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                coefficients: Reference::Existing(self.coefficients.clone()),
                family: Reference::Existing(source),
                region: Reference::Existing(region),
                target: Reference::Existing(target),
                body: Reference::Existing(body),
            })),
        )
    }

    /// An image branch records a proposal, without conferring membership or coverage.
    pub(crate) fn image(&mut self, problem: &Handle, family: &Handle) -> TestResult<Handle> {
        use crate::test_api::solving::{SolutionBranchData, SolutionBranchKind, SolutionSetData};
        let SolveView::SolveProblem(original) = self.cas.solve_view(problem)? else {
            return Err("missing problem".into());
        };
        let domain = original.domain.clone();
        let branch = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::SolutionBranch(SolutionBranchData {
                problem: Reference::Existing(problem.clone()),
                region: Reference::Existing(self.region.clone()),
                domain: Reference::Existing(domain),
                kind: SolutionBranchKind::Image {
                    family: Reference::Existing(family.clone()),
                },
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

    /// An explicit matrix has its own exact row-major scalar sort.
    pub(crate) fn matrix(
        &mut self,
        rows: u32,
        columns: u32,
        entries: &[Handle],
    ) -> TestResult<Handle> {
        use crate::test_api::syntax::Extent;
        admit(
            &mut self.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                operator: Operator::Matrix { rows, columns },
                sort: Sort::Value(AtomicSort::Matrix {
                    field: self.field.clone(),
                    rows: Extent::Literal(rows),
                    columns: Extent::Literal(columns),
                }),
                operands: entries.iter().cloned().map(Reference::Existing).collect(),
                space: None,
                binding: None,
            }),
        )
    }
}

/// Identity family for the zero polynomial, with independent universal membership and recovery.
pub(crate) fn identity_family(offset: i64) -> TestResult<(Fixture, Vec<ApplicationDraft>)> {
    use crate::test_api::solving::{
        CertificateParameterImageCoverage, SolveCertificate, SolveSchema, SolveStatement,
        StatementCoverage, StatementFamilyCoverage, StatementFamilyMembership,
    };
    let mut base = Fixture::new()?;
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&zero, &zero)?;
    let problem = base.problem("x", &predicate, &[])?;
    let family = base.affine(&problem, 0)?;
    let recovery = base.recovery(&family, offset)?;
    let set = base.image(&problem, &family)?;
    let mut certificate = CertificateParameterImageCoverage {
        problem: problem.clone(),
        family: family.clone(),
        recovery: None,
        region: base.region.clone(),
        membership: Vec::new(),
        recovery_proofs: Vec::new(),
        domain: Vec::new(),
        scope: Vec::new(),
    };
    let member = base.step(
        SolveSchema::ParameterImageCoverage,
        SolveStatement::FamilyMembership(StatementFamilyMembership {
            problem: problem.clone(),
            family: family.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::ParameterImageCoverage(certificate.clone()),
    );
    certificate.recovery = Some(recovery.clone());
    certificate.membership = vec![0];
    let mut covered = base.step(
        SolveSchema::ParameterImageCoverage,
        SolveStatement::FamilyCoverage(StatementFamilyCoverage {
            problem: problem.clone(),
            family,
            recovery,
            region: base.region.clone(),
        }),
        SolveCertificate::ParameterImageCoverage(certificate.clone()),
    );
    covered.premises = vec![0];
    let mut complete = base.step(
        SolveSchema::ParameterImageCoverage,
        SolveStatement::Coverage(StatementCoverage {
            problem,
            set,
            region: base.region.clone(),
        }),
        SolveCertificate::ParameterImageCoverage(certificate),
    );
    complete.premises = vec![0, 1];
    Ok((base, vec![member, covered, complete]))
}

/// Build a map draft even when source/target roles intentionally violate admission.
pub(crate) fn role_map(
    source: &[CoordinateRole],
    target: &[CoordinateRole],
) -> TestResult<(Fixture, Draft)> {
    let mut base = Fixture::new()?;
    let spaces = [source, target]
        .into_iter()
        .zip([["t", "y"], ["x", "y"]])
        .map(|(roles, keys)| CoordinateSpaceData {
            header: Reference::Existing(base.header.clone()),
            coordinates: roles
                .iter()
                .zip(keys)
                .map(|(role, key)| Coordinate {
                    key: key.into(),
                    field: Field::Real,
                    role: *role,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let input = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(
            spaces.first().ok_or("missing source")?.clone(),
        )),
    )?;
    let output = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(
            spaces.get(1).ok_or("missing target")?.clone(),
        )),
    )?;
    let scope = vec![vec![AtomicSort::Scalar(Field::Real); source.len()]];
    let region = product(&mut base.cas, &base.header, &input, scope.clone())?;
    let expression = admit(
        &mut base.cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: scope.clone(),
            operator: Operator::Bound { depth: 0, slot: 0 },
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            operands: Vec::new(),
            space: None,
            binding: None,
        }),
    )?;
    let body = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(base.header.clone()),
            scope,
            coordinates: Reference::Existing(output.clone()),
            entries: vec![Reference::Existing(expression); target.len()],
        })),
    )?;
    let draft = Draft::Solve(SolveDraft::FamilyMap(FamilyMapData {
        header: Reference::Existing(base.header.clone()),
        scope: Vec::new(),
        coefficients: Reference::Existing(base.coefficients.clone()),
        family: Reference::Existing(input),
        region: Reference::Existing(region),
        target: Reference::Existing(output),
        body: Reference::Existing(body),
    }));
    Ok((base, draft))
}

/// The two-character transform has an exact inverse and a nonzero triangular minor.
pub(crate) fn character_rank() -> TestResult<(Fixture, ApplicationDraft)> {
    use crate::test_api::solving::{
        CertificateExactLinearEquivalence, RowOperation, SolveCertificate, SolveSchema,
        SolveStatement, StatementRank,
    };
    let mut base = Fixture::new()?;
    let one = base.integer(1, false)?;
    let negative = base.integer(-1, false)?;
    let zero = base.integer(0, false)?;
    let minor = base.integer(-2, false)?;
    let matrix = base.matrix(
        2,
        2,
        &[one.clone(), one.clone(), one.clone(), negative.clone()],
    )?;
    let reduced = base.matrix(2, 2, &[one.clone(), one, zero, minor.clone()])?;
    let claim = SolveStatement::Rank(StatementRank {
        matrix: matrix.clone(),
        domain: base.algebra.clone(),
        rank: 2,
        pivots: vec![0, 1],
        minor: minor.clone(),
        elimination: reduced.clone(),
    });
    let certificate = SolveCertificate::ExactLinearEquivalence(CertificateExactLinearEquivalence {
        matrix,
        augmented_columns: 0,
        operations: vec![RowOperation::AddMultiple {
            destination: 1,
            source: 0,
            value: negative,
        }],
        pivots: vec![0, 1],
        minor,
        reduced,
        region: base.region.clone(),
    });
    let step = base.step(SolveSchema::ExactLinearEquivalence, claim, certificate);
    Ok((base, step))
}

/// Conjugation fixes z+conjugate(z), while an unconstrained complex z need not be real.
pub(crate) fn conjugate_real(symmetric: bool) -> TestResult<(Fixture, ApplicationDraft)> {
    use crate::test_api::solving::{
        CertificateConjugationTransport, SolveCertificate, SolveSchema, SolveStatement,
        StatementRealValues,
    };
    let mut base = Fixture::in_field(&Field::Complex)?;
    let value = base.free("a")?;
    let conjugate = base.expression(Operator::Conjugate, std::slice::from_ref(&value), false)?;
    let result = if symmetric {
        base.expression(Operator::Add, &[value, conjugate], false)?
    } else {
        value
    };
    let algebra = base.algebra.clone();
    let map = base.scalar_map(&algebra, &[])?;
    let step = base.step(
        SolveSchema::ConjugationTransport,
        SolveStatement::RealValues(StatementRealValues {
            values: vec![result.clone()],
            region: base.region.clone(),
        }),
        SolveCertificate::ConjugationTransport(CertificateConjugationTransport {
            map: map.clone(),
            conjugate_map: map,
            images: Vec::new(),
            values: vec![result],
            relations: Vec::new(),
            fixed_field: Vec::new(),
            region: base.region.clone(),
        }),
    );
    Ok((base, step))
}

impl BinomialFixture {
    /// A presentation inspects every nested binomial and root-of-unity dependency.
    pub(crate) fn presentation() -> TestResult<Self> {
        use crate::test_api::solving::{
            CertificateRootSelectionRefinement, RefinementMode, RefinementTargetKind,
            SolveCertificate, SolveForm, SolveSchema, SolveStatement, StatementPresentation,
        };
        let mut fixture = Self::new(2, 2)?;
        let Statement::Solve(statement) =
            &fixture.steps.last().ok_or("missing coverage")?.conclusion
        else {
            return Err("wrong conclusion".into());
        };
        let SolveStatement::Coverage(coverage) = statement.as_ref() else {
            return Err("wrong coverage".into());
        };
        let set = coverage.set.clone();
        let mut step = fixture.base.step(
            SolveSchema::RootSelectionRefinement,
            SolveStatement::Presentation(StatementPresentation {
                set: set.clone(),
                form: SolveForm::Radicals,
                region: fixture.base.region.clone(),
            }),
            SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
                mode: RefinementMode::Presentation,
                target_kind: RefinementTargetKind::SolutionSet,
                target: set,
                witnesses: Vec::new(),
                compatibility: Vec::new(),
                form: Some(SolveForm::Radicals),
            }),
        );
        step.premises = vec![7, 1, 4, 2];
        fixture.steps.push(step);
        Ok(fixture)
    }

    /// Derivatives of x² at the exact selected zero certify multiplicity two independently of set syntax.
    pub(crate) fn multiplicity() -> TestResult<Self> {
        use crate::test_api::solving::{
            CertificateMultiplicityTransfer, CertificateRootSelectionRefinement, MultiplicityMode,
            RefinementMode, RefinementTargetKind, SolveCertificate, SolveSchema, SolveStatement,
            StatementMultiplicity, StatementRootExistence,
        };
        let mut fixture = Self::zero(2)?;
        let Statement::Solve(statement) =
            &fixture.steps.last().ok_or("missing coverage")?.conclusion
        else {
            return Err("wrong conclusion".into());
        };
        let SolveStatement::Coverage(coverage) = statement.as_ref() else {
            return Err("wrong coverage".into());
        };
        let problem = coverage.problem.clone();
        let algebra = fixture.base.algebra.clone();
        let one = fixture.base.coefficient(1)?;
        let polynomial = fixture.base.polynomial(&algebra, &[(1, one)])?;
        let map = fixture.base.scalar_map(&algebra, &[])?;
        let (_, selection, _) = fixture.base.root(&polynomial, &map)?;
        let zero = fixture.base.integer(0, false)?;
        let two = fixture.base.integer(2, false)?;
        let existence = fixture.base.step(
            SolveSchema::RootSelectionRefinement,
            SolveStatement::RootExistence(StatementRootExistence {
                relation: selection.clone(),
                region: fixture.base.region.clone(),
            }),
            SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
                mode: RefinementMode::Existence,
                target_kind: RefinementTargetKind::Selection,
                target: selection.clone(),
                witnesses: vec![zero.clone()],
                compatibility: Vec::new(),
                form: None,
            }),
        );
        fixture.steps.push(existence);
        let mut step = fixture.base.step(
            SolveSchema::MultiplicityTransfer,
            SolveStatement::Multiplicity(StatementMultiplicity {
                problem: problem.clone(),
                selection: selection.clone(),
                multiplicities: vec![2],
                region: fixture.base.region.clone(),
            }),
            SolveCertificate::MultiplicityTransfer(CertificateMultiplicityTransfer {
                mode: MultiplicityMode::Derivatives,
                problem,
                selection,
                multiplicities: vec![2],
                values: vec![zero.clone(), zero, two],
                product: 0,
                nonzero: Vec::new(),
                region: fixture.base.region.clone(),
            }),
        );
        step.premises = vec![4, 5];
        fixture.steps.push(step);
        Ok(fixture)
    }
}

impl Fixture {
    /// Build a distinct real header in this store; source theorem contexts remain distinct.
    fn enter_real(&mut self) -> TestResult {
        self.field = Field::Real;
        let declarations = ["a", "b", "c", "t", "x", "y"]
            .into_iter()
            .map(|key| Declaration {
                key: key.into(),
                display_name: key.into(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            })
            .collect();
        self.header = admit(
            &mut self.cas,
            &Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                branch: Branch::StrictReal,
                declarations,
            })),
        )?;
        self.coefficients = coordinate_product(
            &mut self.cas,
            &self.header,
            &["a", "b", "c"],
            CoordinateRole::Coefficient,
            &self.field,
        )?;
        self.region = product(&mut self.cas, &self.header, &self.coefficients, Vec::new())?;
        self.context = admit(
            &mut self.cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(self.header.clone()),
                admissible: Reference::Existing(self.region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        Ok(())
    }

    /// Exact ground quadratic x²-r is reconstructed in the current explicit ambient.
    fn quadratic(&mut self, radicand: i64) -> TestResult<(Handle, Handle)> {
        let unknown = self.free("x")?;
        let squared = self.expression(Operator::IntegerPower(2), &[unknown], false)?;
        let constant = self.integer(radicand, false)?;
        let polynomial = self.expression(Operator::Sub, &[squared, constant], false)?;
        let zero = self.integer(0, false)?;
        let predicate = self.equation(&polynomial, &zero)?;
        Ok((self.problem("x", &predicate, &[])?, polynomial))
    }
}

/// Exact complete complex coverage of x²-1, with independently supplied product reconstruction.
fn ground_complex_cover() -> TestResult<BinomialFixture> {
    use crate::test_api::solving::{
        CertificateProductCompleteness, RootFactor, SolveCertificate, SolveSchema, SolveStatement,
        StatementCoverage,
    };
    let mut base = Fixture::in_field(&Field::Complex)?;
    let (problem, polynomial) = base.quadratic(1)?;
    let positive = base.integer(1, false)?;
    let negative = base.integer(-1, false)?;
    let unknown = base.free("x")?;
    let first = base.expression(Operator::Sub, &[unknown.clone(), positive.clone()], false)?;
    let second = base.expression(Operator::Sub, &[unknown, negative.clone()], false)?;
    let product = base.expression(Operator::Mul, &[first, second], false)?;
    let algebra = base.algebra.clone();
    let map = base.scalar_map(&algebra, &[])?;
    let identity = base.identity(&polynomial, &product, &map);
    let set = base.finite(&problem, &[positive.clone(), negative.clone()])?;
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
            scalar: positive.clone(),
            factors: vec![
                RootFactor {
                    root: positive,
                    multiplicity: 1,
                },
                RootFactor {
                    root: negative,
                    multiplicity: 1,
                },
            ],
            product: 0,
            existence: Vec::new(),
            region: base.region.clone(),
        }),
    );
    coverage.premises = vec![0];
    Ok(BinomialFixture {
        base,
        steps: vec![identity, coverage],
    })
}

/// Real restriction explicitly rebuilds the ground problem under a separate header.
pub(crate) fn real_restriction(nonreal: bool) -> TestResult<BinomialFixture> {
    use crate::test_api::solving::{
        CertificateRealRestriction, SolveCertificate, SolveSchema, SolveStatement,
        StatementCoverage,
    };
    let mut fixture = if nonreal {
        BinomialFixture::new(2, -1)?
    } else {
        ground_complex_cover()?
    };
    let covered_index = u32::try_from(fixture.steps.len().checked_sub(1).ok_or("no coverage")?)?;
    let Statement::Solve(statement) = &fixture.steps.last().ok_or("missing coverage")?.conclusion
    else {
        return Err("wrong statement".into());
    };
    let SolveStatement::Coverage(coverage) = statement.as_ref() else {
        return Err("wrong coverage".into());
    };
    let SolveView::SolutionSet(set) = fixture.base.cas.solve_view(&coverage.set)? else {
        return Err("missing set".into());
    };
    let branches = set.branches.clone();
    let complex_problem = coverage.problem.clone();
    let zero = fixture.base.integer(0, false)?;
    let partition = fixture.base.split(&zero)?;
    let partition_index = u32::try_from(fixture.steps.len())?;
    fixture.steps.push(partition.clone());
    fixture.base.enter_real()?;
    let (problem, _) = fixture.base.quadratic(if nonreal { -1 } else { 1 })?;
    let values = if nonreal {
        Vec::new()
    } else {
        vec![
            fixture.base.integer(1, false)?,
            fixture.base.integer(-1, false)?,
        ]
    };
    let set = fixture.base.finite(&problem, &values)?;
    let zero = fixture.base.integer(0, false)?;
    let real_cover = fixture.base.split(&zero)?;
    let real_cover_index = u32::try_from(fixture.steps.len())?;
    let transport = restriction::transport(
        &mut fixture.base,
        &problem,
        &complex_problem,
        &real_cover,
        &partition,
    )?;
    fixture.steps.push(real_cover);
    let mut step = fixture.base.step(
        SolveSchema::RealRestriction,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: fixture.base.region.clone(),
        }),
        SolveCertificate::RealRestriction(CertificateRealRestriction {
            coverage: 0,
            problem,
            set,
            transport,
            retained: if nonreal {
                Vec::new()
            } else {
                branches.clone()
            },
            rejected: if nonreal { branches } else { Vec::new() },
            classification: Vec::new(),
            cover: 1,
        }),
    );
    step.premises = vec![covered_index, partition_index, real_cover_index];
    if nonreal {
        step.premises.extend([1, 4]);
    }
    fixture.steps.push(step);
    Ok(fixture)
}

/// One regional branch of a*x=0 uses either the complete identity family or the singleton zero.
fn partition_branch(
    base: &mut Fixture,
    problem: &Handle,
    polynomial: &Handle,
    zero_side: bool,
) -> TestResult<Vec<ApplicationDraft>> {
    use crate::test_api::solving::{
        CertificateParameterImageCoverage, CertificateProductCompleteness, RootFactor,
        SolveCertificate, SolveSchema, SolveStatement, StatementCoverage,
    };
    if zero_side {
        let family = base.affine(problem, 0)?;
        let recovery = base.recovery(&family, 0)?;
        let set = base.image(problem, &family)?;
        let step = base.step(
            SolveSchema::ParameterImageCoverage,
            SolveStatement::Coverage(StatementCoverage {
                problem: problem.clone(),
                set,
                region: base.region.clone(),
            }),
            SolveCertificate::ParameterImageCoverage(CertificateParameterImageCoverage {
                problem: problem.clone(),
                family,
                recovery: Some(recovery),
                region: base.region.clone(),
                membership: Vec::new(),
                recovery_proofs: Vec::new(),
                domain: Vec::new(),
                scope: Vec::new(),
            }),
        );
        return Ok(vec![step]);
    }
    let zero = base.integer(0, false)?;
    let scalar = base.free("a")?;
    let set = base.finite(problem, std::slice::from_ref(&zero))?;
    let algebra = base.algebra.clone();
    let map = base.scalar_map(&algebra, &[])?;
    let identity = base.identity(polynomial, polynomial, &map);
    let mut step = base.step(
        SolveSchema::ProductCompleteness,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
            problem: problem.clone(),
            set,
            scalar,
            factors: vec![RootFactor {
                root: zero,
                multiplicity: 1,
            }],
            product: 0,
            existence: Vec::new(),
            region: base.region.clone(),
        }),
    );
    step.premises = vec![0];
    Ok(vec![identity, step])
}

/// A child proof closes only its own frame and keeps the exact meet region in the conclusion.
fn close_partition_branch(
    base: &mut Fixture,
    problem: &Handle,
    polynomial: &Handle,
    child: &Handle,
    zero_side: bool,
    steps: &mut Vec<ApplicationDraft>,
) -> TestResult<(Handle, u32)> {
    let parent = base.context.clone();
    let domain = base.region.clone();
    let region = admit(
        &mut base.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: base.coefficients.clone(),
            },
            expression: RegionExpr::Meet(vec![
                Reference::Existing(domain.clone()),
                Reference::Existing(child.clone()),
            ]),
        }),
    )?;
    let frame = admit(
        &mut base.cas,
        &Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(parent.clone()),
            region: Reference::Existing(child.clone()),
            hypotheses: Vec::new(),
        }),
    )?;
    base.context.clone_from(&frame);
    base.region.clone_from(&region);
    let mut branch = partition_branch(base, problem, polynomial, zero_side)?;
    let start = u32::try_from(steps.len())?;
    for step in &mut branch {
        for previous in &mut step.premises {
            *previous = previous.checked_add(start).ok_or("proof index overflow")?;
        }
    }
    let conclusion = branch.last().ok_or("missing branch")?.conclusion.clone();
    let Statement::Solve(statement) = &conclusion else {
        return Err("wrong branch statement".into());
    };
    let crate::test_api::solving::SolveStatement::Coverage(coverage) = statement.as_ref() else {
        return Err("missing branch coverage".into());
    };
    let SolveView::SolutionSet(set) = base.cas.solve_view(&coverage.set)? else {
        return Err("missing branch set".into());
    };
    let handle = set.branches.first().ok_or("missing branch")?.clone();
    steps.extend(branch);
    let covered = u32::try_from(steps.len().checked_sub(1).ok_or("missing proof")?)?;
    base.context = parent;
    base.region = domain;
    let construction = u32::try_from(steps.len())?;
    steps.push(ApplicationDraft {
        rule: Primitive::RegionConstruct,
        context: base.context.clone(),
        conclusion: Statement::Constructor {
            region,
            law: crate::test_api::proof::ConstructorLaw::Membership,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    let closed = u32::try_from(steps.len())?;
    steps.push(ApplicationDraft {
        rule: Primitive::ContextClose,
        context: base.context.clone(),
        conclusion,
        premises: vec![covered, construction],
        substitutions: Vec::new(),
        certificate: Certificate::Close { frame },
    });
    Ok((handle, closed))
}

/// Degree/pivot zero keeps the full family while its complement keeps exactly the zero singleton.
pub(crate) fn degree_drop_cover() -> TestResult<BinomialFixture> {
    degree_drop_into(Fixture::new()?)
}

/// Build the same guarded cover in an existing store and either scalar ambient.
fn degree_drop_into(mut base: Fixture) -> TestResult<BinomialFixture> {
    use crate::test_api::solving::{
        CertificateTotalPredicateSplit, PredicateSplitMode, SolutionSetData, SolveCertificate,
        SolveSchema, SolveStatement, StatementCoverage,
    };
    let coefficient = base.free("a")?;
    let unknown = base.free("x")?;
    let polynomial = base.expression(Operator::Mul, &[coefficient.clone(), unknown], false)?;
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&polynomial, &zero)?;
    let problem = base.problem("x", &predicate, &[])?;
    let partition = base.split(&coefficient)?;
    let Statement::Cover { members, .. } = &partition.conclusion else {
        return Err("missing partition".into());
    };
    let children = members.clone();
    let mut steps = vec![partition];
    let mut branches = Vec::new();
    let mut proofs = vec![0];
    for (index, child) in children.iter().enumerate() {
        let (branch, proof) = close_partition_branch(
            &mut base,
            &problem,
            &polynomial,
            child,
            index == 0,
            &mut steps,
        )?;
        branches.push(Reference::Existing(branch));
        proofs.push(proof);
    }
    let set = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::SolutionSet(SolutionSetData {
            problem: Reference::Existing(problem.clone()),
            branches,
        })),
    )?;
    let mut step = base.step(
        SolveSchema::TotalPredicateSplit,
        SolveStatement::Coverage(StatementCoverage {
            problem,
            set,
            region: base.region.clone(),
        }),
        SolveCertificate::TotalPredicateSplit(CertificateTotalPredicateSplit {
            mode: PredicateSplitMode::EliminateCover,
            expression: coefficient,
            region: base.region.clone(),
            children,
            branches: vec![1, 2],
            cover: Some(0),
        }),
    );
    step.premises = proofs;
    steps.push(step);
    Ok(BinomialFixture { base, steps })
}

/// An affine system retains both original conjuncts, including a contradictory augmented row.
pub(crate) fn linear_system(inconsistent: bool) -> TestResult<(Fixture, ApplicationDraft)> {
    use crate::test_api::solving::{
        CertificateExactLinearEquivalence, RowOperation, SolveCertificate, SolveSchema,
        SolveStatement, StatementLinearSystem,
    };
    let mut base = Fixture::new()?;
    let unknown = base.free("x")?;
    let one = base.integer(1, false)?;
    let two = base.integer(2, false)?;
    let right = base.integer(if inconsistent { 3 } else { 2 }, false)?;
    let twice = base.expression(Operator::Mul, &[two.clone(), unknown.clone()], false)?;
    let equation = |base: &mut Fixture, children: &[Handle]| {
        admit(
            &mut base.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(base.header.clone()),
                scope: Vec::new(),
                operator: Operator::Predicate(PredicateKind::Equal),
                sort: Sort::Value(AtomicSort::Boolean),
                operands: children.iter().cloned().map(Reference::Existing).collect(),
                space: None,
                binding: None,
            }),
        )
    };
    let first = equation(&mut base, &[unknown, one.clone()])?;
    let second = equation(&mut base, &[twice, right.clone()])?;
    let predicate = admit(
        &mut base.cas,
        &Draft::Condition {
            header: Reference::Existing(base.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::And,
            arguments: vec![Reference::Existing(first), Reference::Existing(second)],
        },
    )?;
    let problem = base.problem("x", &predicate, &[])?;
    let zero = base.integer(0, false)?;
    let negative = base.integer(-2, false)?;
    let bottom = if inconsistent {
        one.clone()
    } else {
        zero.clone()
    };
    let matrix = base.matrix(2, 2, &[one.clone(), one.clone(), two, right])?;
    let reduced = base.matrix(2, 2, &[one.clone(), one.clone(), zero, bottom])?;
    let pivots = if inconsistent { vec![0, 1] } else { vec![0] };
    let claim = SolveStatement::LinearSystem(StatementLinearSystem {
        problem,
        reduced: reduced.clone(),
        rank: 1,
        augmented_rank: if inconsistent { 2 } else { 1 },
        region: base.region.clone(),
        free_columns: Vec::new(),
    });
    let step = base.step(
        SolveSchema::ExactLinearEquivalence,
        claim,
        SolveCertificate::ExactLinearEquivalence(CertificateExactLinearEquivalence {
            matrix,
            augmented_columns: 1,
            operations: vec![RowOperation::AddMultiple {
                destination: 1,
                source: 0,
                value: negative,
            }],
            pivots,
            minor: one,
            reduced,
            region: base.region.clone(),
        }),
    );
    Ok((base, step))
}

/// A constant map is universally a member of the zero equation but covers only one point.
pub(crate) fn sampled_family() -> TestResult<(Fixture, Vec<ApplicationDraft>)> {
    use crate::test_api::solving::{SolveCertificate, SolveStatement};
    let (mut base, mut steps) = identity_family(0)?;
    steps.pop();
    let Statement::Solve(statement) = &steps.first().ok_or("missing membership")?.conclusion else {
        return Err("wrong statement".into());
    };
    let SolveStatement::FamilyMembership(member) = statement.as_ref() else {
        return Err("wrong membership".into());
    };
    let SolveView::FamilyMap(map) = base.cas.solve_view(&member.family)? else {
        return Err("missing map".into());
    };
    let map = map.clone();
    let zero = base.integer(0, true)?;
    let body = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(base.header.clone()),
            scope: vec![vec![AtomicSort::Scalar(base.field.clone())]],
            coordinates: Reference::Existing(map.target.clone()),
            entries: vec![Reference::Existing(zero)],
        })),
    )?;
    let constant = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::FamilyMap(FamilyMapData {
            header: Reference::Existing(map.header),
            scope: map.scope,
            coefficients: Reference::Existing(map.coefficients),
            family: Reference::Existing(map.family),
            region: Reference::Existing(map.region),
            target: Reference::Existing(map.target),
            body: Reference::Existing(body),
        })),
    )?;
    for step in &mut steps {
        let Statement::Solve(statement) = &mut step.conclusion else {
            return Err("wrong statement".into());
        };
        match statement.as_mut() {
            SolveStatement::FamilyMembership(value) => value.family.clone_from(&constant),
            SolveStatement::FamilyCoverage(value) => value.family.clone_from(&constant),
            _ => return Err("wrong family statement".into()),
        }
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::ParameterImageCoverage(value) = certificate.as_mut() else {
            return Err("wrong family certificate".into());
        };
        value.family.clone_from(&constant);
    }
    Ok((base, steps))
}

impl Fixture {
    /// A ground leading-unit certificate is independent of the requested root polynomial.
    fn ground_unit(&mut self) -> TestResult<ApplicationDraft> {
        let algebra = self.algebra.clone();
        let one = self.coefficient(1)?;
        let modulus = self.polynomial(&algebra, &[(1, one.clone())])?;
        let constant = self.polynomial(&algebra, &[(0, one.clone())])?;
        let empty = self.polynomial(&algebra, &[])?;
        Ok(self.unit(&one, &algebra, &modulus, &constant, &empty))
    }
}

/// General algebraic roots remain exact, but their existence does not establish a radical presentation.
pub(crate) fn general_presentation(radicals: bool) -> TestResult<BinomialFixture> {
    use crate::test_api::solving::{
        CertificateRootSelectionRefinement, RefinementMode, RefinementTargetKind, SolveCertificate,
        SolveForm, SolveSchema, SolveStatement, StatementPresentation,
    };
    let mut base = Fixture::in_field(&Field::Complex)?;
    let unit = base.ground_unit()?;
    let algebra = base.algebra.clone();
    let one = base.coefficient(1)?;
    let polynomial = base.polynomial(&algebra, &[(3, one.clone()), (1, one.clone()), (0, one)])?;
    let map = base.scalar_map(&algebra, &[])?;
    let (relation, selection, root) = base.root(&polynomial, &map)?;
    let existence = base.adjoin(&relation, &selection, 0);
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&zero, &zero)?;
    let problem = base.problem("x", &predicate, &[])?;
    let set = base.finite(&problem, &[root])?;
    let form = if radicals {
        SolveForm::Radicals
    } else {
        SolveForm::AlgebraicRoots
    };
    let mut step = base.step(
        SolveSchema::RootSelectionRefinement,
        SolveStatement::Presentation(StatementPresentation {
            set: set.clone(),
            form,
            region: base.region.clone(),
        }),
        SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
            mode: RefinementMode::Presentation,
            target_kind: RefinementTargetKind::SolutionSet,
            target: set,
            witnesses: Vec::new(),
            compatibility: Vec::new(),
            form: Some(form),
        }),
    );
    step.premises = vec![1];
    Ok(BinomialFixture {
        base,
        steps: vec![unit, existence, step],
    })
}

/// Two separately selected cube roots satisfy the same polynomial without an equality theorem.
pub(crate) fn independent_cube_roots(independent: bool) -> TestResult<BinomialFixture> {
    use crate::test_api::solving::{
        CertificateConjugationTransport, SolveCertificate, SolveSchema, SolveStatement,
        StatementCoefficientEquality,
    };
    let mut base = Fixture::in_field(&Field::Complex)?;
    let unit = base.ground_unit()?;
    let algebra = base.algebra.clone();
    let one = base.coefficient(1)?;
    let negative = base.coefficient(-2)?;
    let polynomial = base.polynomial(&algebra, &[(3, one), (0, negative)])?;
    let map = base.scalar_map(&algebra, &[])?;
    let (relation, selection, left) = base.root(&polynomial, &map)?;
    let first = base.adjoin(&relation, &selection, 0);
    let (selection, right) = base.independent_root(&relation)?;
    let mut second = base.adjoin(&relation, &selection, 0);
    second.premises.push(1);
    let mut step = base.step(
        SolveSchema::ConjugationTransport,
        SolveStatement::CoefficientEquality(StatementCoefficientEquality {
            right: if independent { right } else { left.clone() },
            left,
            map: map.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::ConjugationTransport(CertificateConjugationTransport {
            map: map.clone(),
            conjugate_map: map,
            images: Vec::new(),
            values: Vec::new(),
            relations: Vec::new(),
            fixed_field: Vec::new(),
            region: base.region.clone(),
        }),
    );
    step.premises = vec![1, 2];
    Ok(BinomialFixture {
        base,
        steps: vec![unit, first, second, step],
    })
}

impl Fixture {
    /// A coefficient guard can restrict the problem independently of a claimed theorem region.
    pub(crate) fn coefficient_guard(&mut self, nonzero: bool) -> TestResult<Handle> {
        let parameter = self.free("a")?;
        let zero = self.integer(0, false)?;
        let predicate = self.equation(&parameter, &zero)?;
        let base = Reference::Existing(self.region.clone());
        let predicate = Reference::Existing(predicate);
        admit(
            &mut self.cas,
            &Draft::Region(RegionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: self.coefficients.clone(),
                },
                expression: if nonzero {
                    RegionExpr::GuardFalse { base, predicate }
                } else {
                    RegionExpr::GuardTrue { base, predicate }
                },
            }),
        )
    }

    /// Rebuild only the original region, keeping all original equations and domains identical.
    pub(crate) fn problem_region(
        &mut self,
        handle: &Handle,
        region: &Handle,
    ) -> TestResult<Handle> {
        let SolveView::SolveProblem(value) = self.cas.solve_view(handle)? else {
            return Err("missing problem".into());
        };
        let value = value.clone();
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
                header: Reference::Existing(value.header),
                scope: value.scope,
                predicate: Reference::Existing(value.predicate),
                coefficients: Reference::Existing(value.coefficients),
                unknowns: Reference::Existing(value.unknowns),
                region: Reference::Existing(region.clone()),
                domain: Reference::Existing(value.domain),
                coefficient_domain: Reference::Existing(value.coefficient_domain),
                ambient: value.ambient,
                denominators: value
                    .denominators
                    .into_iter()
                    .map(Reference::Existing)
                    .collect(),
            })),
        )
    }

    /// Preserve branch shape while rebinding a proposal to the explicitly changed original problem.
    pub(crate) fn set_problem(&mut self, handle: &Handle, problem: &Handle) -> TestResult<Handle> {
        use crate::test_api::solving::{SolutionBranchData, SolutionBranchKind, SolutionSetData};
        let SolveView::SolutionSet(value) = self.cas.solve_view(handle)? else {
            return Err("missing set".into());
        };
        let members = value.branches.clone();
        let mut branches = Vec::new();
        for member in members {
            let SolveView::SolutionBranch(value) = self.cas.solve_view(&member)? else {
                return Err("missing branch".into());
            };
            let value = value.clone();
            let kind = match value.kind {
                SolutionBranchKind::Finite { tuples } => SolutionBranchKind::Finite {
                    tuples: tuples.into_iter().map(Reference::Existing).collect(),
                },
                SolutionBranchKind::Image { family } => SolutionBranchKind::Image {
                    family: Reference::Existing(family),
                },
                SolutionBranchKind::Empty => SolutionBranchKind::Empty,
                _ => return Err("unexpected branch".into()),
            };
            branches.push(Reference::Existing(admit(
                &mut self.cas,
                &Draft::Solve(SolveDraft::SolutionBranch(SolutionBranchData {
                    problem: Reference::Existing(problem.clone()),
                    region: Reference::Existing(value.region),
                    domain: Reference::Existing(value.domain),
                    kind,
                })),
            )?));
        }
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::SolutionSet(SolutionSetData {
                problem: Reference::Existing(problem.clone()),
                branches,
            })),
        )
    }

    /// Root contexts make the claimed region explicit without borrowing a hypothesis.
    pub(crate) fn enter_region(&mut self, region: Handle) -> TestResult {
        self.context = admit(
            &mut self.cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(self.header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        self.region = region;
        Ok(())
    }

    /// Presentation is separate from coverage and repeats the requested form in its certificate.
    pub(crate) fn present(
        &self,
        set: &Handle,
        form: crate::test_api::solving::SolveForm,
    ) -> ApplicationDraft {
        use crate::test_api::solving::{
            CertificateRootSelectionRefinement, RefinementMode, RefinementTargetKind,
            SolveCertificate, SolveSchema, SolveStatement, StatementPresentation,
        };
        self.step(
            SolveSchema::RootSelectionRefinement,
            SolveStatement::Presentation(StatementPresentation {
                set: set.clone(),
                form,
                region: self.region.clone(),
            }),
            SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
                mode: RefinementMode::Presentation,
                target_kind: RefinementTargetKind::SolutionSet,
                target: set.clone(),
                witnesses: Vec::new(),
                compatibility: Vec::new(),
                form: Some(form),
            }),
        )
    }

    /// The formal leading coefficient a is mapped independently of its nonzero formal representation.
    fn parameter_binomial(&mut self, image: &Handle) -> TestResult<(Handle, Handle)> {
        use crate::test_api::solving::{CoefficientValueData, CoefficientValueObject};
        let coordinates = coordinate_product(
            &mut self.cas,
            &self.header,
            &["a"],
            CoordinateRole::Coefficient,
            &self.field,
        )?;
        let algebra = self.algebra.clone();
        let domain = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Fraction {
                    base: Reference::Existing(algebra.clone()),
                    coordinates: Reference::Existing(coordinates),
                },
            )),
        )?;
        let one = self.coefficient(1)?;
        let numerator = self.polynomial(&algebra, &[(1, one.clone())])?;
        let denominator = self.polynomial(&algebra, &[(0, one)])?;
        let leading = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(domain.clone()),
                value: CoefficientValueData::Fraction {
                    numerator: Reference::Existing(numerator),
                    denominator: Reference::Existing(denominator),
                },
            })),
        )?;
        let polynomial = self.polynomial(&domain, &[(2, leading)])?;
        let map = self.scalar_map(&domain, std::slice::from_ref(image))?;
        Ok((polynomial, map))
    }
}

/// Constructive existence of zero in a*X² does not establish radical provenance when a can vanish.
pub(crate) fn specialized_presentation(stratum: u8) -> TestResult<BinomialFixture> {
    use crate::test_api::solving::{
        CertificateRootSelectionRefinement, RefinementMode, RefinementTargetKind, SolveCertificate,
        SolveForm, SolveSchema, SolveStatement, StatementRootExistence,
    };
    let mut base = Fixture::in_field(&Field::Complex)?;
    if stratum < 2 {
        let region = base.coefficient_guard(stratum == 1)?;
        base.enter_region(region)?;
    }
    let image = base.free("a")?;
    let (polynomial, map) = base.parameter_binomial(&image)?;
    let (_, selection, root) = base.root(&polynomial, &map)?;
    let zero = base.integer(0, false)?;
    let existence = base.step(
        SolveSchema::RootSelectionRefinement,
        SolveStatement::RootExistence(StatementRootExistence {
            relation: selection.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
            mode: RefinementMode::Existence,
            target_kind: RefinementTargetKind::Selection,
            target: selection,
            witnesses: vec![zero.clone()],
            compatibility: Vec::new(),
            form: None,
        }),
    );
    let predicate = base.equation(&zero, &zero)?;
    let problem = base.problem("x", &predicate, &[])?;
    let set = base.finite(&problem, &[root])?;
    let mut presentation = base.present(&set, SolveForm::Radicals);
    presentation.premises = vec![0];
    Ok(BinomialFixture {
        base,
        steps: vec![existence, presentation],
    })
}

/// An exact family must be represented by an image branch, including the zero-parameter subcase.
pub(crate) fn family_presentation(image: bool) -> TestResult<BinomialFixture> {
    use crate::test_api::solving::{SolveForm, SolveStatement};
    let (mut base, steps) = identity_family(0)?;
    let Statement::Solve(statement) = &steps.last().ok_or("missing coverage")?.conclusion else {
        return Err("wrong statement".into());
    };
    let SolveStatement::Coverage(value) = statement.as_ref() else {
        return Err("wrong coverage".into());
    };
    let set = if image {
        value.set.clone()
    } else {
        let zero = base.integer(0, false)?;
        base.finite(&value.problem, &[zero])?
    };
    let mut presentation = base.present(&set, SolveForm::ExactFamily);
    presentation.premises = vec![0];
    let membership = steps.first().ok_or("missing membership")?.clone();
    Ok(BinomialFixture {
        base,
        steps: vec![membership, presentation],
    })
}

/// Only the final binomial claim changes its original region; all earlier arithmetic remains valid.
pub(crate) fn binomial_region(zero: bool, restricted: bool) -> TestResult<BinomialFixture> {
    use crate::test_api::solving::SolveStatement;
    let mut fixture = if zero {
        BinomialFixture::zero(2)?
    } else {
        BinomialFixture::new(2, 2)?
    };
    let region = if restricted {
        fixture.base.coefficient_guard(false)?
    } else {
        fixture.base.region.clone()
    };
    let step = fixture.steps.last_mut().ok_or("missing coverage")?;
    let Statement::Solve(statement) = &mut step.conclusion else {
        return Err("wrong statement".into());
    };
    let SolveStatement::Coverage(value) = statement.as_mut() else {
        return Err("wrong coverage".into());
    };
    value.problem = fixture.base.problem_region(&value.problem, &region)?;
    value.set = fixture.base.set_problem(&value.set, &value.problem)?;
    Ok(fixture)
}

/// A guarded linear claim is allowed inside its original region, never in the reverse direction.
pub(crate) fn linear_region(inconsistent: bool, contained: bool) -> TestResult<BinomialFixture> {
    use crate::test_api::solving::{SolveCertificate, SolveStatement};
    let (mut base, mut step) = linear_system(inconsistent)?;
    let region = base.coefficient_guard(false)?;
    let Statement::Solve(statement) = &mut step.conclusion else {
        return Err("wrong statement".into());
    };
    let SolveStatement::LinearSystem(value) = statement.as_mut() else {
        return Err("wrong system".into());
    };
    if contained {
        base.enter_region(region)?;
        value.region.clone_from(&base.region);
        step.context.clone_from(&base.context);
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::ExactLinearEquivalence(value) = certificate.as_mut() else {
            return Err("wrong linear certificate".into());
        };
        value.region.clone_from(&base.region);
    } else {
        value.problem = base.problem_region(&value.problem, &region)?;
    }
    Ok(BinomialFixture {
        base,
        steps: vec![step],
    })
}

impl Fixture {
    /// Distinct tautological selectors retain independent choices of the same root relation.
    fn independent_root(&mut self, relation: &Handle) -> TestResult<(Handle, Handle)> {
        use crate::test_api::solving::{RootSelectionData, SelectionMode};
        let zero = self.integer(0, true)?;
        let predicate = admit(
            &mut self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: vec![vec![AtomicSort::Scalar(self.field.clone())]],
                kind: PredicateKind::Equal,
                arguments: vec![Reference::Existing(zero.clone()), Reference::Existing(zero)],
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
        let value = admit(
            &mut self.cas,
            &Draft::RootValue {
                header: Reference::Existing(self.header.clone()),
                selection: Reference::Existing(selection.clone()),
                slot: 0,
            },
        )?;
        Ok((selection, value))
    }
}
