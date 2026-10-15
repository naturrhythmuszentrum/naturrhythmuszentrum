// SPDX-License-Identifier: AGPL-3.0-only
//! Refusals, exact root ceilings and independent rejection of hostile pullback certificates.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::pullbacks::{Fixture, roots};
use nrz_cas::limits::BudgetKind;
use nrz_cas::proof::{ApplicationDraft, Certificate, DistributionStatement, Primitive, Statement};
use nrz_cas::request::{Outcome, PullbackKind, RefusalReason};
use nrz_cas::{FaultKind, InvalidCode, Rational};

/// Multiple, incomplete, repeated or unordered roots cannot establish a simple-zero formula.
/// ´test:integration:pullback-root-hypothesis-refusals´
#[test]
fn pullback_root_hypothesis_refusals() -> TestResult {
    for (repeated, supplied) in [
        (true, vec!["1"]),
        (false, vec!["1"]),
        (false, vec!["-1", "-1", "1"]),
        (false, vec!["1", "-1"]),
    ] {
        let mut fixture = Fixture::new()?;
        let source = fixture.source()?;
        let map = fixture.quadratic(repeated)?;
        let request = fixture
            .base
            .request(fixture.task(roots(&supplied)?), &[source, map])?;
        assert!(matches!(
            fixture.base.cas.run(&request),
            Outcome::Refused {
                reason: RefusalReason::PullbackHypotheses,
                ..
            }
        ));
    }
    let mut fixture = Fixture::new()?;
    let source = fixture.source()?;
    let (map, _) = fixture.affine("0", "1")?;
    let request = fixture
        .base
        .request(fixture.task(PullbackKind::Affine), &[source, map])?;
    assert!(matches!(
        fixture.base.cas.run(&request),
        Outcome::Refused {
            reason: RefusalReason::PullbackHypotheses,
            ..
        }
    ));
    Ok(())
}

/// The zero ceiling is a peak: affine needs one, the quadratic needs two, including fresh replay.
/// ´test:integration:pullback-exact-zero-budget´
#[test]
fn pullback_exact_zero_budget() -> TestResult {
    for simple in [false, true] {
        let exact = if simple { 2 } else { 1 };
        for cap in [exact, exact - 1] {
            let mut fixture = Fixture::new()?;
            let source = fixture.source()?;
            let (map, intent) = if simple {
                (fixture.quadratic(false)?, roots(&["-1", "1"])?)
            } else {
                (fixture.affine("2", "-1")?.0, PullbackKind::Affine)
            };
            let mut request = fixture.base.request(fixture.task(intent), &[source, map])?;
            request.limits.pullback_zeros = Some(cap);
            let outcome = fixture.base.cas.run(&request);
            if cap == exact {
                assert!(matches!(outcome, Outcome::Proved { .. }));
            } else {
                assert!(
                    matches!(outcome, Outcome::Exhausted { fault, .. } if matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::PullbackZeros, .. }))
                );
            }
        }
    }
    Ok(())
}

/// Signed slopes, support points, leading coefficients and reciprocal absolute Jacobians are checked.
/// ´test:integration:pullback-certificate-tampering´
#[test]
fn pullback_certificate_tampering() -> TestResult {
    let mut fixture = Fixture::new()?;
    let source = fixture.source()?;
    let map = fixture.quadratic(false)?;
    let request = fixture
        .base
        .request(fixture.task(roots(&["-1", "1"])?), &[source, map])?;
    let result = fixture.base.proved(&request)?;
    let evidence = fixture
        .base
        .cas
        .pullback_certificate(&result.witness)?
        .clone();
    for mutation in 0..5 {
        let mut changed = evidence.clone();
        let zero = changed.zeros.first_mut().ok_or("first zero")?;
        match mutation {
            0 => zero.point = Rational::zero(),
            1 => zero.slope = Rational::one(),
            2 => zero.inverse_jacobian = Rational::one(),
            3 => changed.leading = Rational::zero(),
            _ => {
                let _removed = changed.zeros.pop();
            }
        }
        let draft = application(
            &fixture,
            Primitive::DistSimpleZeroPullback,
            result.witness.judgment().statement.clone(),
            Certificate::Pullback(Box::new(changed)),
        );
        assert!(
            fixture
                .base
                .cas
                .check_proof(&fixture.base.context, &[draft], &mut budget()?)
                .is_err()
        );
    }
    Ok(())
}

/// The actual inverse, map, source space and delta-sum output remain mathematical dependencies.
/// ´test:integration:pullback-map-and-space-tampering´
#[test]
fn pullback_map_and_space_tampering() -> TestResult {
    let mut fixture = Fixture::new()?;
    let source = fixture.source()?;
    let (map, _) = fixture.affine("-3", "6")?;
    let request = fixture
        .base
        .request(fixture.task(PullbackKind::Affine), &[source, map])?;
    let result = fixture.base.proved(&request)?;
    let evidence = fixture
        .base
        .cas
        .pullback_certificate(&result.witness)?
        .clone();
    for mutation in 0..4 {
        let mut claim = result.witness.judgment().statement.clone();
        let Statement::Distribution(statement) = &mut claim else {
            return Err("distribution".into());
        };
        let DistributionStatement::Pullback(value) = statement.as_mut() else {
            return Err("pullback".into());
        };
        match mutation {
            0 => value.inverse = Some(fixture.y.clone()),
            1 => value.map.clone_from(&fixture.base.x),
            2 => value.source_space.clone_from(&fixture.target_space),
            _ => value.result = fixture.weighted(&[("2", "-1/3")])?,
        }
        let draft = application(
            &fixture,
            Primitive::DistAffinePullback,
            claim,
            Certificate::Pullback(Box::new(evidence.clone())),
        );
        assert!(
            fixture
                .base
                .cas
                .check_proof(&fixture.base.context, &[draft], &mut budget()?)
                .is_err()
        );
    }
    Ok(())
}

