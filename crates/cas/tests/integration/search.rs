// SPDX-License-Identifier: AGPL-3.0-only
//! Complete public-API client flows for search.

use nrz_cas::{Constant, GuardKind, LimitsOverride, Operation, Operator, Primitive, Relation};

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
    let request = fixture.request(Operation::Differentiate("x".into()), &[square])?;
    let result = completed(fixture.cas.run(&request))?;
    assert!(result.witness.is_discharged());
    equal(&mut fixture, &result.witness.claim().right, &expected)?;
    assert_eq!(
        result.witness.claim().relation,
        Relation::Derivative {
            variable: "x".into()
        }
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
    let mut request = fixture.request(Operation::Equal, &[quotient, expected.clone()])?;
    request.limits = LimitsOverride::default();
    let result = completed(fixture.cas.run(&request))?;
    assert_eq!(result.witness.claim().right, expected);
    assert!(result.expansion_credits > 0);
    assert!(result.trace.len() > 2);
    assert_eq!(result.witness.claim().residuals.len(), 1);
    let guard = result
        .witness
        .claim()
        .residuals
        .first()
        .ok_or("missing increment guard")?;
    assert_eq!(guard.kind, GuardKind::NonZero);
    assert_eq!(guard.term, increment);
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
        &[equality(Primitive::Jets, &sum, &variable)],
        &mut budget()?,
    )?;
    fixture
        .cas
        .register_derived(&proof, proof.claim(), 1, &mut budget()?)?;
    let request = fixture.request(Operation::Equal, &[sum, variable])?;
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
    let request = fixture.request(Operation::Jacobian(variables), &outputs)?;
    let result = completed(fixture.cas.run(&request))?;
    assert_eq!(
        *fixture.cas.expression(&result.witness.claim().right)?.0,
        Operator::Matrix {
            rows: 3,
            columns: 8
        }
    );
    assert_ne!(result.witness.claim().residuals, [] as [nrz_cas::Guard; 0]);
    assert!(
        result
            .witness
            .claim()
            .residuals
            .iter()
            .all(|guard| guard.kind == GuardKind::NonZero)
    );
    replay(&fixture, &result)?;
    let cells = fixture
        .cas
        .operands(&result.witness.claim().right, &mut budget()?)?;
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
    let request = fixture.request(Operation::Expand, &[input])?;
    let result = completed(fixture.cas.run(&request))?;
    equal(&mut fixture, &result.witness.claim().right, &expected)?;
    replay(&fixture, &result)?;
    Ok(())
}
