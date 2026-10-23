// SPDX-License-Identifier: AGPL-3.0-only
//! Independent determinant construction supplies no reciprocal positivity guard.

use crate::domain::{AmbientDomain, HeaderDraft, RegionDraft, RegionExpr};
use crate::limits::Limits;
use crate::request::{CoveredInput, Request, Task};
use crate::solving::{
    CoordinateRole, FamilyArgument, FamilyArgumentRole, FamilyExpressionData, LiesOver,
    ParameterBaseData, SolveDraft, SpaceFamilyData,
};
use crate::syntax::{
    AtomicSort, Branch, Declaration, Draft, Environment, ExpressionDraft, Field, Operator,
    PredicateKind, Sort,
};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Family, Fixture, admit, expression, reference};
use crate::{Cas, Handle, Rational};

fn store() -> TestResult<Fixture> {
    let mut cas = Cas::new(Limits::blind_cubic())?;
    let declarations = [
        ("L", Field::Real),
        ("C", Field::Real),
        ("z", Field::Complex),
    ]
    .into_iter()
    .map(|(key, field)| Declaration {
        key: key.into(),
        display_name: key.into(),
        sort: Sort::Value(AtomicSort::Scalar(field)),
    })
    .collect();
    let header = admit(
        &mut cas,
        Draft::Header(HeaderDraft::Core(Environment {
            field: Field::Complex,
            branch: Branch::PrincipalComplex,
            declarations,
        })),
    )?;
    let real = expression(
        &mut cas,
        &header,
        Field::Real,
        Operator::Free("L".into()),
        Vec::new(),
        Vec::new(),
    )?;
    let complex = expression(
        &mut cas,
        &header,
        Field::Complex,
        Operator::Free("z".into()),
        Vec::new(),
        Vec::new(),
    )?;
    let real_space = cas.scalar_ambient(&real)?.ok_or("Real ambient")?;
    let complex_space = cas.scalar_ambient(&complex)?.ok_or("Complex ambient")?;
    Ok(Fixture {
        cas,
        header,
        real,
        complex,
        real_space,
        complex_space,
    })
}

fn region(fixture: &mut Fixture, coordinates: &Handle) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: coordinates.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
    )
}

fn guard(fixture: &mut Fixture, base: &Handle, key: &str, sign: i8) -> TestResult<Handle> {
    if sign == 0 {
        return Ok(base.clone());
    }
    let mut subject = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Free(key.into()),
        Vec::new(),
        Vec::new(),
    )?;
    if sign < 0 {
        subject = expression(
            &mut fixture.cas,
            &fixture.header,
            Field::Real,
            Operator::Neg,
            Vec::new(),
            vec![reference(&subject)],
        )?;
    }
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind: PredicateKind::Positive,
            arguments: vec![reference(&subject)],
        },
    )?;
    let ambient = fixture.cas.state.region(base.logical_id())?.ambient.clone();
    admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient,
            expression: RegionExpr::GuardTrue {
                base: reference(base),
                predicate: reference(&predicate),
            },
        }),
    )
}

fn family(fixture: &mut Fixture, signs: [i8; 2]) -> TestResult<Family> {
    let parameters = fixture.coordinates(&[
        ("L", Field::Real, CoordinateRole::Coefficient),
        ("C", Field::Real, CoordinateRole::Coefficient),
    ])?;
    let coefficients = fixture.coordinates(&[])?;
    let unknowns = fixture.coordinates(&[("z", Field::Complex, CoordinateRole::Unknown)])?;
    let total = parameters.clone();
    let base_region = region(fixture, &parameters)?;
    let base = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::ParameterBase(ParameterBaseData {
            header: reference(&fixture.header),
            coordinates: reference(&parameters),
            domain: reference(&base_region),
        })),
    )?;
    let handle = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::SpaceFamily(SpaceFamilyData {
            base: reference(&base),
            fibre_space: reference(&fixture.complex_space),
            coefficients: reference(&coefficients),
            unknowns: reference(&unknowns),
        })),
    )?;
    let base_region = guard(fixture, &base_region, "L", signs[0])?;
    let base_region = guard(fixture, &base_region, "C", signs[1])?;
    let descriptor = fixture.cas.state.region(base_region.logical_id())?.clone();
    let expression = match descriptor.expression {
        RegionExpr::GuardTrue { base, predicate } => RegionExpr::GuardTrue {
            base: reference(&fixture.cas.handle(base)),
            predicate: reference(&fixture.cas.handle(predicate)),
        },
        RegionExpr::Ambient => RegionExpr::Ambient,
        _ => return Err("guard constructor".into()),
    };
    let region = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: Vec::new(),
                ambient: descriptor.ambient,
                expression,
            },
            lies_over: LiesOver {
                family: reference(&handle),
                stratum: reference(&base_region),
            },
        },
    )?;
    Ok(Family {
        parameters,
        base_region,
        handle,
        region,
        total,
    })
}

