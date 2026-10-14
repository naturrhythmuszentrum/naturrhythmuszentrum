// SPDX-License-Identifier: AGPL-3.0-only
//! Boundary fixtures construct formulas; no production calculus computes expected answers.

use super::budgets::derivation_budget as budget;
use super::cases::Fixture;
use super::common::TestResult;
use crate::test_api::Handle;
use crate::test_api::domain::Layer;
use crate::test_api::proof::{Statement, VerifiedWitness};
use crate::test_api::request::{BoundaryFact, Outcome, ResultPayload, SearchResult, Task};
use crate::test_api::syntax::{CaseName, NamedCase, Operator, PredicateKind};

/// Build an explicitly parameterized named function through the public syntax constructor.
pub(crate) fn named(fixture: &mut Fixture, name: CaseName) -> TestResult<Handle> {
    let definition = NamedCase {
        name,
        coordinate: "x".into(),
        parameter: None,
    };
    Ok(fixture
        .cas
        .named_case(&fixture.domain, &definition, &mut budget()?)?)
}

/// J and Q use hand-written quadratic branches; the right branch owns zero.
pub(crate) fn quadratic(fixture: &mut Fixture, jump: bool) -> TestResult<Handle> {
    let coordinate = fixture.x.clone();
    let square = fixture.term(Operator::IntegerPower(2), std::slice::from_ref(&coordinate))?;
    let right = if jump {
        fixture.term(Operator::Add, &[square.clone(), fixture.one.clone()])?
    } else {
        let two = fixture.number("2")?;
        fixture.term(Operator::Mul, &[two, square.clone()])?
    };
    let nonnegative = fixture.predicate(
        PredicateKind::NonNegative,
        std::slice::from_ref(&coordinate),
    )?;
    let left_region = fixture.region(&nonnegative, false)?;
    let right_region = fixture.region(&nonnegative, true)?;
    fixture.cases(&[(left_region, square), (right_region, right)])
}

/// One exact requested derivative order at zero.
pub(crate) fn point(fixture: &mut Fixture, function: &Handle, order: u32) -> TestResult<Outcome> {
    let location = fixture.boundary(&fixture.zero.clone())?;
    let request = fixture.request(
        Task::Boundary {
            coordinate: "x".into(),
            point: location,
            requested_facts: vec![BoundaryFact::Derivative { order }],
        },
        std::slice::from_ref(function),
    )?;
    Ok(fixture.cas.run(&request))
}

/// An exact whole-axis derivative request.
pub(crate) fn whole(fixture: &mut Fixture, function: &Handle, order: u32) -> TestResult<Outcome> {
    let request = fixture.request(
        Task::Differentiate {
            coordinate: "x".into(),
            order,
            layer: Layer::Pointwise,
            domain: fixture.domain.clone(),
        },
        std::slice::from_ref(function),
    )?;
    Ok(fixture.cas.run(&request))
}

/// Borrow every sealed component without treating a producer's status as a mathematical oracle.
pub(crate) fn facts(result: &SearchResult) -> TestResult<&[VerifiedWitness]> {
    match result.payload.as_deref().ok_or("missing typed result")? {
        ResultPayload::DerivativeMap(value) => Ok(value.facts()),
        ResultPayload::Boundary(value) => Ok(value.facts()),
        ResultPayload::Facts(values)
        | ResultPayload::Distribution(values)
        | ResultPayload::PairingScalar(values) => Ok(values),
    }
}

/// Compare a claimed literal with an independently supplied exact number.
pub(crate) fn literal(fixture: &mut Fixture, actual: &Handle, expected: &str) -> TestResult {
    let expected = fixture.number(expected)?;
    assert_eq!(*actual, expected);
    Ok(())
}

/// Find one point theorem by explicit order, never by the last statement's shape alone.
pub(crate) fn derivative_fact(result: &SearchResult, order: u32) -> TestResult<&Statement> {
    facts(result)?.iter().map(|proof| &proof.judgment().statement)
        .find(|statement| matches!(statement, Statement::DerivativeAt(value) if value.order == order)
            || matches!(statement, Statement::DerivativeDoesNotExist(value) if value.order == order))
        .ok_or_else(|| "missing point derivative fact".into())
}
