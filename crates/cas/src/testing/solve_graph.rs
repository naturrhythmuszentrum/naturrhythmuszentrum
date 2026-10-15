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
