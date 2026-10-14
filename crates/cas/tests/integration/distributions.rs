// SPDX-License-Identifier: AGPL-3.0-only
//! Original-source calculus, finite point changes, deterministic bytes and expected replay.

use crate::testing::boundary::facts;
use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::distributions::{Fixture, output, replay};
use nrz_cas::domain::Layer;
use nrz_cas::proof::{DistributionStatement, DomainContract, Judgment, Ownership, Statement};
use nrz_cas::request::{BoundaryFact, Outcome, Task};
use nrz_cas::syntax::{CaseName, NamedCase, Operator, PredicateKind};
use nrz_cas::{Cas, Handle, Rational};

/// Complete source coverage precedes elaboration, embedding, differentiation and supported action.
/// ´test:integration:distribution-original-source-lifecycle´
#[test]
fn distribution_original_source_lifecycle() -> TestResult {
    let mut fixture = Fixture::new()?;
    let coordinate = fixture.base.x.clone();
    let nonnegative = fixture.base.predicate(
        PredicateKind::NonNegative,
        std::slice::from_ref(&coordinate),
    )?;
    let negative = fixture
        .base
        .term(Operator::Neg, std::slice::from_ref(&coordinate))?;
    let form = fixture
        .base
        .ordered(&[(nonnegative, coordinate.clone())], Some(negative))?;
    let (function, elaboration) = fixture.base.elaborate(&form)?;
    let bytes = fixture
        .base
        .cas
        .export_witness(&elaboration.witness, &mut budget()?)?;
    Cas::new(budget()?.limits().clone())?.inspect(&bytes, &mut budget()?)?;
    let (embedded, embedding) = fixture.embed(&function)?;
    replay(&fixture, &embedding)?;
    let request = fixture.base.request(
        Task::Differentiate {
            coordinate: "x".into(),
            domain: fixture.base.domain.clone(),
            layer: Layer::Distribution,
            order: 2,
        },
        &[embedded],
    )?;
    let Outcome::Proved { result, usage, .. } = fixture.base.cas.run(&request) else {
        return Err("distribution derivative".into());
    };
    assert!(
        result.replay_usage.condition_steps <= usage.condition_steps.saturating_mul(10).max(10)
    );
    replay(&fixture, &result)?;
    let three = fixture.base.number("3")?;
    let square = fixture
        .base
        .term(Operator::IntegerPower(2), &[coordinate])?;
    let polynomial = fixture.base.term(Operator::Add, &[square, three])?;
    let paired = fixture.pair(&output(&result)?, &polynomial)?;
    assert_eq!(output(&paired)?, fixture.base.number("6")?);
    replay(&fixture, &paired)?;
    Ok(())
}

