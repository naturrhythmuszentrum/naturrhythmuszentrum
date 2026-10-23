// SPDX-License-Identifier: AGPL-3.0-only
//! An exact LC radicand exercises separate-factor and explicit-quotient sign transport.

mod transport;

use super::{Engine, guard};
use crate::budget::{Budget, Limits};
use crate::domain_model::{AmbientDomain, HeaderDraft, RegionDraft, RegionExpr};
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Declaration, Environment, Field, Operator, PredicateKind, Sort};
use crate::solving::coefficients::{
    fraction::Fraction,
    model::Value,
    polynomial::{Polynomial, Term},
    scalar::{Ground, Scalar},
};
use crate::solving::{
    CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData, SolveDraft,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn admit(cas: &mut Cas, draft: Draft, budget: &mut Budget) -> TestResult<Handle> {
    cas.admit(&[draft], budget)?
        .pop()
        .ok_or_else(|| "LC sign fixture".into())
}

fn base(cas: &mut Cas, budget: &mut Budget) -> TestResult<(Handle, Handle, Handle)> {
    let declarations = ["L", "C"]
        .into_iter()
        .map(|name| Declaration {
            key: name.into(),
            display_name: name.into(),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
        })
        .collect();
    let header = admit(
        cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations,
            ..Environment::default()
        })),
        budget,
    )?;
    let coordinates = ["L", "C"]
        .into_iter()
        .map(|name| Coordinate {
            key: name.into(),
            field: Field::Real,
            role: CoordinateRole::Coefficient,
        })
        .collect();
    let coordinates = admit(
        cas,
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(header.clone()),
            scalar_spaces: Vec::new(),
            coordinates,
        })),
        budget,
    )?;
    let region = admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: coordinates.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
        budget,
    )?;
    let ground = admit(
        cas,
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
        budget,
    )?;
    let domain = admit(
        cas,
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Fraction {
                base: Reference::Existing(ground),
                coordinates: Reference::Existing(coordinates),
            },
        )),
        budget,
    )?;
    Ok((header, region, domain))
}

fn expression(
    cas: &mut Cas,
    header: &Handle,
    operator: Operator,
    operands: &[Handle],
    budget: &mut Budget,
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Expression(ExpressionDraft {
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            binding: None,
            space: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        }),
        budget,
    )
}

fn positive(
    cas: &mut Cas,
    header: &Handle,
    region: &Handle,
    subject: &Handle,
    budget: &mut Budget,
) -> TestResult<Handle> {
    let predicate = admit(
        cas,
        Draft::Condition {
            kind: PredicateKind::Positive,
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(subject.clone())],
        },
        budget,
    )?;
    let ambient = cas.state.region(region.logical_id())?.ambient.clone();
    admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient,
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(region.clone()),
                predicate: Reference::Existing(predicate),
            },
        }),
        budget,
    )
}

fn radicand(
    cas: &Cas,
    domain: &Handle,
    budget: &mut Budget,
) -> TestResult<crate::solving::coefficients::model::Element> {
    let one = Scalar::constant(Ground::Rational, true, budget)?;
    let negative = one.neg(budget)?;
    let numerator = Polynomial::new(
        2,
        Ground::Rational,
        vec![Term {
            powers: vec![0, 0],
            coefficient: negative,
        }],
        budget,
    )?;
    let denominator = Polynomial::new(
        2,
        Ground::Rational,
        vec![Term {
            powers: vec![1, 1],
            coefficient: one,
        }],
        budget,
    )?;
    let fraction = Fraction::new(&numerator, &denominator, budget)?;
    Ok(Engine {
        state: &cas.state,
        domain,
    }
    .element(Value::Fraction(fraction)))
}

/// Separate positive L and C guards and an explicit reciprocal guard both yield supported proposals.
/// ´test:unit:lc-reciprocal-sign-producer-gap´
#[test]
fn lc_reciprocal_sign_producer_gap() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let mut budget = Budget::new(Limits::default())?;
    let (header, mut region, domain) = base(&mut cas, &mut budget)?;
    let inductance = expression(
        &mut cas,
        &header,
        Operator::Free("L".into()),
        &[],
        &mut budget,
    )?;
    let capacitance = expression(
        &mut cas,
        &header,
        Operator::Free("C".into()),
        &[],
        &mut budget,
    )?;
    region = positive(&mut cas, &header, &region, &inductance, &mut budget)?;
    region = positive(&mut cas, &header, &region, &capacitance, &mut budget)?;
    let value = radicand(&cas, &domain, &mut budget)?;
    assert!(guard(&mut cas, &region, &value, &mut budget)?.is_some());
    let product = expression(
        &mut cas,
        &header,
        Operator::Mul,
        &[inductance, capacitance],
        &mut budget,
    )?;
    let one = expression(
        &mut cas,
        &header,
        Operator::Rational(crate::Rational::one()),
        &[],
        &mut budget,
    )?;
    let positive_reciprocal = expression(
        &mut cas,
        &header,
        Operator::Div,
        &[one, product],
        &mut budget,
    )?;
    region = positive(
        &mut cas,
        &header,
        &region,
        &positive_reciprocal,
        &mut budget,
    )?;
    assert!(guard(&mut cas, &region, &value, &mut budget)?.is_some());
    Ok(())
}
