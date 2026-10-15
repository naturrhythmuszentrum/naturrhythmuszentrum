// SPDX-License-Identifier: AGPL-3.0-only
//! Public finite-orbit replay, mutation and resource contracts.

use crate::testing::common::TestResult;
use crate::testing::solve_orbits::Fixture;
use nrz_cas::FaultKind;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::Statement;
use nrz_cas::solving::{CertificatePermutationOrbitCoverage, SolveStatement};

/// Both character orientations and their first three powers have independently supplied exact orbits.
/// ´test:integration:orbit-character-orientations´
#[test]
fn orbit_character_orientations() -> TestResult {
    for orientation in [1, 2] {
        for (exponent, cardinality) in [(1, 6), (2, 6), (3, 2)] {
            let mut fixture = Fixture::new(orientation, exponent)?;
            let step = fixture.step();
            let witness = fixture.base.cas.check_proof(
                &fixture.base.context,
                std::slice::from_ref(&step),
                &mut Budget::new(Limits::blind_cubic())?,
            )?;
            assert_eq!(witness.hypotheses(), []);
            assert_eq!(witness.judgment().residuals, []);
            let Statement::Solve(statement) = &witness.judgment().statement else {
                return Err("wrong statement".into());
            };
            let SolveStatement::Orbit(orbit) = statement.as_ref() else {
                return Err("wrong orbit".into());
            };
            assert_eq!(orbit.members.len(), cardinality);
            assert_eq!(orbit.seed, fixture.certificate.seed);
            assert_eq!(orbit.generators, [vec![1, 0, 2], vec![0, 2, 1]]);
        }
    }
    assert_ne!(nrz_cas::solving::active_schema_bitmap() & (1 << 14), 0);
    Ok(())
}

/// Mutate well-formed evidence while keeping its conclusion synchronized.
fn mutate(value: &mut CertificatePermutationOrbitCoverage, mutation: u8) -> TestResult {
    match mutation {
        0 => {
            let _member = value.members.pop();
            let _word = value.words.pop();
            let _action = value.actions.pop();
        }
        1 => {
            value
                .members
                .get_mut(1)
                .ok_or("missing member")?
                .clone_from(&value.seed);
        }
        2 => {
            *value
                .actions
                .first_mut()
                .and_then(|row| row.first_mut())
                .ok_or("missing action")? = 99;
        }
        3 => {
            *value.generators.first_mut().ok_or("missing generator")? = vec![0, 0, 2];
        }
        4 => value.degree = 4,
        5 => value.words.get_mut(1).ok_or("missing word")?.clear(),
        6 => {
            value.generators.clear();
            for row in &mut value.actions {
                row.clear();
            }
            for word in &mut value.words {
                word.clear();
            }
        }
        _ => value.words.get_mut(1).ok_or("missing word")?.push(99),
    }
    Ok(())
}

/// Missing/duplicate members, false actions, nonbijections and disconnected orbits all refuse atomically.
/// ´test:integration:orbit-certificate-mutations´
#[test]
fn orbit_certificate_mutations() -> TestResult {
    let mut fixture = Fixture::new(1, 1)?;
    let original = fixture.certificate.clone();
    for mutation in 0..8 {
        fixture.certificate.clone_from(&original);
        mutate(&mut fixture.certificate, mutation)?;
        let step = fixture.step();
        let before = fixture.base.cas.counts();
        assert!(
            fixture
                .base
                .cas
                .check_proof(
                    &fixture.base.context,
                    std::slice::from_ref(&step),
                    &mut Budget::new(Limits::blind_cubic())?,
                )
                .is_err()
        );
        assert_eq!(fixture.base.cas.counts(), before);
    }
    fixture.certificate = original;
    let mut step = fixture.step();
    let Statement::Solve(statement) = &mut step.conclusion else {
        return Err("wrong statement".into());
    };
    let SolveStatement::Orbit(orbit) = statement.as_mut() else {
        return Err("wrong orbit".into());
    };
    orbit.members.swap(0, 1);
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                std::slice::from_ref(&step),
                &mut Budget::new(Limits::blind_cubic())?,
            )
            .is_err()
    );
    Ok(())
}

/// An arbitrary generated subgroup is accepted without silently upgrading it to full symmetry.
/// ´test:integration:orbit-subgroup-and-trivial-action´
#[test]
fn orbit_subgroup_and_trivial_action() -> TestResult {
    let mut fixture = Fixture::new(1, 1)?;
    let image = usize::try_from(
        *fixture
            .certificate
            .actions
            .first()
            .and_then(|row| row.first())
            .ok_or("missing action")?,
    )?;
    let second = fixture
        .certificate
        .members
        .get(image)
        .ok_or("missing image")?
        .clone();
    fixture.certificate.members = vec![fixture.certificate.seed.clone(), second];
    fixture.certificate.generators.truncate(1);
    fixture.certificate.actions = vec![vec![1], vec![0]];
    fixture.certificate.words = vec![vec![], vec![0]];
    let step = fixture.step();
    fixture.base.cas.check_proof(
        &fixture.base.context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    fixture.certificate.members.truncate(1);
    fixture.certificate.generators.clear();
    fixture.certificate.actions = vec![vec![]];
    fixture.certificate.words = vec![vec![]];
    let trivial = fixture.step();
    fixture.base.cas.check_proof(
        &fixture.base.context,
        std::slice::from_ref(&trivial),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    Ok(())
}

/// Each orbit counter succeeds at its measured N and fails at N-1 without publishing a proof prefix.
/// ´test:integration:orbit-budget-boundaries´
#[test]
fn orbit_budget_boundaries() -> TestResult {
    let mut fixture = Fixture::new(1, 3)?;
    let step = fixture.step();
    let mut measured = Budget::new(Limits::blind_cubic())?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        std::slice::from_ref(&step),
        &mut measured,
    )?;
    let usage = measured.usage();
    for (kind, count) in [
        (BudgetKind::OrbitPermutations, usage.orbit_permutations),
        (BudgetKind::OrbitMembers, usage.orbit_members),
        (BudgetKind::OrbitTerms, usage.orbit_terms),
    ] {
        assert!(count > 0);
        for (limit, succeeds) in [(count, true), (count - 1, false)] {
            let mut limits = Limits::blind_cubic();
            match kind {
                BudgetKind::OrbitPermutations => limits.orbit_permutations = limit,
                BudgetKind::OrbitMembers => limits.orbit_members = limit,
                _ => limits.orbit_terms = limit,
            }
            let before = fixture.base.cas.counts();
            let result = fixture.base.cas.check_proof(
                &fixture.base.context,
                std::slice::from_ref(&step),
                &mut Budget::new(limits)?,
            );
            if succeeds {
                assert!(result.is_ok());
            } else {
                let failure = result.err().ok_or("short budget accepted")?;
                assert!(
                    matches!(failure.kind, FaultKind::Exhausted { budget, .. } if budget == kind)
                );
                assert_eq!(fixture.base.cas.counts(), before);
            }
        }
    }
    Ok(())
}
