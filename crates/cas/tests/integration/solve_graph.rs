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
    let expected =
        if nrz_cas::solving::capabilities()[9] == nrz_cas::solving::Availability::Available {
            SolveReason::RealIsolationUnavailable
        } else {
            SolveReason::CoefficientDomainUnavailable
        };
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unresolved { solve_reason: Some(reason), .. } if reason == expected
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

/// A fresh receiver replays a typed coefficient closure through the existing hypothesis checker.
/// ´test:integration:solve-coefficient-witness-v4-round-trip´
#[test]
fn solve_coefficient_witness_v4_round_trip() -> TestResult {
    use nrz_cas::limits::{Budget, Limits};
    let (origin, witness) = crate::testing::solve_graph::coefficient_witness()?;
    let bytes = origin.export_witness(&witness, &mut Budget::new(Limits::default())?)?;
    assert_eq!(bytes.get(..8), Some(b"NRZCAS04".as_slice()));
    assert_eq!(witness.hypotheses().len(), 1);
    let mut receiver = nrz_cas::Cas::new(Limits::default())?;
    let replayed = receiver.inspect(&bytes, &mut Budget::new(Limits::default())?)?;
    assert_eq!(
        receiver.export_witness(&replayed, &mut Budget::new(Limits::default())?)?,
        bytes
    );
    assert_eq!(replayed.hypotheses().len(), 1);
    let expected = replayed.judgment().clone();
    let checked = receiver.replay(&bytes, &expected, &mut Budget::new(Limits::default())?)?;
    assert_eq!(checked.judgment(), &expected);
    Ok(())
}

/// Root binders, polynomial order and scalar selection attachments survive fresh-store replay.
/// ´test:integration:solve-root-witness-v4-round-trip´
#[test]
fn solve_root_witness_v4_round_trip() -> TestResult {
    use nrz_cas::limits::{Budget, Limits};
    let (origin, witness) = crate::testing::solve_graph::root_witness()?;
    let bytes = origin.export_witness(&witness, &mut Budget::new(Limits::default())?)?;
    assert_eq!(witness.hypotheses().len(), 1);
    let mut receiver = nrz_cas::Cas::new(Limits::default())?;
    let replayed = receiver.inspect(&bytes, &mut Budget::new(Limits::default())?)?;
    assert_eq!(
        receiver.export_witness(&replayed, &mut Budget::new(Limits::default())?)?,
        bytes
    );
    let expected = replayed.judgment().clone();
    assert_eq!(
        receiver
            .replay(&bytes, &expected, &mut Budget::new(Limits::default())?)?
            .judgment(),
        &expected
    );
    Ok(())
}

/// Original denominator obligations survive source coverage and cannot be erased by cancellation.
/// ´test:integration:solve-original-denominators-survive´
#[test]
fn solve_original_denominators_survive() -> TestResult {
    let (mut fixture, request) = crate::testing::solve_graph::quotient_request(false)?;
    let expected =
        if nrz_cas::solving::capabilities()[9] == nrz_cas::solving::Availability::Available {
            SolveReason::RealIsolationUnavailable
        } else {
            SolveReason::CoefficientDomainUnavailable
        };
    assert!(matches!(
        fixture.cas.run(&request),
        Outcome::Unresolved { solve_reason: Some(reason), .. } if reason == expected
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
