// SPDX-License-Identifier: AGPL-3.0-only
//! Pinned service determinism, finite resource refusal and independence from unrelated objects.

use crate::testing::common::TestResult;
use crate::testing::solve_blind::polynomial;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::request::Outcome;

/// Identical empty-cache requests have identical canonical coverage bytes under the pinned profile.
/// ´test:integration:blind-pinned-repeat-determinism´
#[test]
fn blind_pinned_repeat_determinism() -> TestResult {
    let mut encoded = Vec::new();
    let mut work = Vec::new();
    for _ in 0..2 {
        let (mut fixture, request) = polynomial(&[8, 0, 0, 1])?;
        let outcome = fixture.cas.run(&request);
        let Outcome::Solved { answer, usage, .. } = outcome else {
            return Err(format!("{outcome:?}").into());
        };
        work.push(usage.work);
        encoded.push(fixture.cas.export_witness(
            &answer.coverage.ok_or("coverage missing")?,
            &mut Budget::new(Limits::blind_cubic())?,
        )?);
    }
    assert_eq!(encoded.first(), encoded.get(1));
    assert_eq!(work.first(), work.get(1));
    assert!(
        work.first()
            .is_some_and(|work| *work <= Limits::blind_cubic().work)
    );
    Ok(())
}

/// An exhausted generic orbit cannot be published as complete or silently converted into unavailability.
/// ´test:integration:blind-orbit-exhaustion´
#[test]
fn blind_orbit_exhaustion() -> TestResult {
    let (mut fixture, mut request) = polynomial(&[1, -3, 0, 1])?;
    request.limits.orbit_permutations = Some(0);
    let outcome = fixture.cas.run(&request);
    assert!(matches!(
        outcome,
        Outcome::Exhausted {
            solve_best: None,
            fault: nrz_cas::Fault {
                kind: nrz_cas::FaultKind::Exhausted {
                    budget: BudgetKind::OrbitPermutations,
                    ..
                },
                ..
            },
            ..
        }
    ));
    Ok(())
}

/// Unrelated stored syntax cannot alter the mathematical route or impose full-store proof charges.
/// ´test:integration:blind-unrelated-store-content´
#[test]
fn blind_unrelated_store_content() -> TestResult {
    let mut work = Vec::new();
    let mut encodings = Vec::new();
    for unrelated in [0, 1000] {
        let (mut fixture, request) = polynomial(&[8, 0, 0, 1])?;
        for value in 100..100 + unrelated {
            fixture.integer(value, false)?;
        }
        let outcome = fixture.cas.run(&request);
        let Outcome::Solved { answer, usage, .. } = outcome else {
            return Err(format!("{outcome:?}").into());
        };
        work.push(usage.work);
        encodings.push(fixture.cas.export_witness(
            &answer.coverage.ok_or("coverage missing")?,
            &mut Budget::new(Limits::blind_cubic())?,
        )?);
    }
    assert_eq!(encodings.first(), encodings.get(1));
    let baseline = *work.first().ok_or("baseline missing")?;
    assert!(*work.get(1).ok_or("unrelated run missing")? <= baseline + 100_000);
    Ok(())
}
