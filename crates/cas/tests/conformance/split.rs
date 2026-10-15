// SPDX-License-Identifier: AGPL-3.0-only
//! Finite-point extension laws, exact split budgets and hostile endpoint witnesses.

use crate::testing::boundary::named;
use crate::testing::budgets::derivation_budget as budget;
use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use crate::testing::split::{odd, task};
use nrz_cas::limits::{BudgetKind, LimitsOverride};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, SplitKind, Statement};
use nrz_cas::request::Outcome;
use nrz_cas::syntax::{CaseName, Operator, PredicateKind};
use nrz_cas::{FaultKind, Rational};

/// The value or absence at zero leaves integral x² on the left and 2x² on the right unchanged.
/// ´test:integration:split-finite-point-extension´
#[test]
fn split_finite_point_extension() -> TestResult {
    for changed in [false, true] {
        let mut fixture = Fixture::new()?;
        let function = punctured_quadratic(&mut fixture, changed)?;
        let request = fixture.request(task(SplitKind::Integral, "-1", "2")?, &[function])?;
        let result = fixture.proved(&request)?;
        let Statement::Split(value) = &result.witness.judgment().statement else {
            return Err("split value".into());
        };
        assert_eq!(value.value, Rational::parse("17/3", &mut budget()?)?);
        let evidence = fixture.cas.split_certificate(&result.witness)?;
        let point = evidence
            .points
            .iter()
            .find(|point| point.point.is_zero())
            .ok_or("zero point")?;
        assert_eq!(
            point.value,
            if changed {
                Some(Rational::parse("7", &mut budget()?)?)
            } else {
                None
            }
        );
    }
    Ok(())
}

/// Exact two-branch function with an independently controlled isolated value.
fn punctured_quadratic(fixture: &mut Fixture, changed: bool) -> TestResult<nrz_cas::Handle> {
    let x = fixture.x.clone();
    let square = fixture.term(Operator::IntegerPower(2), std::slice::from_ref(&x))?;
    let two = fixture.number("2")?;
    let doubled = fixture.term(Operator::Mul, &[two, square.clone()])?;
    let positive = fixture.predicate(PredicateKind::Positive, std::slice::from_ref(&x))?;
    let negative = fixture.term(Operator::Neg, std::slice::from_ref(&x))?;
    let negative = fixture.predicate(PredicateKind::Positive, &[negative])?;
    let left = fixture.region(&negative, true)?;
    let right = fixture.region(&positive, true)?;
    let mut branches = vec![(left, square), (right, doubled)];
    if changed {
        let equal = fixture.predicate(
            PredicateKind::Equal,
            &[fixture.x.clone(), fixture.zero.clone()],
        )?;
        let singleton = fixture.region(&equal, true)?;
        branches.push((singleton, fixture.number("7")?));
    }
    fixture.cases(&branches)
}

/// Every integer point is required; noninteger or reversed summation endpoints refuse.
/// ´test:integration:split-sum-endpoint-refusals´
#[test]
fn split_sum_endpoint_refusals() -> TestResult {
    for (lower, upper) in [("1/2", "9"), ("9", "1")] {
        let mut fixture = Fixture::new()?;
        let function = odd(&mut fixture, false)?;
        let request = fixture.request(task(SplitKind::Sum, lower, upper)?, &[function])?;
        assert!(matches!(fixture.cas.run(&request), Outcome::Invalid { .. }));
    }
    let mut fixture = Fixture::new()?;
    let function = punctured_quadratic(&mut fixture, false)?;
    let request = fixture.request(task(SplitKind::Sum, "-1", "2")?, &[function])?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::PendingPremise { .. }
    ));
    Ok(())
}

/// A pole on an open cell has no polynomial integrability certificate.
/// ´test:integration:split-improper-integral-refused´
#[test]
fn split_improper_integral_refused() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = fixture.term(Operator::Div, &[fixture.one.clone(), fixture.x.clone()])?;
    let request = fixture.request(task(SplitKind::Integral, "-1", "1")?, &[function])?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unsupported { .. } | Outcome::Unresolved { .. } | Outcome::PendingPremise { .. }
    ));
    Ok(())
}

