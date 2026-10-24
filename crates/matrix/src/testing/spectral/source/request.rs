// SPDX-License-Identifier: AGPL-3.0-only
//! Original determinant syntax retains local family bodies and each written division.

use super::{Fixture, TestResult, admit, expression, family::Family, reference};
use nrz_bindings::readings::algebra::{
    Handle, Rational,
    domain::ContextDraft,
    limits::Budget,
    request::{CoveredInput, Request, Task},
    solving::{
        CoefficientDomainData, ExclusionPolicy, FamilyArgument, FamilyArgumentRole,
        FamilyExpressionData, SolveDraft, SolveForm, SolveGoal, SolveProblemData, SolveSpec,
    },
    source::WalkEvent,
    syntax::{AtomicSort, Draft, ExpressionDraft, Field, Operator, PredicateKind, Sort},
};

fn local(
    fixture: &mut Fixture,
    family: &Family,
    operator: Operator,
    inputs: &[(Handle, FamilyArgumentRole)],
    budget: &mut Budget,
) -> TestResult<(Handle, Handle)> {
    let data = FamilyExpressionData {
        family: reference(&family.handle),
        ambient: reference(&fixture.complex_space),
        region: reference(&family.region),
        scope: Vec::new(),
        operator,
        result: Sort::Value(AtomicSort::Scalar(Field::Complex)),
        arguments: inputs
            .iter()
            .map(|(value, role)| FamilyArgument {
                role: *role,
                value: reference(value),
            })
            .collect(),
    };
    let body = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::FamilyExpression(data.clone())),
        budget,
    )?;
    let view = fixture.cas.family_expression(&data, budget)?;
    Ok((view, body))
}

fn determinant(
    fixture: &mut Fixture,
    family: &Family,
    scales: [i64; 2],
    invalid_zero: bool,
    budget: &mut Budget,
) -> TestResult<(Handle, Handle)> {
    let zero = diagonal_zero(fixture, invalid_zero, budget)?;
    let one = expression(
        fixture,
        Field::Real,
        Operator::Rational(Rational::one()),
        &[],
        budget,
    )?;
    let capacitance = expression(
        fixture,
        Field::Real,
        Operator::Free("C".into()),
        &[],
        budget,
    )?;
    let capacitance = scaled(fixture, capacitance, scales[1], budget)?;
    let upper = expression(
        fixture,
        Field::Real,
        Operator::Div,
        &[one.clone(), capacitance],
        budget,
    )?;
    let inductance = fixture.real.clone();
    let inductance = scaled(fixture, inductance, scales[0], budget)?;
    let reciprocal = expression(
        fixture,
        Field::Real,
        Operator::Div,
        &[one, inductance],
        budget,
    )?;
    let lower = expression(fixture, Field::Real, Operator::Neg, &[reciprocal], budget)?;
    let cross = expression(fixture, Field::Real, Operator::Mul, &[upper, lower], budget)?;
    let unknown = fixture.complex.clone();
    let (_, diagonal) = local(
        fixture,
        family,
        Operator::Sub,
        &[
            (unknown, FamilyArgumentRole::Fibre),
            (zero, FamilyArgumentRole::Base),
        ],
        budget,
    )?;
    let (_, square) = local(
        fixture,
        family,
        Operator::Mul,
        &[
            (diagonal.clone(), FamilyArgumentRole::Family),
            (diagonal, FamilyArgumentRole::Family),
        ],
        budget,
    )?;
    let (left, _) = local(
        fixture,
        family,
        Operator::Sub,
        &[
            (square, FamilyArgumentRole::Family),
            (cross, FamilyArgumentRole::Base),
        ],
        budget,
    )?;
    let zero = expression(
        fixture,
        Field::Complex,
        Operator::Rational(Rational::zero()),
        &[],
        budget,
    )?;
    Ok((left, zero))
}

