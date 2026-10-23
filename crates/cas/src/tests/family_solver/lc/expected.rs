// SPDX-License-Identifier: AGPL-3.0-only
//! The expected coefficient identity is assembled from 1/(L*C), independently of the reader.

use crate::Handle;
use crate::proof::{Judgment, Statement};
use crate::request::{Request, Task};
use crate::solving::{CoefficientMapData, MapMode, MapTarget, SolveDraft};
use crate::syntax::{Draft, Field, Operator};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, expression, reference};

fn coefficient(
    fixture: &Fixture,
    domain: &Handle,
) -> TestResult<crate::solving::coefficients::model::Element> {
    use crate::solving::coefficients::{
        arithmetic::Engine,
        fraction::Fraction,
        model::Value,
        polynomial::{Polynomial, Term},
        scalar::{Ground, Scalar},
    };
    let mut budget = super::ledger()?;
    let numerator = Polynomial::new(
        2,
        Ground::Rational,
        vec![Term {
            powers: vec![0, 0],
            coefficient: Scalar::constant(Ground::Rational, true, &mut budget)?,
        }],
        &mut budget,
    )?;
    let denominator = Polynomial::new(
        2,
        Ground::Rational,
        vec![Term {
            powers: vec![1, 1],
            coefficient: Scalar::constant(Ground::Rational, true, &mut budget)?,
        }],
        &mut budget,
    )?;
    let engine = Engine {
        state: &fixture.cas.state,
        domain,
    };
    Ok(engine.element(Value::Fraction(Fraction::new(
        &numerator,
        &denominator,
        &mut budget,
    )?)))
}

fn map(
    fixture: &mut Fixture,
    domain: &Handle,
    region: &Handle,
    images: &[Handle],
) -> TestResult<Handle> {
    let over = fixture
        .cas
        .state
        .region(region.logical_id())?
        .lies_over
        .as_ref()
        .ok_or("LC projection")?;
    let family = fixture.cas.handle(over.family);
    admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: reference(domain),
            target: MapTarget::Family {
                family: reference(&family),
            },
            mode: MapMode::Specialization,
            region: reference(region),
            images: images.iter().map(reference).collect(),
            denominators: Vec::new(),
        })),
    )
}

pub(super) fn identity(fixture: &mut Fixture, request: &Request) -> TestResult<Judgment> {
    let Task::Solve(specification) = &request.task else {
        return Err("LC solve request".into());
    };
    let original =
        crate::solving::graph::problem(&fixture.cas.state, specification.problem.logical_id())?
            .clone();
    let (_, [left, _]) = crate::solving::elementary::evidence::predicate(
        &fixture.cas,
        original.predicate.logical_id(),
    )?
    else {
        return Err("LC equation".into());
    };
    let left = fixture.cas.handle(*left);
    let value = coefficient(fixture, &original.coefficient_domain)?;
    let value = crate::solving::towers::materialize::value(
        &mut fixture.cas,
        &value,
        &mut super::ledger()?,
    )?;
    let capacitance = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Free("C".into()),
        Vec::new(),
        Vec::new(),
    )?;
    let images = [fixture.real.clone(), capacitance];
    let interpretation = map(
        fixture,
        &original.coefficient_domain,
        &original.region,
        &images,
    )?;
    let literal = admit(
        &mut fixture.cas,
        Draft::ExactCoefficient {
            header: reference(&fixture.header),
            value: reference(&value),
            map: reference(&interpretation),
        },
    )?;
    let square = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Complex,
        Operator::IntegerPower(2),
        Vec::new(),
        vec![reference(&fixture.complex)],
    )?;
    let right = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Complex,
        Operator::Add,
        Vec::new(),
        vec![reference(&square), reference(&literal)],
    )?;
    Ok(Judgment {
        context: request.context.clone(),
        layer: crate::domain::Layer::Pointwise,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Equality {
            left,
            right,
            domain: original.region,
            domain_contract: crate::proof::DomainContract::DefinedAndEqual,
        },
    })
}
