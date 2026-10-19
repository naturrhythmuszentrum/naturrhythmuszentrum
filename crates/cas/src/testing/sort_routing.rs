// SPDX-License-Identifier: AGPL-3.0-only
//! Original mixed declarations used only to test structural routing.

use super::common::TestResult;
use super::sorts::{admit, expression, reference};
use crate::test_api::domain::{AmbientDomain, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::limits::Limits;
use crate::test_api::solving::{
    CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData, SolveDraft,
    SolveProblemData,
};
use crate::test_api::syntax::{
    AtomicSort, Branch, Declaration, Draft, Environment, Field, Operator, PredicateKind, Sort,
};
use crate::test_api::{Cas, Handle};

/// Independent original syntax with no producer hint or certificate.
pub(crate) struct Fixture {
    /// Owning graph.
    pub(crate) cas: Cas,
    /// Original problem whose unknown has the requested field.
    pub(crate) problem: Handle,
    /// Original Real parameter.
    pub(crate) real: Handle,
    /// Original Complex parameter.
    pub(crate) complex: Handle,
}

impl Fixture {
    /// An unused Complex declaration remains present in every original header.
    pub(crate) fn new(field: Field, denominator: u8) -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let header = original_header(&mut cas, &field)?;
        let coefficients = coordinates(
            &mut cas,
            &header,
            &[("r", Field::Real), ("c", Field::Complex)],
            CoordinateRole::Coefficient,
        )?;
        let unknowns = coordinates(
            &mut cas,
            &header,
            &[("x", field.clone())],
            CoordinateRole::Unknown,
        )?;
        let region = product_region(&mut cas, &header, &coefficients)?;
        let domain = product_region(&mut cas, &header, &unknowns)?;
        let real = expression(
            &mut cas,
            &header,
            Field::Real,
            Operator::Free("r".into()),
            Vec::new(),
            Vec::new(),
        )?;
        let complex = expression(
            &mut cas,
            &header,
            Field::Complex,
            Operator::Free("c".into()),
            Vec::new(),
            Vec::new(),
        )?;
        let unknown = expression(
            &mut cas,
            &header,
            field.clone(),
            Operator::Free("x".into()),
            Vec::new(),
            Vec::new(),
        )?;
        let predicate = admit(
            &mut cas,
            Draft::Condition {
                kind: PredicateKind::Equal,
                header: reference(&header),
                scope: Vec::new(),
                arguments: vec![reference(&unknown), reference(&unknown)],
            },
        )?;
        let algebra = admit(
            &mut cas,
            Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            )),
        )?;
        let original_denominators = denominators(&mut cas, &header, &real, &complex, denominator)?;
        let problem = admit(
            &mut cas,
            Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
                header: reference(&header),
                scope: Vec::new(),
                predicate: reference(&predicate),
                coefficients: reference(&coefficients),
                unknowns: reference(&unknowns),
                region: reference(&region),
                domain: reference(&domain),
                coefficient_domain: reference(&algebra),
                ambient: field,
                denominators: original_denominators,
            })),
        )?;
        Ok(Self {
            cas,
            problem,
            real,
            complex,
        })
    }
}

/// Build the immutable original frame, including an intentionally unused Complex declaration.
fn original_header(cas: &mut Cas, field: &Field) -> TestResult<Handle> {
    let declarations = [
        ("r", Field::Real),
        ("c", Field::Complex),
        ("x", field.clone()),
    ]
    .into_iter()
    .map(|(key, field)| Declaration {
        key: key.into(),
        display_name: key.into(),
        sort: Sort::Value(AtomicSort::Scalar(field)),
    })
    .collect();
    let branch = if *field == Field::Real {
        Branch::StrictReal
    } else {
        Branch::PrincipalComplex
    };
    admit(
        cas,
        Draft::Header(HeaderDraft::Core(Environment {
            field: field.clone(),
            branch,
            declarations,
        })),
    )
}

/// Original coordinate rows retain their exact declared fields and roles.
fn coordinates(
    cas: &mut Cas,
    header: &Handle,
    entries: &[(&str, Field)],
    role: CoordinateRole,
) -> TestResult<Handle> {
    let coordinates = entries
        .iter()
        .map(|(key, field)| Coordinate {
            key: (*key).into(),
            field: field.clone(),
            role,
        })
        .collect();
    admit(
        cas,
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            scalar_spaces: Vec::new(),
            header: reference(header),
            coordinates,
        })),
    )
}

/// Build an original product region without a coverage claim.
fn product_region(cas: &mut Cas, header: &Handle, coordinates: &Handle) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Region(RegionDraft {
            header: reference(header),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: coordinates.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
    )
}

/// Canceled and projected Complex dependencies remain original syntax.
fn denominators(
    cas: &mut Cas,
    header: &Handle,
    real: &Handle,
    complex: &Handle,
    choice: u8,
) -> TestResult<Vec<crate::test_api::Reference>> {
    let value = match choice {
        0 => return Ok(Vec::new()),
        1 => complex.clone(),
        2 => expression(
            cas,
            header,
            Field::Real,
            Operator::RealPart,
            Vec::new(),
            vec![reference(complex)],
        )?,
        3 => expression(
            cas,
            header,
            Field::Complex,
            Operator::Sub,
            Vec::new(),
            vec![reference(complex), reference(complex)],
        )?,
        4 => real.clone(),
        _ => return Err("invalid fixture choice".into()),
    };
    Ok(vec![reference(&value)])
}
