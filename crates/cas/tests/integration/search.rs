// SPDX-License-Identifier: AGPL-3.0-only
//! Complete public-API client flows for search.

use nrz_cas::limits::LimitsOverride;
use nrz_cas::proof::{GuardKind, Primitive, Statement};
use nrz_cas::request::Task;
use nrz_cas::syntax::{Constant, Operator};

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::pendulum::{check_point_domain, operating_point, pendulum_row};
use crate::testing::requests::Fixture;
use crate::testing::witness::{completed, equal, equality, replay};

/// A shared square differentiates to the hand-derived 2x and replays without producer state.
/// ´test:integration:generated-square-derivative´
#[test]
fn generated_square_derivative() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let square = fixture.term(Operator::Mul, &[variable.clone(), variable.clone()])?;
    let two = fixture.number("2")?;
    let expected = fixture.term(Operator::Mul, &[two, variable])?;
    let request = fixture.request(
        Task::Differentiate {
            coordinate: "x".into(),
            order: 1,
            layer: nrz_cas::domain::Layer::Pointwise,
            domain: fixture.domain.clone(),
        },
        &[square],
    )?;
    let result = completed(fixture.cas.run(&request))?;
    assert!(result.witness.is_discharged());
    equal(
        &mut fixture,
        crate::testing::witness::right(result.witness.judgment())?,
        &expected,
    )?;
    assert!(
        matches!(&result.witness.judgment().statement, Statement::DerivativeOn { coordinate,
        domain, .. } if coordinate == "x" && *domain == fixture.domain)
    );
    replay(&fixture, &result)?;
    Ok(())
}

/// An actual larger intermediate appears before the difference quotient reaches 2x+h.
/// ´test:integration:generated-difference-quotient´
#[test]
fn generated_difference_quotient() -> TestResult {
    let mut fixture = Fixture::new(&["x", "h"])?;
    let variable = fixture.variable("x")?;
    let increment = fixture.variable("h")?;
    let sum = fixture.term(Operator::Add, &[variable.clone(), increment.clone()])?;
    let raised = fixture.term(Operator::IntegerPower(2), &[sum])?;
    let square = fixture.term(Operator::IntegerPower(2), std::slice::from_ref(&variable))?;
    let difference = fixture.term(Operator::Sub, &[raised, square])?;
    let quotient = fixture.term(Operator::Div, &[difference, increment.clone()])?;
    let two = fixture.number("2")?;
    let twice = fixture.term(Operator::Mul, &[two, variable])?;
    let expected = fixture.term(Operator::Add, &[twice, increment.clone()])?;
    let mut request = fixture.request(
        Task::Equal {
            domain: fixture.domain.clone(),
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        },
        &[quotient, expected.clone()],
    )?;
    request.limits = LimitsOverride::default();
    let result = completed(fixture.cas.run(&request))?;
    assert_eq!(
        crate::testing::witness::right(result.witness.judgment())?.clone(),
        expected
    );
    assert!(result.expansion_credits > 0);
    assert!(result.trace.len() > 2);
    assert_eq!(result.witness.judgment().residuals.len(), 1);
    let guard = result
        .witness
        .judgment()
        .residuals
        .first()
        .ok_or("missing increment guard")?;
    assert_eq!(
        *guard,
        crate::testing::witness::guarded(
            &mut fixture.cas,
            &fixture.context,
            GuardKind::NonZero,
            &increment
        )?
    );
    replay(&fixture, &result)?;
    Ok(())
}

/// A derived equality is expanded from graph proof records and rechecked during search.
/// ´test:integration:search-reuses-checked-shortcut´
#[test]
fn search_reuses_checked_shortcut() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let zero = fixture.number("0")?;
    let sum = fixture.term(Operator::Add, &[variable.clone(), zero])?;
    let proof = fixture.cas.check_proof(
        &fixture.context,
        &[equality(
            &fixture.context,
            &fixture.domain,
            Primitive::Jets,
            &sum,
            &variable,
        )],
        &mut budget()?,
    )?;
    fixture
        .cas
        .register_derived(&proof, proof.judgment(), 1, &mut budget()?)?;
    let request = fixture.request(
        Task::Equal {
            domain: fixture.domain.clone(),
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        },
        &[sum, variable],
    )?;
    let result = completed(fixture.cas.run(&request))?;
    assert_eq!(result.cost, 1);
    replay(&fixture, &result)?;
    Ok(())
}