/// Allocation order cannot change exact proof bytes or finite endpoint evidence.
/// ´test:integration:split-deterministic-bytes´
#[test]
fn split_deterministic_bytes() -> TestResult {
    for kind in [SplitKind::Integral, SplitKind::Sum] {
        let mut outputs = Vec::new();
        for shifted in [false, true] {
            let mut fixture = Fixture::new()?;
            if shifted {
                fixture.number("123/17")?;
            }
            let function = odd(&mut fixture, true)?;
            let request = fixture.request(task(kind, "1", "9")?, &[function])?;
            let result = fixture.proved(&request)?;
            outputs.push(
                fixture
                    .cas
                    .export_witness(&result.witness, &mut budget()?)?,
            );
        }
        assert_eq!(outputs.first(), outputs.get(1));
    }
    Ok(())
}

/// The checker rejects changed primitives, endpoint values, finite points and omitted cells atomically.
/// ´test:integration:split-certificate-tampering´
#[test]
fn split_certificate_tampering() -> TestResult {
    for kind in [SplitKind::Integral, SplitKind::Sum] {
        let mut fixture = Fixture::new()?;
        let function = named(&mut fixture, CaseName::Abs)?;
        let request = fixture.request(task(kind, "-1", "2")?, &[function])?;
        let result = fixture.proved(&request)?;
        let evidence = fixture.cas.split_certificate(&result.witness)?.clone();
        for mutation in 0..4 {
            let mut changed = evidence.clone();
            let piece = changed.pieces.first_mut().ok_or("piece")?;
            match mutation {
                0 => piece.primitive.clear(),
                1 => piece.end = piece.end.add(&Rational::one(), &mut budget()?)?,
                2 => piece.lower = piece.lower.sub(&Rational::one(), &mut budget()?)?,
                _ => changed.pieces.clear(),
            }
            let draft = ApplicationDraft {
                context: fixture.context.clone(),
                rule: if kind == SplitKind::Integral {
                    Primitive::IntegralCaseSplit
                } else {
                    Primitive::SumCaseSplit
                },
                conclusion: result.witness.judgment().statement.clone(),
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::Split(Box::new(changed)),
            };
            assert!(
                fixture
                    .cas
                    .check_proof(&fixture.context, &[draft], &mut budget()?)
                    .is_err()
            );
        }
    }
    Ok(())
}

/// Exact resource peaks succeed and one below refuses the same calculation.
/// ´test:integration:split-exact-resource-boundaries´
#[test]
fn split_exact_resource_boundaries() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = named(&mut fixture, CaseName::Abs)?;
    let request = fixture.request(task(SplitKind::Integral, "-1", "2")?, &[function])?;
    let Outcome::Proved { result, usage, .. } = fixture.cas.run(&request) else {
        return Err("split proof".into());
    };
    let boundaries = [
        (
            BudgetKind::CaseArms,
            usage.case_arms.max(result.replay_usage.case_arms),
        ),
        (
            BudgetKind::RegionCells,
            usage.region_cells.max(result.replay_usage.region_cells),
        ),
        (
            BudgetKind::BoundaryNodes,
            usage.boundary_nodes.max(result.replay_usage.boundary_nodes),
        ),
    ];
    for (kind, exact) in boundaries {
        assert!(exact > 0);
        for cap in [exact, exact - 1] {
            let mut receiver = Fixture::new()?;
            let function = named(&mut receiver, CaseName::Abs)?;
            let mut request =
                receiver.request(task(SplitKind::Integral, "-1", "2")?, &[function])?;
            set_limit(&mut request.limits, kind, cap);
            let outcome = receiver.cas.run(&request);
            if cap == exact {
                assert!(matches!(outcome, Outcome::Proved { .. }));
            } else {
                assert!(
                    matches!(outcome, Outcome::Exhausted { fault, .. } if matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind))
                );
            }
        }
    }
    Ok(())
}

/// Change only the resource under test.
const fn set_limit(limits: &mut LimitsOverride, kind: BudgetKind, value: u64) {
    match kind {
        BudgetKind::CaseArms => limits.case_arms = Some(value),
        BudgetKind::RegionCells => limits.region_cells = Some(value),
        BudgetKind::BoundaryNodes => limits.boundary_nodes = Some(value),
        _ => {}
    }
}
