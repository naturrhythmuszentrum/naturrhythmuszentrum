// SPDX-License-Identifier: AGPL-3.0-only
//! Complete original source, traversal, elaboration and expected fresh replay flows.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use nrz_cas::domain::Layer;
use nrz_cas::proof::{DomainContract, Judgment, Ownership, Statement};
use nrz_cas::request::{BoundaryFact, Outcome, Task};
use nrz_cas::source::{SyntaxNode, WalkEvent};
use nrz_cas::syntax::{Operator, PredicateKind};
use nrz_cas::{FaultKind, InvalidCode};

/// A filled reciprocal is total, and its source elaboration survives expected fresh replay.
/// ´test:integration:original-filled-reciprocal-flow´
#[test]
fn original_filled_reciprocal_flow() -> TestResult {
    let mut f = Fixture::new()?;
    let zero = f.predicate(PredicateKind::Equal, &[f.x.clone(), f.zero.clone()])?;
    let reciprocal = f.term(Operator::Div, &[f.one.clone(), f.x.clone()])?;
    let form = f.ordered(&[(zero, f.zero.clone())], Some(reciprocal))?;
    let (cases, elaboration) = f.elaborate(&form)?;
    let request = f.request(
        Task::Coverage {
            domain: f.domain.clone(),
        },
        std::slice::from_ref(&cases),
    )?;
    f.proved(&request)?;
    let bytes = f.cas.export_witness(&elaboration.witness, &mut budget()?)?;
    let mut receiver = Fixture::new()?;
    let zero = receiver.predicate(
        PredicateKind::Equal,
        &[receiver.x.clone(), receiver.zero.clone()],
    )?;
    let reciprocal = receiver.term(Operator::Div, &[receiver.one.clone(), receiver.x.clone()])?;
    let original = receiver.ordered(
        &[(zero.clone(), receiver.zero.clone())],
        Some(reciprocal.clone()),
    )?;
    let at_zero = receiver.region(&zero, true)?;
    let elsewhere = receiver.region(&zero, false)?;
    let expected_cases =
        receiver.cases(&[(at_zero, receiver.zero.clone()), (elsewhere, reciprocal)])?;
    let expected = Judgment {
        context: receiver.context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Elaboration {
            source: original.syntax,
            result: expected_cases,
            domain: receiver.domain.clone(),
        },
        residuals: Vec::new(),
    };
    let imported = receiver.cas.replay(&bytes, &expected, &mut budget()?)?;
    assert_eq!(
        receiver.cas.export_witness(&imported, &mut budget()?)?,
        bytes
    );
    Ok(())
}

/// Inactive division by zero remains visible in the complete original walk.
/// ´test:integration:inactive-partial-case-complete-walk´
#[test]
fn inactive_partial_case_complete_walk() -> TestResult {
    let mut f = Fixture::new()?;
    let always = f.predicate(PredicateKind::NonZero, &[f.one.clone()])?;
    let bad = f.term(Operator::Div, &[f.one.clone(), f.zero.clone()])?;
    let undefined_guard = f.predicate(PredicateKind::Positive, std::slice::from_ref(&bad))?;
    let form = f.ordered(
        &[(always, f.one.clone()), (undefined_guard, bad.clone())],
        Some(bad.clone()),
    )?;
    let walk = f.cas.walk_original(&form.root, &mut budget()?)?;
    let mut inactive_uses = 0;
    for event in walk.events {
        if let WalkEvent::Visit(visit) = event {
            if visit.expression == bad {
                inactive_uses += 1;
            }
            let descriptor = f.cas.describe_syntax(&visit.expression, &mut budget()?)?;
            if visit.expression == form.syntax {
                assert!(matches!(descriptor.node, SyntaxNode::SourceForm(_)));
            }
        }
    }
    assert_eq!(inactive_uses, 3);
    assert!(walk.receipt.is_original());
    let (cases, _) = f.elaborate(&form)?;
    let request = f.request(
        Task::Equal {
            domain: f.domain.clone(),
            domain_contract: DomainContract::SameDefinedDomain,
        },
        &[cases, f.one.clone()],
    )?;
    f.proved(&request)?;
    Ok(())
}