fn local(
    fixture: &mut Fixture,
    family: &Family,
    operator: Operator,
    inputs: &[(Handle, FamilyArgumentRole)],
) -> TestResult<Handle> {
    let arguments = inputs
        .iter()
        .map(|(value, role)| {
            let original = if *role == FamilyArgumentRole::Family {
                let view = fixture.cas.state.expression(value.logical_id())?;
                let [arm] = view.arms.as_slice() else {
                    return Err("LC family view".into());
                };
                fixture.cas.handle(fixture.cas.state.arm(*arm)?.value)
            } else {
                value.clone()
            };
            Ok(FamilyArgument {
                role: *role,
                value: reference(&original),
            })
        })
        .collect::<TestResult<Vec<_>>>()?;
    Ok(fixture.cas.family_expression(
        &FamilyExpressionData {
            family: reference(&family.handle),
            ambient: reference(&fixture.complex_space),
            region: reference(&family.region),
            scope: Vec::new(),
            operator,
            result: Sort::Value(AtomicSort::Scalar(Field::Complex)),
            arguments,
        },
        &mut super::ledger()?,
    )?)
}

fn real(fixture: &mut Fixture, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
    expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        operator,
        Vec::new(),
        operands.iter().map(reference).collect(),
    )
}

fn determinant(fixture: &mut Fixture, family: &Family) -> TestResult<(Handle, Handle)> {
    let zero = real(fixture, Operator::Rational(Rational::zero()), &[])?;
    let one = real(fixture, Operator::Rational(Rational::one()), &[])?;
    let capacitance = real(fixture, Operator::Free("C".into()), &[])?;
    let upper = real(fixture, Operator::Div, &[one.clone(), capacitance])?;
    let inductance = fixture.real.clone();
    let reciprocal = real(fixture, Operator::Div, &[one, inductance])?;
    let lower = real(fixture, Operator::Neg, &[reciprocal])?;
    let cross = real(fixture, Operator::Mul, &[upper, lower])?;
    let unknown = fixture.complex.clone();
    let diagonal = local(
        fixture,
        family,
        Operator::Sub,
        &[
            (unknown, FamilyArgumentRole::Fibre),
            (zero, FamilyArgumentRole::Base),
        ],
    )?;
    let square = local(
        fixture,
        family,
        Operator::Mul,
        &[
            (diagonal.clone(), FamilyArgumentRole::Family),
            (diagonal, FamilyArgumentRole::Family),
        ],
    )?;
    let left = local(
        fixture,
        family,
        Operator::Sub,
        &[
            (square, FamilyArgumentRole::Family),
            (cross, FamilyArgumentRole::Base),
        ],
    )?;
    let zero = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Complex,
        Operator::Rational(Rational::zero()),
        Vec::new(),
        Vec::new(),
    )?;
    Ok((left, zero))
}

fn source(fixture: &mut Fixture, left: &Handle, zero: &Handle) -> TestResult<CoveredInput> {
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
    )?;
    let source = admit(
        &mut fixture.cas,
        Draft::Source {
            key: "original-LC-determinant".into(),
            text: "det(sI-[[0,1/C],[-1/L,0]])=0".into(),
        },
    )?;
    let root = fixture
        .cas
        .materialize_original(&boolean, &source, &mut super::ledger()?)?;
    let receipt = fixture
        .cas
        .walk_original(&root, &mut super::ledger()?)?
        .receipt;
    Ok(CoveredInput { root, receipt })
}

fn denominators(
    fixture: &mut Fixture,
    original: &Handle,
    input: &CoveredInput,
    retain: bool,
) -> TestResult<Handle> {
    let data = crate::solving::graph::problem(&fixture.cas.state, original.logical_id())?.clone();
    let mut pending = vec![input.root.logical_id()];
    let mut divisors = Vec::new();
    while let Some(id) = pending.pop() {
        let occurrence = fixture.cas.state.occurrence(id)?;
        if let crate::graph::Vertex::Expression(value) =
            fixture.cas.state.node(occurrence.expression)?
            && value.operator == Operator::Div
        {
            divisors.push(reference(
                &fixture
                    .cas
                    .handle(*value.operands.get(1).ok_or("LC divisor")?),
            ));
        }
        pending.extend(occurrence.children.iter().rev().copied());
    }
    assert_eq!(divisors.len(), 4);
    admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::SolveProblem(crate::solving::SolveProblemData {
            header: reference(&data.header),
            scope: data.scope,
            ambient: data.ambient,
            coefficient_domain: reference(&data.coefficient_domain),
            coefficients: reference(&data.coefficients),
            unknowns: reference(&data.unknowns),
            predicate: reference(&data.predicate),
            region: reference(&data.region),
            domain: reference(&data.domain),
            denominators: if retain { divisors } else { Vec::new() },
        })),
    )
}

pub(super) fn request(signs: [i8; 2], retain: bool) -> TestResult<(Fixture, Request)> {
    let mut fixture = store()?;
    let family = family(&mut fixture, signs).map_err(|error| format!("LC family: {error}"))?;
    let (left, zero) =
        determinant(&mut fixture, &family).map_err(|error| format!("LC determinant: {error}"))?;
    let (problem, context) = super::super::problem(&mut fixture, &family, &left, &zero)?;
    let input = source(&mut fixture, &left, &zero)?;
    let problem = denominators(&mut fixture, &problem, &input, retain)?;
    Ok((
        fixture,
        Request {
            task: Task::Solve(Box::new(crate::solving::SolveSpec {
                problem,
                goal: crate::solving::SolveGoal::Complete,
                form: crate::solving::SolveForm::Radicals,
                exclusions: crate::solving::ExclusionPolicy::PreserveOriginal,
            })),
            context,
            inputs: vec![input],
            premises: Vec::new(),
            limits: super::super::profile(),
        },
    ))
}
