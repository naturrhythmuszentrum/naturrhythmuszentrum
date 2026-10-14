// SPDX-License-Identifier: AGPL-3.0-only
//! Complete public-API client flows for proofs.

use nrz_cas::{Cas, FaultKind, InvalidCode};

use nrz_cas::proof::{GuardKind, Judgment, Primitive, Statement};
use nrz_cas::syntax::{Operator, PredicateKind};

use crate::testing::budgets::proof_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::proofs::{Fixture, cancellation};
use crate::testing::wire::canonical_fixture;
use crate::testing::witness::{derivative, equality};

/// Hypotheses discharge guards relative to their exact context and keep provenance visible.
/// ´test:integration:assumed-cancellation´
#[test]
fn assumed_cancellation() -> TestResult {
    let (mut fixture, _, mut step) = cancellation()?;
    fixture.assume("positive-x", PredicateKind::Positive, &fixture.x.clone())?;
    step.context = fixture.context.clone();
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    assert!(witness.is_discharged());
    assert_eq!(
        witness.hypotheses(),
        &[nrz_cas::domain::HypothesisRef {
            context: fixture.context.clone(),
            slot: 0
        }]
    );
    let reference = witness
        .hypotheses()
        .first()
        .ok_or("missing root hypothesis")?;
    assert_eq!(fixture.cas.hypothesis_name(reference)?, Some("positive-x"));
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let (mut fresh, quotient, _) = cancellation()?;
    fresh.assume("positive-x", PredicateKind::Positive, &fresh.x.clone())?;
    let expected = Judgment {
        context: fresh.context.clone(),
        layer: nrz_cas::domain::Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: quotient,
            right: fresh.one.clone(),
            domain: fresh.domain.clone(),
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    };
    let replayed = fresh.cas.replay(&bytes, &expected, &mut budget()?)?;
    assert_eq!(fresh.cas.export_witness(&replayed, &mut budget()?)?, bytes);
    assert_eq!(
        replayed.hypotheses(),
        &[nrz_cas::domain::HypothesisRef {
            context: fresh.context.clone(),
            slot: 0
        }]
    );
    Ok(())
}

/// Supplied product-rule premises derive 2x, while differentiation never becomes equality.
/// ´test:integration:checked-derivative-premises´
#[test]
fn checked_derivative_premises() -> TestResult {
    let mut fixture = Fixture::new()?;
    let square = fixture.term(Operator::Mul, &[fixture.x.clone(), fixture.x.clone()])?;
    let first = fixture.term(Operator::Mul, &[fixture.one.clone(), fixture.x.clone()])?;
    let second = fixture.term(Operator::Mul, &[fixture.x.clone(), fixture.one.clone()])?;
    let sum = fixture.term(Operator::Add, &[first, second])?;
    let target = fixture.term(Operator::Mul, &[fixture.two.clone(), fixture.x.clone()])?;
    let steps = vec![
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DifferentiateVariable,
            &fixture.x,
            &fixture.one,
            &[],
        ),
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DifferentiateProduct,
            &square,
            &sum,
            &[0, 0],
        ),
        equality(
            &fixture.context,
            &fixture.domain,
            Primitive::Jets,
            &sum,
            &target,
        ),
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DerivativeTransport,
            &square,
            &target,
            &[1, 2],
        ),
    ];
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &steps, &mut budget()?)?;
    assert!(witness.is_discharged());
    assert!(
        matches!(&witness.judgment().statement, Statement::DerivativeOn { coordinate, domain, ..
        } if coordinate == "x" && *domain == fixture.domain)
    );
    let mut bad = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Symmetry,
        &fixture.one,
        &fixture.x,
    );
    bad.premises = vec![0];
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[steps.first().ok_or("missing derivative")?.clone(), bad],
                &mut budget()?,
            )
            .is_err()
    );
    let mut missing = steps.get(1).ok_or("missing product")?.clone();
    missing.premises = vec![0];
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[steps.first().ok_or("missing variable")?.clone(), missing],
                &mut budget()?,
            )
            .is_err()
    );
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut fresh = Cas::new(budget()?.limits().clone())?;
    let inspected = fresh.inspect(&bytes, &mut budget()?)?;
    assert_eq!(fresh.export_witness(&inspected, &mut budget()?)?, bytes);
    Ok(())
}

