// SPDX-License-Identifier: AGPL-3.0-only
//! Public source-to-radical coverage and independent fresh-store replay.

use crate::testing::common::TestResult;
use crate::testing::solve_blind::polynomial;
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::request::Outcome;
use nrz_cas::solving::{SolveForm, SolveStatement};

/// Complete factor coverage and radical provenance must independently replay without hidden lemmas.
/// ´test:integration:blind-binomial-coverage-replay´
#[test]
fn blind_binomial_coverage_replay() -> TestResult {
    let (mut fixture, request) = polynomial(&[8, 0, 0, 1])?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("unexpected outcome: {outcome:?}").into());
    };
    assert_eq!(answer.form, SolveForm::Radicals);
    assert_eq!(answer.unresolved_regions, []);
    let coverage = answer.coverage.ok_or("coverage missing")?;
    assert!(coverage.is_discharged());
    assert_eq!(coverage.hypotheses(), []);
    assert!(
        matches!(&coverage.judgment().statement, nrz_cas::proof::Statement::Solve(statement) if matches!(statement.as_ref(), SolveStatement::Coverage(_)))
    );
    let bytes = fixture
        .cas
        .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut fresh = nrz_cas::Cas::new(Limits::blind_cubic())?;
    let imported = fresh.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    let expected = imported.judgment().clone();
    let replayed = fresh.replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert_eq!(
        fresh.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    for fact in &answer.facts {
        assert!(fact.is_discharged());
    }
    Ok(())
}

/// Discovery begins with only the original equation and must recover its entire factor product.
/// ´test:integration:blind-general-ground-cubic´
#[test]
fn blind_general_ground_cubic() -> TestResult {
    for coefficients in [[-4, -15, 0, 1], [1, -3, 0, 1], [2, -3, 0, 1], [6, 11, 6, 1]] {
        let (mut fixture, request) = polynomial(&coefficients)?;
        let outcome = fixture.cas.run(&request);
        let Outcome::Solved { answer, .. } = outcome else {
            return Err(format!("{coefficients:?}: {outcome:?}").into());
        };
        assert_eq!(answer.unresolved_regions, []);
        assert!(
            answer
                .coverage
                .ok_or("missing complete coverage")?
                .is_discharged()
        );
    }
    Ok(())
}

/// Strategy unavailability seals no nonsolvability assertion about an independent quintic.
/// ´test:integration:blind-independent-quintic-unavailable´
#[test]
fn blind_independent_quintic_unavailable() -> TestResult {
    let (mut fixture, request) =
        crate::testing::solve_blind::symbolic(&["f", "e", "d", "c", "b", "a"])?;
    let outcome = fixture.cas.run(&request);
    assert!(matches!(
        outcome,
        Outcome::Unresolved {
            solve_reason: Some(nrz_cas::solving::SolveReason::GenericRadicalRouteUnavailable),
            blocker: None,
            ..
        }
    ));
    Ok(())
}

/// Supported higher-degree binomials still produce a complete family with a finite noncubic profile.
/// ´test:integration:blind-quintic-binomial-complete´
#[test]
fn blind_quintic_binomial_complete() -> TestResult {
    let (mut fixture, request) = polynomial(&[-2, 0, 0, 0, 0, 1])?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("quintic binomial: {outcome:?}").into());
    };
    assert_eq!(answer.unresolved_regions, []);
    assert!(
        answer
            .coverage
            .ok_or("quintic binomial coverage")?
            .is_discharged()
    );
    Ok(())
}

/// The zero polynomial retains the whole original coordinate family rather than an empty finite set.
/// ´test:integration:blind-zero-polynomial-family´
#[test]
fn blind_zero_polynomial_family() -> TestResult {
    let (mut fixture, request) = polynomial(&[0, 0, 0, 0])?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("zero polynomial: {outcome:?}").into());
    };
    assert_eq!(answer.unresolved_regions, []);
    assert!(
        answer
            .coverage
            .ok_or("zero family coverage")?
            .is_discharged()
    );
    let nrz_cas::solving::SolveView::SolutionSet(set) = fixture.cas.solve_view(&answer.set)? else {
        return Err("zero family set".into());
    };
    assert_eq!(set.branches.len(), 1);
    let nrz_cas::solving::SolveView::SolutionBranch(branch) = fixture
        .cas
        .solve_view(set.branches.first().ok_or("zero family branch")?)?
    else {
        return Err("zero family branch".into());
    };
    assert!(matches!(
        branch.kind,
        nrz_cas::solving::SolutionBranchKind::Image { .. }
    ));
    Ok(())
}
