// SPDX-License-Identifier: AGPL-3.0-only
//! Public construction fixtures shared by graph conformance and lifecycle tests.

use crate::test_api::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::limits::{Budget, Limits, LimitsOverride};
use crate::test_api::request::{CoveredInput, Request, Task};
use crate::test_api::solving::{
    Coordinate, CoordinateRole, CoordinateSpaceData, ExclusionPolicy, SolveDraft, SolveForm,
    SolveGoal, SolveProblemData, SolveSpec,
};
use crate::test_api::syntax::{
    AtomicSort, Branch, Declaration, Draft, Environment, ExpressionDraft, Field, OccurrenceDraft,
    Operator, PredicateKind, Sort,
};
use crate::test_api::{Cas, Handle, Reference};

use super::common::TestResult;

/// A symbolic problem with original source occurrences and an explicit empty coefficient product.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Owning store.
    pub(crate) cas: Cas,
    /// Immutable declaration header.
    pub(crate) header: Handle,
    /// Graph-owned original problem.
    pub(crate) problem: Handle,
    /// Requested coefficient region and context.
    pub(crate) context: Handle,
    /// Original predicate occurrence root.
    pub(crate) root: Handle,
}

/// One admission helper keeps all test assertions at their appropriate outer layer.
pub(crate) fn admit(cas: &mut Cas, draft: &Draft) -> TestResult<Handle> {
    cas.admit(
        std::slice::from_ref(draft),
        &mut Budget::new(Limits::default())?,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "missing admitted object".into())
}

/// Scalar syntax with an explicit immutable header and ordered operands.
pub(crate) const fn expression(
    header: Reference,
    operator: Operator,
    sort: Sort,
    operands: Vec<Reference>,
) -> Draft {
    Draft::Expression(ExpressionDraft {
        header,
        operator,
        sort,
        operands,
        space: None,
        scope: Vec::new(),
        binding: None,
    })
}

impl Fixture {
    /// Build x = x with one unknown and no coefficient parameters.
    pub(crate) fn new() -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let header = admit(
            &mut cas,
            &Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                branch: Branch::StrictReal,
                declarations: vec![Declaration {
                    key: "x".into(),
                    display_name: "x".into(),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                }],
            })),
        )?;
        let coefficients = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                header: Reference::Existing(header.clone()),
                coordinates: Vec::new(),
            })),
        )?;
        let unknowns = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                header: Reference::Existing(header.clone()),
                coordinates: vec![Coordinate {
                    key: "x".into(),
                    field: Field::Real,
                    role: CoordinateRole::Unknown,
                }],
            })),
        )?;
        let region = product(&mut cas, &header, coefficients.clone())?;
        let domain = product(&mut cas, &header, unknowns.clone())?;
        let context = admit(
            &mut cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let (predicate, root) = original(&mut cas, &header)?;
        let coefficient_domain = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                crate::test_api::solving::CoefficientDomainData::Rational,
            )),
        )?;
        let problem = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                predicate: Reference::Existing(predicate),
                coefficients: Reference::Existing(coefficients),
                unknowns: Reference::Existing(unknowns),
                region: Reference::Existing(region),
                domain: Reference::Existing(domain),
                coefficient_domain: Reference::Existing(coefficient_domain),
                ambient: Field::Real,
                denominators: Vec::new(),
            })),
        )?;
        Ok(Self {
            cas,
            header,
            problem,
            context,
            root,
        })
    }

    /// Complete the original walk before constructing the exact requested task.
    pub(crate) fn request(&self) -> TestResult<Request> {
        let walk = self
            .cas
            .walk_original(&self.root, &mut Budget::new(Limits::default())?)?;
        Ok(Request {
            context: self.context.clone(),
            task: Task::Solve(Box::new(SolveSpec {
                problem: self.problem.clone(),
                goal: SolveGoal::Complete,
                form: SolveForm::AnyExact,
                exclusions: ExclusionPolicy::PreserveOriginal,
            })),
            inputs: vec![CoveredInput {
                root: self.root.clone(),
                receipt: walk.receipt,
            }],
            premises: Vec::new(),
            limits: LimitsOverride::default(),
        })
    }
}

/// Product regions are first-class exact syntax, including the singleton empty tuple product.
fn product(cas: &mut Cas, header: &Handle, coordinates: Handle) -> TestResult<Handle> {
    admit(
        cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product { coordinates },
            expression: RegionExpr::Ambient,
        }),
    )
}