/// Rechecking an incomplete exact root list reports its mathematical refusal, independently of generation.
/// ´test:integration:pullback-incomplete-witness-refusal´
#[test]
fn pullback_incomplete_witness_refusal() -> TestResult {
    let mut fixture = Fixture::new()?;
    let source = fixture.source()?;
    let map = fixture.quadratic(false)?;
    let request = fixture
        .base
        .request(fixture.task(roots(&["-1", "1"])?), &[source, map])?;
    let result = fixture.base.proved(&request)?;
    let mut evidence = fixture
        .base
        .cas
        .pullback_certificate(&result.witness)?
        .clone();
    let _removed = evidence.zeros.pop();
    let draft = application(
        &fixture,
        Primitive::DistSimpleZeroPullback,
        result.witness.judgment().statement.clone(),
        Certificate::Pullback(Box::new(evidence)),
    );
    let error = fixture
        .base
        .cas
        .check_proof(&fixture.base.context, &[draft], &mut budget()?)
        .err()
        .ok_or("incomplete roots accepted")?;
    assert_eq!(
        error.kind,
        FaultKind::Invalid(InvalidCode::PullbackIncompleteZeros)
    );
    Ok(())
}

/// The hostile application has no hidden premise or substitution authority.
fn application(
    fixture: &Fixture,
    rule: Primitive,
    conclusion: Statement,
    certificate: Certificate,
) -> ApplicationDraft {
    ApplicationDraft {
        context: fixture.base.context.clone(),
        rule,
        conclusion,
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate,
    }
}

/// Negative affine maps reverse endpoints and must map the entire declared target interval onto the source.
/// ´test:integration:pullback-affine-interval-map´
#[test]
fn pullback_affine_interval_map() -> TestResult {
    for (upper, succeeds) in [("3", true), ("4", false)] {
        let mut fixture = Fixture::new()?;
        fixture.interval(true, "-3", "3")?;
        fixture.interval(false, "1", upper)?;
        let source = fixture.source()?;
        let (map, inverse) = fixture.affine("-3", "6")?;
        let output = fixture.weighted(&[("2", "1/3")])?;
        let expected = fixture.expected(source.clone(), map.clone(), Some(inverse), output);
        let request = fixture
            .base
            .request(fixture.task(PullbackKind::Affine), &[source, map])?;
        let outcome = fixture.base.cas.run(&request);
        if succeeds {
            let Outcome::Proved { result, .. } = outcome else {
                return Err("finite affine map".into());
            };
            assert_eq!(*result.witness.judgment(), expected);
        } else {
            assert!(matches!(
                outcome,
                Outcome::Refused {
                    reason: RefusalReason::PullbackHypotheses,
                    ..
                }
            ));
        }
    }
    Ok(())
}

/// A root at an excluded endpoint contributes nothing, but remains required by exhaustive factorization.
/// ´test:integration:pullback-simple-zero-interval-support´
#[test]
fn pullback_simple_zero_interval_support() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.interval(false, "-1", "2")?;
    let source = fixture.source()?;
    let map = fixture.quadratic(false)?;
    let output = fixture.weighted(&[("1", "1/2")])?;
    let expected = fixture.expected(source.clone(), map.clone(), None, output);
    let request = fixture
        .base
        .request(fixture.task(roots(&["-1", "1"])?), &[source, map])?;
    let result = fixture.base.proved(&request)?;
    assert_eq!(*result.witness.judgment(), expected);
    assert_eq!(
        fixture
            .base
            .cas
            .pullback_certificate(&result.witness)?
            .zeros
            .len(),
        2
    );
    Ok(())
}

/// An algebraically polynomial map with a strict zero-power hole is not a smooth coordinate map.
/// ´test:integration:pullback-strict-map-totality´
#[test]
fn pullback_strict_map_totality() -> TestResult {
    let mut fixture = Fixture::new()?;
    let source = fixture.source()?;
    let hole = fixture.base.term(
        nrz_cas::syntax::Operator::IntegerPower(0),
        &[fixture.base.x.clone()],
    )?;
    let map = fixture.base.term(
        nrz_cas::syntax::Operator::Mul,
        &[hole, fixture.base.x.clone()],
    )?;
    let request = fixture
        .base
        .request(fixture.task(PullbackKind::Affine), &[source, map])?;
    assert!(matches!(
        fixture.base.cas.run(&request),
        Outcome::Refused {
            reason: RefusalReason::PullbackHypotheses,
            ..
        }
    ));
    Ok(())
}