/// A reached undefined predicate blocks elaboration even when an otherwise value is total.
/// ´test:integration:undefined-reached-guard-no-fallthrough´
#[test]
fn undefined_reached_guard_no_fallthrough() -> TestResult {
    let mut f = Fixture::new()?;
    let bad = f.term(Operator::Div, &[f.one.clone(), f.zero.clone()])?;
    let guard = f.predicate(PredicateKind::Positive, &[bad])?;
    let form = f.ordered(&[(guard, f.zero.clone())], Some(f.one.clone()))?;
    let request = f.elaborate_request(&form)?;
    assert!(matches!(f.cas.run(&request), Outcome::Refused { fault, .. }
        if fault.kind == FaultKind::Invalid(InvalidCode::RefutedDomain)));
    let root = f
        .cas
        .materialize_derived(&form.syntax, &form.syntax, &mut budget()?)?;
    let receipt = f.cas.walk_derived(&root, &mut budget()?)?.receipt;
    let mut generated = request;
    generated.inputs = vec![nrz_cas::request::CoveredInput { root, receipt }];
    assert!(
        matches!(f.cas.run(&generated), Outcome::Invalid { fault, .. }
        if fault.kind == FaultKind::Invalid(InvalidCode::Receipt))
    );
    Ok(())
}

/// An original filled quotient merges to one and replays against an independently built target.
/// ´test:integration:original-merge-expected-replay´
#[test]
fn original_merge_expected_replay() -> TestResult {
    let mut f = Fixture::new()?;
    let nz = f.predicate(PredicateKind::NonZero, &[f.x.clone()])?;
    let quotient = f.term(Operator::Div, &[f.x.clone(), f.x.clone()])?;
    let form = f.ordered(&[(nz.clone(), quotient)], Some(f.one.clone()))?;
    let (cases, _) = f.elaborate(&form)?;
    let nonzero = f.region(&nz, true)?;
    let zero = f.region(&nz, false)?;
    let request = f.request(
        Task::Merge {
            regions: vec![nonzero, zero],
            proposed_value: f.one.clone(),
        },
        &[cases],
    )?;
    let result = f.proved(&request)?;
    let bytes = f.cas.export_witness(&result.witness, &mut budget()?)?;
    let mut receiver = Fixture::new()?;
    let nz = receiver.predicate(PredicateKind::NonZero, &[receiver.x.clone()])?;
    let quotient = receiver.term(Operator::Div, &[receiver.x.clone(), receiver.x.clone()])?;
    let nonzero = receiver.region(&nz, true)?;
    let zero = receiver.region(&nz, false)?;
    let cases = receiver.cases(&[(nonzero, quotient), (zero, receiver.one.clone())])?;
    let expected = Judgment {
        context: receiver.context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: cases,
            right: receiver.one.clone(),
            domain: receiver.domain.clone(),
            domain_contract: DomainContract::SameDefinedDomain,
        },
        residuals: Vec::new(),
    };
    receiver.cas.replay(&bytes, &expected, &mut budget()?)?;
    Ok(())
}

/// Original singleton ownership has deterministic bytes and an independently constructed target.
/// ´test:integration:original-ownership-expected-replay´
#[test]
fn original_ownership_expected_replay() -> TestResult {
    let mut f = Fixture::new()?;
    let zero = f.predicate(PredicateKind::Equal, &[f.x.clone(), f.zero.clone()])?;
    let form = f.ordered(&[(zero, f.one.clone())], Some(f.zero.clone()))?;
    let (function, _) = f.elaborate(&form)?;
    let point = f.boundary(&f.zero.clone())?;
    let request = f.request(
        Task::Boundary {
            coordinate: "x".into(),
            point,
            requested_facts: vec![BoundaryFact::Ownership],
        },
        &[function],
    )?;
    let first = f.proved(&request)?;
    let second = f.proved(&request)?;
    let bytes = f.cas.export_witness(&first.witness, &mut budget()?)?;
    assert_eq!(
        f.cas.export_witness(&second.witness, &mut budget()?)?,
        bytes
    );
    let mut receiver = Fixture::new()?;
    let zero = receiver.predicate(
        PredicateKind::Equal,
        &[receiver.x.clone(), receiver.zero.clone()],
    )?;
    let singleton = receiver.region(&zero, true)?;
    let punctured = receiver.region(&zero, false)?;
    let function = receiver.cases(&[
        (singleton, receiver.one.clone()),
        (punctured, receiver.zero.clone()),
    ])?;
    let boundary = receiver.boundary(&receiver.zero.clone())?;
    let expected = Judgment {
        context: receiver.context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Ownership {
            function,
            boundary,
            value: Ownership::Present(receiver.one.clone()),
        },
        residuals: Vec::new(),
    };
    let replayed = receiver.cas.replay(&bytes, &expected, &mut budget()?)?;
    assert_eq!(
        receiver.cas.export_witness(&replayed, &mut budget()?)?,
        bytes
    );
    Ok(())
}
