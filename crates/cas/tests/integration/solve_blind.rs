// SPDX-License-Identifier: AGPL-3.0-only
//! Public source-to-radical coverage and independent fresh-store replay.

use crate::testing::common::TestResult;
use crate::testing::solve_blind::polynomial;
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::request::Outcome;
use nrz_cas::solving::{SolveForm, SolveStatement};

/// Compare imported coverage with a freshly constructed original request, preserving the exact region.
fn replay_coverage(
    fixture: &mut crate::testing::solve_blind::Fixture,
    request: &nrz_cas::request::Request,
    bytes: &[u8],
) -> TestResult<nrz_cas::Handle> {
    let nrz_cas::request::Task::Solve(original) = &request.task else {
        return Err("expected solve request".into());
    };
    let imported = fixture
        .cas
        .inspect(bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    let nrz_cas::proof::Statement::Solve(statement) = &imported.judgment().statement else {
        return Err("imported solve claim".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("imported coverage claim".into());
    };
    let expected = nrz_cas::proof::Judgment {
        context: fixture.context.clone(),
        layer: nrz_cas::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: nrz_cas::proof::Statement::Solve(Box::new(SolveStatement::Coverage(
            nrz_cas::solving::StatementCoverage {
                problem: original.problem.clone(),
                set: claim.set.clone(),
                region: fixture.region.clone(),
            },
        ))),
    };
    let replayed =
        fixture
            .cas
            .replay(bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert_eq!(replayed.hypotheses(), []);
    assert!(replayed.is_discharged());
    assert_eq!(
        fixture
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(claim.set.clone())
}

/// Coverage and requested presentation are independent expected claims in the fresh original store.
fn replay_complete(
    cas: &nrz_cas::Cas,
    answer: &nrz_cas::solving::SolveAnswer,
    fresh: (
        crate::testing::solve_blind::Fixture,
        nrz_cas::request::Request,
    ),
) -> TestResult {
    let coverage = answer.coverage.as_ref().ok_or("complete coverage")?;
    assert!(coverage.is_discharged());
    assert_eq!(coverage.hypotheses(), []);
    let bytes = cas.export_witness(coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut fixture, request) = fresh;
    let set = replay_coverage(&mut fixture, &request, &bytes)?;
    let presentation = answer.facts.last().ok_or("radical presentation")?;
    assert!(presentation.is_discharged());
    assert_eq!(presentation.hypotheses(), []);
    let bytes = cas.export_witness(presentation, &mut Budget::new(Limits::blind_cubic())?)?;
    let expected = nrz_cas::proof::Judgment {
        context: fixture.context.clone(),
        layer: nrz_cas::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: nrz_cas::proof::Statement::Solve(Box::new(SolveStatement::Presentation(
            nrz_cas::solving::StatementPresentation {
                set,
                form: SolveForm::Radicals,
                region: fixture.region.clone(),
            },
        ))),
    };
    let replayed =
        fixture
            .cas
            .replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(replayed.is_discharged());
    assert_eq!(replayed.hypotheses(), []);
    assert_eq!(
        fixture
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

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
    replay_complete(&fixture.cas, &answer, polynomial(&[8, 0, 0, 1])?)?;
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
        assert_eq!(answer.form, SolveForm::Radicals);
        replay_complete(&fixture.cas, &answer, polynomial(&coefficients)?)?;
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

/// The original Complex parameter problem has complete coverage and radical presentation on every stratum.
/// ´test:integration:blind-symbolic-complex-complete´
#[test]
fn blind_symbolic_complex_complete() -> TestResult {
    let (mut fixture, request) = crate::testing::solve_blind::symbolic(&["d", "c", "b", "a"])?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("complex symbolic acceptance: {outcome:?}").into());
    };
    assert_eq!(answer.form, SolveForm::Radicals);
    assert_eq!(answer.unresolved_regions, [] as [nrz_cas::Handle; 0]);
    replay_complete(
        &fixture.cas,
        &answer,
        crate::testing::solve_blind::symbolic(&["d", "c", "b", "a"])?,
    )?;
    Ok(())
}

/// The Real fixture is refused by the producer ambient check before any initial coefficient map.
/// Both discriminant signs and lower-degree families remain blocked by the named real-isolation boundary.
/// ´test:integration:blind-symbolic-real-complete´
#[test]
fn blind_symbolic_real_complete() -> TestResult {
    let (mut fixture, request) = crate::testing::solve_blind::symbolic_in_field(
        &["d", "c", "b", "a"],
        &nrz_cas::syntax::Field::Real,
    )?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Unresolved { solve_reason, .. } = outcome else {
        return Err(format!("real symbolic boundary moved: {outcome:?}").into());
    };
    assert_eq!(
        solve_reason,
        Some(nrz_cas::solving::SolveReason::RealIsolationUnavailable)
    );
    Ok(())
}

/// Nonreal cubic roots retain their Gaussian embedding through both fresh expected-claim replays.
/// ´test:integration:blind-nonreal-cubic-replay´
#[test]
fn blind_nonreal_cubic_replay() -> TestResult {
    let coefficients = [(0, 2), (-2, 1), (-1, -1), (1, 0)];
    let (mut fixture, request) = crate::testing::solve_blind::gaussian_polynomial(&coefficients)?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("nonreal cubic: {outcome:?}").into());
    };
    assert_eq!(answer.form, SolveForm::Radicals);
    replay_complete(
        &fixture.cas,
        &answer,
        crate::testing::solve_blind::gaussian_polynomial(&coefficients)?,
    )
}