/// Three by eight exact partials preserve the shared final two columns and nominal coefficients.
/// ´test:integration:pendulum-shared-jacobian´
#[test]
fn pendulum_shared_jacobian() -> TestResult {
    let keys = ["l1", "l2", "l3", "t1", "t2", "t3", "r", "q"];
    let mut fixture = Fixture::new(&keys)?;
    let mut outputs = Vec::new();
    for (length, time) in [("l1", "t1"), ("l2", "t2"), ("l3", "t3")] {
        outputs.push(pendulum_row(&mut fixture, length, time)?);
    }
    let variables = keys.iter().map(|key| (*key).to_owned()).collect();
    let request = fixture.request(Task::Jacobian(variables), &outputs)?;
    let result = completed(fixture.cas.run(&request))?;
    assert_eq!(
        *fixture
            .cas
            .expression(crate::testing::witness::right(result.witness.judgment())?)?
            .0,
        Operator::Matrix {
            rows: 3,
            columns: 8
        }
    );
    assert_ne!(
        result.witness.judgment().residuals,
        [] as [nrz_cas::proof::GuardedObligation; 0]
    );
    assert!(result.witness.judgment().residuals.iter().all(|guard| {
        fixture
            .cas
            .predicate_kind(&guard.predicate)
            .is_ok_and(|kind| *kind == nrz_cas::syntax::PredicateKind::NonZero)
            && guard.home == fixture.context
            && guard.region == fixture.domain
            && guard.scope.is_empty()
            && guard.layer == nrz_cas::domain::Layer::Pointwise
            && guard.space.is_none()
    }));
    replay(&fixture, &result)?;
    let cells = fixture.cas.operands(
        crate::testing::witness::right(result.witness.judgment())?,
        &mut budget()?,
    )?;
    assert_eq!(cells.len(), 24);
    let point = operating_point(&mut fixture)?;
    for input in &outputs {
        check_point_domain(&mut fixture, input, &point)?;
    }
    let pi = fixture.term(Operator::Constant(Constant::Pi), &[])?;
    let pi_squared = fixture.term(Operator::IntegerPower(2), &[pi])?;
    let timing = fixture.number("-621/625")?;
    let common_timing = fixture.number("-1242/625")?;
    let zero = fixture.number("0")?;
    let time_cell = fixture.term(Operator::Mul, &[timing, pi_squared.clone()])?;
    let common_cell = fixture.term(Operator::Mul, &[common_timing, pi_squared.clone()])?;
    for (position, cell) in cells.iter().enumerate() {
        let row = position / 8;
        let column = position % 8;
        let expected = if column == row || column == 6 {
            &pi_squared
        } else if column == row + 3 {
            &time_cell
        } else if column == 7 {
            &common_cell
        } else {
            &zero
        };
        let specialized = fixture.cas.substitute(cell, &point, &mut budget()?)?;
        equal(&mut fixture, &specialized, expected)?;
    }
    Ok(())
}

/// Polynomial expansion produces the independently supplied square coefficients.
/// ´test:integration:generated-polynomial-expansion´
#[test]
fn generated_polynomial_expansion() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let one = fixture.number("1")?;
    let two = fixture.number("2")?;
    let shifted = fixture.term(Operator::Add, &[variable.clone(), one.clone()])?;
    let input = fixture.term(Operator::IntegerPower(2), &[shifted])?;
    let squared = fixture.term(Operator::IntegerPower(2), std::slice::from_ref(&variable))?;
    let linear = fixture.term(Operator::Mul, &[two, variable])?;
    let tail = fixture.term(Operator::Add, &[linear, squared])?;
    let expected = fixture.term(Operator::Add, &[one, tail])?;
    let request = fixture.request(Task::Expand, &[input])?;
    let result = completed(fixture.cas.run(&request))?;
    equal(
        &mut fixture,
        crate::testing::witness::right(result.witness.judgment())?,
        &expected,
    )?;
    replay(&fixture, &result)?;
    Ok(())
}
