// SPDX-License-Identifier: AGPL-3.0-only
//! Separate positive Real parameters lie beneath the original Complex fibre.

use super::{Fixture, TestResult, admit, expression, reference};
use nrz_bindings::readings::algebra::{
    Cas, Handle,
    domain::{AmbientDomain, HeaderDraft, RegionDraft, RegionExpr},
    limits::{Budget, Limits},
    solving::{
        Coordinate, CoordinateRole, CoordinateSpaceData, LiesOver, ParameterBaseData, SolveDraft,
        SpaceFamilyData,
    },
    syntax::{
        AtomicSort, Branch, Declaration, Draft, Environment, Field, Operator, PredicateKind, Sort,
    },
};

pub(super) struct Family {
    pub(super) parameters: Handle,
    pub(super) unknowns: Handle,
    pub(super) handle: Handle,
    pub(super) region: Handle,
}

pub(super) fn store(limits: Limits, budget: &mut Budget) -> TestResult<Fixture> {
    let mut cas = Cas::new(limits)?;
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
    .collect::<Vec<_>>();
    let header = admit(
        &mut cas,
        Draft::Header(HeaderDraft::Core(Environment {
            field: Field::Complex,
            branch: Branch::PrincipalComplex,
            declarations,
        })),
        budget,
    )?;
    let real = scalar(&mut cas, &header, "L", Field::Real, budget)?;
    let complex = scalar(&mut cas, &header, "z", Field::Complex, budget)?;
    let complex_space = cas.scalar_ambient(&complex)?.ok_or("Complex ambient")?;
    Ok(Fixture {
        cas,
        header,
        real,
        complex,
        complex_space,
    })
}

fn scalar(
    cas: &mut Cas,
    header: &Handle,
    key: &str,
    field: Field,
    budget: &mut Budget,
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Expression(nrz_bindings::readings::algebra::syntax::ExpressionDraft {
            header: reference(header),
            scope: Vec::new(),
            operator: Operator::Free(key.into()),
            sort: Sort::Value(AtomicSort::Scalar(field)),
            operands: Vec::new(),
            space: None,
            binding: None,
        }),
        budget,
    )
}

fn coordinates(
    fixture: &mut Fixture,
    entries: &[(&str, Field, CoordinateRole)],
    budget: &mut Budget,
) -> TestResult<Handle> {
    let coordinates = entries
        .iter()
        .map(|(key, field, role)| Coordinate {
            key: (*key).into(),
            field: field.clone(),
            role: *role,
        })
        .collect();
    admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: reference(&fixture.header),
            coordinates,
            scalar_spaces: Vec::new(),
        })),
        budget,
    )
}

pub(super) fn region(
    fixture: &mut Fixture,
    coordinates: &Handle,
    budget: &mut Budget,
) -> TestResult<Handle> {
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
        budget,
    )
}

fn guard(
    fixture: &mut Fixture,
    coordinates: &Handle,
    base: &Handle,
    key: &str,
    sign: i8,
    budget: &mut Budget,
) -> TestResult<(Handle, RegionExpr)> {
    if sign == 0 {
        return Ok((base.clone(), RegionExpr::Ambient));
    }
    let mut subject = expression(
        fixture,
        Field::Real,
        Operator::Free(key.into()),
        &[],
        budget,
    )?;
    if sign < 0 {
        subject = expression(fixture, Field::Real, Operator::Neg, &[subject], budget)?;
    }
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind: PredicateKind::Positive,
            arguments: vec![reference(&subject)],
        },
        budget,
    )?;
    let guard_expr = RegionExpr::GuardTrue {
        base: reference(base),
        predicate: reference(&predicate),
    };
    let result = admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: coordinates.clone(),
            },
            expression: guard_expr.clone(),
        }),
        budget,
    )?;
    Ok((result, guard_expr))
}

pub(super) fn build(
    fixture: &mut Fixture,
    signs: [i8; 2],
    budget: &mut Budget,
) -> TestResult<Family> {
    let parameters = coordinates(
        fixture,
        &[
            ("L", Field::Real, CoordinateRole::Coefficient),
            ("C", Field::Real, CoordinateRole::Coefficient),
        ],
        budget,
    )?;
    let coefficients = coordinates(fixture, &[], budget)?;
    let unknowns = coordinates(
        fixture,
        &[("z", Field::Complex, CoordinateRole::Unknown)],
        budget,
    )?;
    let base_region = region(fixture, &parameters, budget)?;
    let base = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::ParameterBase(ParameterBaseData {
            header: reference(&fixture.header),
            coordinates: reference(&parameters),
            domain: reference(&base_region),
        })),
        budget,
    )?;
    let handle = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::SpaceFamily(SpaceFamilyData {
            base: reference(&base),
            fibre_space: reference(&fixture.complex_space),
            coefficients: reference(&coefficients),
            unknowns: reference(&unknowns),
        })),
        budget,
    )?;
    let (base_region, first) = guard(fixture, &parameters, &base_region, "L", signs[0], budget)?;
    let (base_region, second) = guard(fixture, &parameters, &base_region, "C", signs[1], budget)?;
    let expression = if signs[1] == 0 { first } else { second };
    let region = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: parameters.clone(),
                },
                expression,
            },
            lies_over: LiesOver {
                family: reference(&handle),
                stratum: reference(&base_region),
            },
        },
        budget,
    )?;
    Ok(Family {
        parameters,
        unknowns,
        handle,
        region,
    })
}