/// Arbitrary rational step heights remain classically distinct and distributionally equal.
/// ´test:integration:distribution-step-point-values´
#[test]
fn distribution_step_point_values() -> TestResult {
    let mut fixture = Fixture::new()?;
    let first = step(&mut fixture, 0)?;
    let second = step(&mut fixture, 7)?;
    for (function, expected) in [(&first, "0"), (&second, "7")] {
        let location = fixture.base.boundary(&fixture.base.zero.clone())?;
        let request = fixture.base.request(
            Task::Boundary {
                coordinate: "x".into(),
                point: location,
                requested_facts: vec![BoundaryFact::Ownership],
            },
            std::slice::from_ref(function),
        )?;
        let result = fixture.base.proved(&request)?;
        let owner = facts(&result)?
            .iter()
            .find_map(|fact| match &fact.judgment().statement {
                Statement::Ownership {
                    value: Ownership::Present(value),
                    ..
                } => Some(value),
                _ => None,
            })
            .ok_or("owned step value")?;
        assert_eq!(*owner, fixture.base.number(expected)?);
    }
    let (first, _) = fixture.embed(&first)?;
    let (second, _) = fixture.embed(&second)?;
    let request = fixture.base.request(
        Task::Equal {
            domain: fixture.base.domain.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        &[first, second],
    )?;
    let result = fixture.base.proved(&request)?;
    replay(&fixture, &result)?;
    Ok(())
}

/// Unrelated graph admissions cannot perturb canonical distribution proof bytes.
/// ´test:integration:distribution-canonical-bytes´
#[test]
fn distribution_canonical_bytes() -> TestResult {
    let mut encodings = Vec::new();
    for extra in [false, true] {
        let mut fixture = Fixture::new()?;
        if extra {
            fixture.base.number("19")?;
        }
        let function = step(&mut fixture, 7)?;
        let (embedded, _) = fixture.embed(&function)?;
        let result = fixture.derivative(&embedded, 1)?;
        assert_eq!(output(&result)?, fixture.delta("0", 0)?);
        encodings.push(replay(&fixture, &result)?);
    }
    let [first, second] = encodings.as_slice() else {
        return Err("two encodings".into());
    };
    assert_eq!(first, second);
    Ok(())
}

/// A fresh receiver builds the hand-expected theorem before seeing the sender's witness.
/// ´test:integration:distribution-independent-expected-replay´
#[test]
fn distribution_independent_expected_replay() -> TestResult {
    let mut sender = Fixture::new()?;
    let (delta, polynomial) = pairing_inputs(&mut sender)?;
    let result = sender.pair(&delta, &polynomial)?;
    let bytes = sender
        .base
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut receiver = Fixture::new()?;
    let inputs: [_; 2] = pairing_inputs(&mut receiver)?.into();
    let pairing = receiver.node(Operator::PairNearSupport, &inputs)?;
    let expected = Judgment {
        context: receiver.base.context.clone(),
        layer: Layer::Distribution,
        space: Some(receiver.space.clone()),
        residuals: Vec::new(),
        statement: Statement::Distribution(Box::new(DistributionStatement::Pairing {
            pairing,
            value: receiver.base.number("7")?,
            space: receiver.space.clone(),
        })),
    };
    let accepted = receiver
        .base
        .cas
        .replay(&bytes, &expected, &mut budget()?)?;
    assert_eq!(accepted.judgment(), &expected);
    Ok(())
}

/// A supplied classical derivative and zero value jumps retain an explicit bridge expansion.
/// ´test:integration:distribution-classical-bridge-recipe´
#[test]
fn distribution_classical_bridge_recipe() -> TestResult {
    let mut fixture = Fixture::new()?;
    let classical = fixture.base.cas.check_proof(
        &fixture.base.context,
        &[nrz_cas::proof::ApplicationDraft {
            context: fixture.base.context.clone(),
            rule: nrz_cas::proof::Primitive::DifferentiateVariable,
            conclusion: Statement::DerivativeOn {
                function: fixture.base.x.clone(),
                derivative: fixture.base.one.clone(),
                coordinate: "x".into(),
                domain: fixture.base.domain.clone(),
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: nrz_cas::proof::Certificate::None,
        }],
        &mut budget()?,
    )?;
    let coordinate = fixture.base.x.clone();
    let (embedded, _) = fixture.embed(&coordinate)?;
    let mut request = fixture.base.request(
        Task::Differentiate {
            coordinate: "x".into(),
            order: 1,
            layer: Layer::Distribution,
            domain: fixture.base.domain.clone(),
        },
        &[embedded],
    )?;
    request.premises.push(classical);
    let result = fixture.base.proved(&request)?;
    let expected = fixture.node(Operator::RegularEmbed, &[fixture.base.one.clone()])?;
    assert_eq!(output(&result)?, expected);
    let bytes = replay(&fixture, &result)?;
    assert!(
        bytes
            .windows(b"diff.variable".len())
            .any(|window| window == b"diff.variable")
    );
    Ok(())
}

/// A transparent step's point value is an explicit independent parameter.
fn step(fixture: &mut Fixture, height: i64) -> TestResult<Handle> {
    Ok(fixture.base.cas.named_case(
        &fixture.base.domain,
        &NamedCase {
            name: CaseName::Step,
            coordinate: "x".into(),
            parameter: Some(Rational::from_ratio(height, 1, &mut budget()?)?),
        },
        &mut budget()?,
    )?)
}

/// x squared plus three and delta at two are specified without evaluating either action.
fn pairing_inputs(fixture: &mut Fixture) -> TestResult<(Handle, Handle)> {
    let delta = fixture.delta("2", 0)?;
    let coordinate = fixture.base.x.clone();
    let square = fixture
        .base
        .term(Operator::IntegerPower(2), &[coordinate])?;
    let three = fixture.base.number("3")?;
    let polynomial = fixture.base.term(Operator::Add, &[square, three])?;
    Ok((delta, polynomial))
}
