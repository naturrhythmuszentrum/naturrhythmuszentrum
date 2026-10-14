// SPDX-License-Identifier: AGPL-3.0-only
//! Independent pendulum model, nominal substitutions and checked domain evidence.
//!
//! Compiled only in the public test target.

use crate::test_api::{Constant, Handle, Operator, Primitive, Substitution};

use super::budgets::derivation_budget as budget;
use super::common::TestResult;
use super::requests::Fixture;
use super::witness::equality;

/// Independently construct one of the three pendulum outputs from its defining formula.
pub(crate) fn pendulum_row(fixture: &mut Fixture, length: &str, time: &str) -> TestResult<Handle> {
    let length = fixture.variable(length)?;
    let time = fixture.variable(time)?;
    let shared_length = fixture.variable("r")?;
    let shared_time = fixture.variable("q")?;
    let one = fixture.number("1")?;
    let four = fixture.number("4")?;
    let pi = fixture.term(Operator::Constant(Constant::Pi), &[])?;
    let pi_squared = fixture.term(Operator::IntegerPower(2), &[pi])?;
    let scale = fixture.term(Operator::Mul, &[four, pi_squared])?;
    let offset = fixture.term(Operator::Add, &[length, shared_length])?;
    let numerator = fixture.term(Operator::Mul, &[scale, offset])?;
    let period = fixture.term(Operator::IntegerPower(2), &[time])?;
    let relative = fixture.term(Operator::Add, &[one, shared_time])?;
    let timing = fixture.term(Operator::IntegerPower(2), &[relative])?;
    let denominator = fixture.term(Operator::Mul, &[period, timing])?;
    fixture.term(Operator::Div, &[numerator, denominator])
}

/// Named coordinates remain separate even when several share the same nominal rational.
pub(crate) fn operating_point(fixture: &mut Fixture) -> TestResult<Vec<Substitution>> {
    let length = fixture.number("621/625")?;
    let time = fixture.number("2")?;
    let zero = fixture.number("0")?;
    let mut substitutions = Vec::new();
    for (key, value) in [
        ("l1", &length),
        ("l2", &length),
        ("l3", &length),
        ("q", &zero),
        ("r", &zero),
        ("t1", &time),
        ("t2", &time),
        ("t3", &time),
    ] {
        substitutions.push(Substitution {
            key: key.into(),
            replacement: value.clone(),
        });
    }
    Ok(substitutions)
}

/// The equality-instantiation schema checks original domain requirements at the operating point.
pub(crate) fn check_point_domain(
    fixture: &mut Fixture,
    input: &Handle,
    point: &[Substitution],
) -> TestResult {
    let substituted = fixture.cas.substitute(input, point, &mut budget()?)?;
    let mut instance = equality(Primitive::Substitute, &substituted, &substituted);
    instance.premises = vec![0];
    instance.substitutions = point.to_vec();
    let proof = fixture.cas.check_proof(
        &fixture.context,
        &[equality(Primitive::Reflexivity, input, input), instance],
        &mut budget()?,
    )?;
    assert!(proof.is_discharged());
    Ok(())
}