/// Preserve both original uses of x even when their mathematical syntax is interned.
fn original(cas: &mut Cas, header: &Handle) -> TestResult<(Handle, Handle)> {
    let values = cas.admit(
        &[
            Draft::Source {
                key: "solve-input".into(),
                text: "x=x".into(),
            },
            expression(
                Reference::Existing(header.clone()),
                Operator::Free("x".into()),
                Sort::Value(AtomicSort::Scalar(Field::Real)),
                Vec::new(),
            ),
            Draft::Condition {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Equal,
                arguments: vec![Reference::Local(1), Reference::Local(1)],
            },
            expression(
                Reference::Existing(header.clone()),
                Operator::Predicate(PredicateKind::Equal),
                Sort::Value(AtomicSort::Boolean),
                vec![Reference::Local(1), Reference::Local(1)],
            ),
            occurrence(Reference::Local(1), Reference::Local(0), Vec::new()),
            occurrence(Reference::Local(1), Reference::Local(0), Vec::new()),
            occurrence(
                Reference::Local(3),
                Reference::Local(0),
                vec![Reference::Local(4), Reference::Local(5)],
            ),
        ],
        &mut Budget::new(Limits::default())?,
    )?;
    Ok((
        values.get(2).ok_or("predicate missing")?.clone(),
        values.get(6).ok_or("occurrence missing")?.clone(),
    ))
}

/// Original occurrences retain source ownership and ordered children.
const fn occurrence(expression: Reference, source: Reference, children: Vec<Reference>) -> Draft {
    Draft::Occurrence(OccurrenceDraft {
        expression,
        source: Some(source),
        span: None,
        children,
        provenance: None,
        binder_names: Vec::new(),
    })
}

/// Existing hypothesis replay can carry exact coefficient syntax without enabling a solve theorem.
pub(crate) fn coefficient_witness() -> TestResult<(Cas, crate::test_api::proof::VerifiedWitness)> {
    let mut cas = Cas::new(Limits::default())?;
    let header = admit(
        &mut cas,
        &Draft::Header(HeaderDraft::Core(Environment {
            field: Field::Complex,
            branch: Branch::PrincipalComplex,
            declarations: Vec::new(),
        })),
    )?;
    let coordinates = admit(
        &mut cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(header.clone()),
            coordinates: Vec::new(),
        })),
    )?;
    let region = product(&mut cas, &header, coordinates)?;
    let literal = coefficient_literal_fixture(&mut cas, &header, &region)?;
    let witness = assumed_literal(&mut cas, &header, region, literal)?;
    Ok((cas, witness))
}

/// Construct a family proposal using either its bound slot or an escaped free declaration.
pub(crate) fn family_draft(fixture: &mut Fixture, escaped: bool) -> TestResult<Draft> {
    use crate::test_api::solving::{FamilyMapData, SolveView};
    let SolveView::SolveProblem(problem) = fixture.cas.solve_view(&fixture.problem)? else {
        return Err("problem missing".into());
    };
    let coefficients = problem.coefficients.clone();
    let target = problem.unknowns.clone();
    let family = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(fixture.header.clone()),
            coordinates: vec![Coordinate {
                key: "x".into(),
                field: Field::Real,
                role: CoordinateRole::Family,
            }],
        })),
    )?;
    let scope = vec![vec![AtomicSort::Scalar(Field::Real)]];
    let region = admit(
        &mut fixture.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: scope.clone(),
            ambient: AmbientDomain::Product {
                coordinates: family.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
    )?;
    let body = family_body(&mut fixture.cas, &fixture.header, &target, scope, escaped)?;
    Ok(Draft::Solve(SolveDraft::FamilyMap(FamilyMapData {
        header: Reference::Existing(fixture.header.clone()),
        scope: Vec::new(),
        coefficients: Reference::Existing(coefficients),
        family: Reference::Existing(family),
        region: Reference::Existing(region),
        target: Reference::Existing(target),
        body: Reference::Existing(body),
    })))
}

/// Build the Gaussian constant and its explicit standard ambient embedding.
fn coefficient_literal_fixture(
    cas: &mut Cas,
    header: &Handle,
    region: &Handle,
) -> TestResult<Handle> {
    use crate::test_api::solving::{
        CoefficientDomainData, CoefficientMapData, CoefficientValueData, CoefficientValueObject,
        MapMode, MapTarget,
    };
    let domain = admit(
        cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Gaussian,
        )),
    )?;
    let value = admit(
        cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(domain.clone()),
            value: CoefficientValueData::Gaussian {
                real: crate::test_api::Rational::zero(),
                imaginary: crate::test_api::Rational::one(),
            },
        })),
    )?;
    let map = admit(
        cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(domain),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(header.clone()),
                field: Field::Complex,
            },
            mode: MapMode::Embedding,
            region: Reference::Existing(region.clone()),
            images: Vec::new(),
            denominators: Vec::new(),
        })),
    )?;
    admit(
        cas,
        &Draft::ExactCoefficient {
            header: Reference::Existing(header.clone()),
            value: Reference::Existing(value),
            map: Reference::Existing(map),
        },
    )
}