fn source(
    fixture: &mut Fixture,
    left: &Handle,
    zero: &Handle,
    budget: &mut Budget,
) -> TestResult<(CoveredInput, Vec<Handle>)> {
    let boolean = admit(
        &mut fixture.cas,
        Draft::Expression(ExpressionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            operator: Operator::Predicate(PredicateKind::Equal),
            sort: Sort::Value(AtomicSort::Boolean),
            operands: vec![reference(left), reference(zero)],
            space: None,
            binding: None,
        }),
        budget,
    )?;
    let source = admit(
        &mut fixture.cas,
        Draft::Source {
            key: "original-LC-determinant".into(),
            text: "det(sI-[[0,1/C],[-1/L,0]])=0".into(),
        },
        budget,
    )?;
    let root = fixture
        .cas
        .materialize_original(&boolean, &source, budget)?;
    let walk = fixture.cas.walk_original(&root, budget)?;
    let mut divisors = Vec::new();
    for event in walk.events {
        let WalkEvent::Visit(visit) = event else {
            continue;
        };
        if fixture
            .cas
            .expression(&visit.expression)
            .is_ok_and(|(operator, _)| *operator == Operator::Div)
        {
            let operands = fixture.cas.operands(&visit.expression, budget)?;
            divisors.push(operands.get(1).ok_or("LC divisor")?.clone());
        }
    }
    Ok((
        CoveredInput {
            root,
            receipt: walk.receipt,
        },
        divisors,
    ))
}

pub(super) fn build(
    fixture: &mut Fixture,
    family: &Family,
    scales: [i64; 2],
    invalid_zero: bool,
    budget: &mut Budget,
) -> TestResult<Request> {
    let (left, zero) = determinant(fixture, family, scales, invalid_zero, budget)?;
    let (input, denominators) = source(fixture, &left, &zero, budget)?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind: PredicateKind::Equal,
            arguments: vec![reference(&left), reference(&zero)],
        },
        budget,
    )?;
    let ground = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
        budget,
    )?;
    let algebra = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Fraction {
                base: reference(&ground),
                coordinates: reference(&family.parameters),
            },
        )),
        budget,
    )?;
    let domain = super::family::region(fixture, &family.unknowns, budget)?;
    let problem = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient: Field::Complex,
            coefficient_domain: reference(&algebra),
            coefficients: reference(&family.parameters),
            unknowns: reference(&family.unknowns),
            predicate: reference(&predicate),
            region: reference(&family.region),
            domain: reference(&domain),
            denominators: denominators.iter().map(reference).collect(),
        })),
        budget,
    )?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&family.region),
            assumptions: Vec::new(),
        }),
        budget,
    )?;
    Ok(Request {
        task: Task::Solve(Box::new(SolveSpec {
            problem,
            goal: SolveGoal::Complete,
            form: SolveForm::Radicals,
            exclusions: ExclusionPolicy::PreserveOriginal,
        })),
        context,
        inputs: vec![input],
        premises: Vec::new(),
        limits: super::profile(),
    })
}

fn scaled(
    fixture: &mut Fixture,
    input: Handle,
    factor: i64,
    budget: &mut Budget,
) -> TestResult<Handle> {
    if factor == 1 {
        return Ok(input);
    }
    let coefficient = Rational::parse(&factor.to_string(), budget)?;
    let coefficient = expression(
        fixture,
        Field::Real,
        Operator::Rational(coefficient),
        &[],
        budget,
    )?;
    expression(
        fixture,
        Field::Real,
        Operator::Mul,
        &[coefficient, input],
        budget,
    )
}

fn diagonal_zero(fixture: &mut Fixture, invalid: bool, budget: &mut Budget) -> TestResult<Handle> {
    let zero = expression(
        fixture,
        Field::Real,
        Operator::Rational(Rational::zero()),
        &[],
        budget,
    )?;
    if !invalid {
        return Ok(zero);
    }
    let one = expression(
        fixture,
        Field::Real,
        Operator::Rational(Rational::one()),
        &[],
        budget,
    )?;
    let undefined = expression(
        fixture,
        Field::Real,
        Operator::Div,
        &[one, zero.clone()],
        budget,
    )?;
    expression(
        fixture,
        Field::Real,
        Operator::Mul,
        &[zero, undefined],
        budget,
    )
}
