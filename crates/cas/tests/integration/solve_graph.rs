// SPDX-License-Identifier: AGPL-3.0-only
//! Complete source coverage and honest unavailable solving service lifecycles.

use crate::testing::common::TestResult;
use crate::testing::solve_graph::Fixture;
use nrz_cas::request::Outcome;
use nrz_cas::solving::SolveReason;

/// A covered symbolic problem remains unresolved until its independent producers exist.
/// ´test:integration:solve-original-request-is-unresolved´
#[test]
fn solve_original_request_is_unresolved() -> TestResult {
    let mut fixture = Fixture::new()?;
    let request = fixture.request()?;
    assert_eq!(
        request
            .inputs
            .first()
            .ok_or("input missing")?
            .receipt
            .visited(),
        3
    );
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unresolved {
            solve_reason: Some(SolveReason::CoefficientDomainUnavailable),
            ..
        }
    ));
    Ok(())
}

/// A receipt cannot authorize a different problem's original predicate.
/// ´test:integration:solve-request-predicate-mismatch´
#[test]
fn solve_request_predicate_mismatch() -> TestResult {
    let mut fixture = Fixture::new()?;
    let other = Fixture::new()?;
    let mut request = fixture.request()?;
    request.task = nrz_cas::request::Task::Solve(Box::new(nrz_cas::solving::SolveSpec {
        problem: other.problem,
        goal: nrz_cas::solving::SolveGoal::Complete,
        form: nrz_cas::solving::SolveForm::AnyExact,
        exclusions: nrz_cas::solving::ExclusionPolicy::PreserveOriginal,
    }));
    let before = fixture.cas.counts();
    assert!(matches!(fixture.cas.run(&request), Outcome::Invalid { .. }));
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Original denominator obligations survive source coverage and cannot be erased by cancellation.
/// ´test:integration:solve-original-denominators-survive´
#[test]
fn solve_original_denominators_survive() -> TestResult {
    let (mut fixture, request) = crate::testing::solve_graph::quotient_request(false)?;
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unresolved {
            solve_reason: Some(SolveReason::CoefficientDomainUnavailable),
            ..
        }
    ));
    let (mut omitted, invalid_request) = crate::testing::solve_graph::quotient_request(true)?;
    let before = omitted.cas.counts();
    assert!(matches!(
        omitted.cas.run(&invalid_request),
        Outcome::Invalid { .. }
    ));
    assert_eq!(omitted.cas.counts(), before);
    Ok(())
}