/// The body uses a typed bound frame, with a deliberate escaped-key mutation for refusal tests.
fn family_body(
    cas: &mut Cas,
    header: &Handle,
    target: &Handle,
    scope: Vec<Vec<AtomicSort>>,
    escaped: bool,
) -> TestResult<Handle> {
    use crate::test_api::solving::TupleData;
    let entry = admit(
        cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            scope: scope.clone(),
            operator: if escaped {
                Operator::Free("x".into())
            } else {
                Operator::Bound { depth: 0, slot: 0 }
            },
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            operands: Vec::new(),
            space: None,
            binding: None,
        }),
    )?;
    admit(
        cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(header.clone()),
            scope,
            coordinates: Reference::Existing(target.clone()),
            entries: vec![Reference::Existing(entry)],
        })),
    )
}

/// An explicit hypothesis transports syntax without claiming any solving theorem.
fn assumed_literal(
    cas: &mut Cas,
    header: &Handle,
    region: Handle,
    literal: Handle,
) -> TestResult<crate::test_api::proof::VerifiedWitness> {
    let predicate = admit(
        cas,
        &Draft::Condition {
            kind: PredicateKind::Named("supplied-coefficient-property".into()),
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(literal)],
        },
    )?;
    let context = admit(
        cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(region.clone()),
            assumptions: vec![crate::test_api::domain::AssumptionDraft {
                key: "supplied".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
    )?;
    let step = crate::test_api::proof::ApplicationDraft {
        rule: crate::test_api::proof::Primitive::ContextHypothesis,
        context: context.clone(),
        conclusion: crate::test_api::proof::Statement::PredicateFact { region, predicate },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: crate::test_api::proof::Certificate::Hypothesis(
            crate::test_api::domain::HypothesisRef {
                context: context.clone(),
                slot: 0,
            },
        ),
    };
    Ok(cas.check_proof(
        &context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::default())?,
    )?)
}

/// Carry a bound root selection through a supplied hypothesis and independent v3 replay.
pub(crate) fn root_witness() -> TestResult<(Cas, crate::test_api::proof::VerifiedWitness)> {
    let mut cas = Cas::new(Limits::default())?;
    let header = admit(
        &mut cas,
        &Draft::Header(HeaderDraft::Core(Environment {
            field: Field::Complex,
            branch: Branch::PrincipalComplex,
            declarations: Vec::new(),
        })),
    )?;
    let coordinates = admit(
        &mut cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(header.clone()),
            coordinates: Vec::new(),
        })),
    )?;
    let region = product(&mut cas, &header, coordinates)?;
    let selection = root_selection_fixture(&mut cas, &header, &region, Field::Complex)?;
    let literal = admit(
        &mut cas,
        &Draft::RootValue {
            header: Reference::Existing(header.clone()),
            selection: Reference::Existing(selection),
            slot: 0,
        },
    )?;
    let witness = assumed_literal(&mut cas, &header, region, literal)?;
    Ok((cas, witness))
}

/// The relation is a monic formal quadratic over Q with an explicit scalar specialization.
fn root_selection_fixture(
    cas: &mut Cas,
    header: &Handle,
    region: &Handle,
    field: Field,
) -> TestResult<Handle> {
    use crate::test_api::solving::{
        CoefficientDomainData, CoefficientMapData, MapMode, MapTarget, RootRelationData,
    };
    let domain = admit(
        cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
    )?;
    let polynomial = quadratic_fixture(cas, &domain, field.clone())?;
    let map = admit(
        cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(domain),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(header.clone()),
                field: field.clone(),
            },
            mode: MapMode::Specialization,
            region: Reference::Existing(region.clone()),
            images: Vec::new(),
            denominators: Vec::new(),
        })),
    )?;
    let relation = admit(
        cas,
        &Draft::Solve(SolveDraft::RootRelation(RootRelationData {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: field.clone(),
            polynomial: Reference::Existing(polynomial),
            binder: vec![AtomicSort::Scalar(field.clone())],
            map: Reference::Existing(map),
            region: Reference::Existing(region.clone()),
            multiplicity: None,
        })),
    )?;
    joint_selection(cas, header, relation, field)
}

/// Polynomial coefficients are independent exact rational graph values in decreasing graded order.
fn quadratic_fixture(cas: &mut Cas, domain: &Handle, field: Field) -> TestResult<Handle> {
    use crate::test_api::solving::{
        CoefficientValueData, CoefficientValueObject, PolynomialData, PolynomialTerm,
    };
    let mut coefficients = Vec::new();
    for text in ["1", "-2"] {
        let value = crate::test_api::Rational::parse(text, &mut Budget::new(Limits::default())?)?;
        coefficients.push(admit(
            cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(domain.clone()),
                value: CoefficientValueData::Rational { value },
            })),
        )?);
    }
    admit(
        cas,
        &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
            domain: Reference::Existing(domain.clone()),
            variables: vec![field],
            terms: vec![
                PolynomialTerm {
                    exponents: vec![2],
                    coefficient: Reference::Existing(
                        coefficients
                            .first()
                            .ok_or("leading coefficient missing")?
                            .clone(),
                    ),
                },
                PolynomialTerm {
                    exponents: vec![0],
                    coefficient: Reference::Existing(
                        coefficients.get(1).ok_or("constant missing")?.clone(),
                    ),
                },
            ],
        })),
    )
}