/// A canonical witness replays in a fresh store, and a different intended claim is rejected.
/// ´test:integration:expected-claim-replay´
#[test]
fn expected_claim_replay() -> TestResult {
    let (mut producer, _, step) = cancellation()?;
    let witness = producer
        .cas
        .check_proof(&producer.context, &[step], &mut budget()?)?;
    let bytes = producer.cas.export_witness(&witness, &mut budget()?)?;
    assert_eq!(bytes, canonical_fixture()?);
    let (mut receiver, quotient, _) = cancellation()?;
    let expected = Judgment {
        context: receiver.context.clone(),
        layer: nrz_cas::domain::Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: quotient,
            right: receiver.one.clone(),
            domain: receiver.domain.clone(),
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        },
        residuals: vec![crate::testing::witness::guarded(
            &mut receiver.cas,
            &receiver.context,
            GuardKind::NonZero,
            &(receiver.x.clone()),
        )?],
    };
    let replayed = receiver.cas.replay(&bytes, &expected, &mut budget()?)?;
    assert_eq!(
        receiver.cas.export_witness(&replayed, &mut budget()?)?,
        bytes
    );
    let before = receiver.cas.canonical_snapshot(&mut budget()?)?;
    let mut wrong = expected.clone();
    let Statement::Equality { right, .. } = &mut wrong.statement else {
        return Err("expected equality".into());
    };
    *right = receiver.zero.clone();
    let fault = receiver
        .cas
        .replay(&bytes, &wrong, &mut budget()?)
        .err()
        .ok_or("accepted wrong endpoint")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    wrong = expected;
    wrong.residuals.clear();
    assert!(receiver.cas.replay(&bytes, &wrong, &mut budget()?).is_err());
    assert_eq!(receiver.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Quotient premises reconstruct the full quotient rule and reduce its power guard to y nonzero.
/// ´test:integration:checked-quotient-derivative´
#[test]
fn checked_quotient_derivative() -> TestResult {
    let mut fixture = Fixture::new()?;
    let quotient = fixture.term(Operator::Div, &[fixture.x.clone(), fixture.y.clone()])?;
    let left_product = fixture.term(Operator::Mul, &[fixture.one.clone(), fixture.y.clone()])?;
    let right_product = fixture.term(Operator::Mul, &[fixture.x.clone(), fixture.zero.clone()])?;
    let numerator = fixture.term(Operator::Sub, &[left_product, right_product])?;
    let denominator = fixture.term(Operator::IntegerPower(2), &[fixture.y.clone()])?;
    let derivative_value = fixture.term(Operator::Div, &[numerator, denominator])?;
    let simplified = fixture.term(Operator::Div, &[fixture.one.clone(), fixture.y.clone()])?;
    let steps = [
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DifferentiateVariable,
            &fixture.x,
            &fixture.one,
            &[],
        ),
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DifferentiateVariable,
            &fixture.y,
            &fixture.zero,
            &[],
        ),
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DifferentiateQuotient,
            &quotient,
            &derivative_value,
            &[0, 1],
        ),
        equality(
            &fixture.context,
            &fixture.domain,
            Primitive::RationalIdentity,
            &derivative_value,
            &simplified,
        ),
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DerivativeTransport,
            &quotient,
            &simplified,
            &[2, 3],
        ),
    ];
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &steps, &mut budget()?)?;
    assert_eq!(
        witness.judgment().residuals,
        vec![crate::testing::witness::guarded(
            &mut fixture.cas,
            &fixture.context,
            GuardKind::NonZero,
            &(fixture.y.clone())
        )?],
    );
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut fresh = Cas::new(budget()?.limits().clone())?;
    assert!(!fresh.inspect(&bytes, &mut budget()?)?.is_discharged());
    Ok(())
}