/// One bound slot is constrained by definedness, without asserting its existence.
fn joint_selection(
    cas: &mut Cas,
    header: &Handle,
    relation: Handle,
    field: Field,
) -> TestResult<Handle> {
    use crate::test_api::solving::{RootSelectionData, SelectionMode};
    let scope = vec![vec![AtomicSort::Scalar(field.clone())]];
    let bound = admit(
        cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            scope: scope.clone(),
            operator: Operator::Bound { depth: 0, slot: 0 },
            sort: Sort::Value(AtomicSort::Scalar(field)),
            space: None,
            binding: None,
            operands: Vec::new(),
        }),
    )?;
    let predicate = admit(
        cas,
        &Draft::Condition {
            header: Reference::Existing(header.clone()),
            scope,
            kind: PredicateKind::Defined,
            arguments: vec![Reference::Existing(bound)],
        },
    )?;
    admit(
        cas,
        &Draft::Solve(SolveDraft::RootSelection(RootSelectionData {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            relations: vec![Reference::Existing(relation)],
            predicate: Reference::Existing(predicate),
            mode: SelectionMode::Constrained,
            convention: None,
            interval: None,
        })),
    )
}

/// Rebuild a problem against a new original predicate, retaining every declared product and region.
fn replace_predicate(
    fixture: &mut Fixture,
    predicate: Handle,
    denominators: Vec<Handle>,
) -> TestResult<Handle> {
    let crate::test_api::solving::SolveView::SolveProblem(base) =
        fixture.cas.solve_view(&fixture.problem)?
    else {
        return Err("problem missing".into());
    };
    let draft = Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
        header: Reference::Existing(base.header.clone()),
        scope: base.scope.clone(),
        predicate: Reference::Existing(predicate),
        coefficients: Reference::Existing(base.coefficients.clone()),
        unknowns: Reference::Existing(base.unknowns.clone()),
        region: Reference::Existing(base.region.clone()),
        domain: Reference::Existing(base.domain.clone()),
        coefficient_domain: Reference::Existing(base.coefficient_domain.clone()),
        ambient: base.ambient.clone(),
        denominators: denominators.into_iter().map(Reference::Existing).collect(),
    }));
    admit(&mut fixture.cas, &draft)
}

/// Original x/x = 1 keeps the denominator occurrence even though simplification could cancel it.
pub(crate) fn quotient_request(omit_denominator: bool) -> TestResult<(Fixture, Request)> {
    let mut fixture = Fixture::new()?;
    let header = Reference::Existing(fixture.header.clone());
    let scalar = Sort::Value(AtomicSort::Scalar(Field::Real));
    let values = fixture.cas.admit(
        &[
            expression(
                header.clone(),
                Operator::Free("x".into()),
                scalar.clone(),
                Vec::new(),
            ),
            expression(
                header.clone(),
                Operator::Rational(crate::test_api::Rational::one()),
                scalar.clone(),
                Vec::new(),
            ),
            expression(
                header.clone(),
                Operator::Div,
                scalar,
                vec![Reference::Local(0), Reference::Local(0)],
            ),
            expression(
                header.clone(),
                Operator::Predicate(PredicateKind::Equal),
                Sort::Value(AtomicSort::Boolean),
                vec![Reference::Local(2), Reference::Local(1)],
            ),
            Draft::Condition {
                header,
                scope: Vec::new(),
                kind: PredicateKind::Equal,
                arguments: vec![Reference::Local(2), Reference::Local(1)],
            },
            Draft::Source {
                key: "quotient-original".into(),
                text: "x/x=1".into(),
            },
        ],
        &mut Budget::new(Limits::default())?,
    )?;
    let variable = values.first().ok_or("unknown missing")?.clone();
    let syntax = values.get(3).ok_or("syntax missing")?;
    let predicate = values.get(4).ok_or("predicate missing")?.clone();
    let source = values.get(5).ok_or("source missing")?;
    fixture.root =
        fixture
            .cas
            .materialize_original(syntax, source, &mut Budget::new(Limits::default())?)?;
    let denominators = if omit_denominator {
        Vec::new()
    } else {
        vec![variable]
    };
    fixture.problem = replace_predicate(&mut fixture, predicate, denominators)?;
    let request = fixture.request()?;
    Ok((fixture, request))
}
