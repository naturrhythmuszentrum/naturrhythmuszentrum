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

/// The measured generation cap succeeds exactly and refuses one below without changing the profile.
/// ´test:integration:blind-generation-work-boundary´
#[test]
fn blind_generation_work_boundary() -> TestResult {
    let (mut fixture, request) = polynomial(&[8, 0, 0, 1])?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { usage, .. } = outcome else {
        return Err(format!("baseline: {outcome:?}").into());
    };
    let required = usage.work;
    for cap in [
        required,
        required.checked_sub(1).ok_or("empty generation ledger")?,
    ] {
        let (mut fixture, mut request) = polynomial(&[8, 0, 0, 1])?;
        request.limits.work = Some(cap);
        let outcome = fixture.cas.run(&request);
        if cap == required {
            let Outcome::Solved { usage, .. } = outcome else {
                return Err(format!("exact cap: {outcome:?}").into());
            };
            assert_eq!(usage.work, required);
        } else {
            let Outcome::Exhausted { fault, usage, .. } = outcome else {
                return Err(format!("below cap: {outcome:?}").into());
            };
            assert!(
                matches!(fault.kind, nrz_cas::FaultKind::Exhausted { budget: BudgetKind::Work, limit, needed, .. } if limit == cap && needed > limit)
            );
            assert!(usage.work <= cap);
            assert_eq!(fault.location, nrz_cas::Location::new("exact coefficients"));
        }
    }
    Ok(())
}

/// Public fixtures provide only an original source request.
type SourceFactory = fn() -> TestResult<(
    crate::testing::solve_blind::Fixture,
    nrz_cas::request::Request,
)>;

/// One public request changes only its measured selected resource ceiling.
fn service_boundary(
    factory: SourceFactory,
    kind: BudgetKind,
    usage: fn(&nrz_cas::limits::BudgetUsage) -> u64,
    limit: fn(&mut nrz_cas::limits::LimitsOverride, u64),
    expected_location: nrz_cas::Location,
) -> TestResult {
    let (mut fixture, request) = factory()?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved {
        answer,
        usage: measured,
        ..
    } = outcome
    else {
        return Err(format!("resource baseline: {outcome:?}").into());
    };
    let required = usage(&measured);
    assert!(required > 0, "unexercised {kind:?}");
    let coverage = answer.coverage.ok_or("resource baseline coverage")?;
    let bytes = fixture
        .cas
        .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    for cap in [required, required - 1] {
        let (mut fixture, mut request) = factory()?;
        limit(&mut request.limits, cap);
        let outcome = fixture.cas.run(&request);
        if cap == required {
            let Outcome::Solved { answer, .. } = outcome else {
                return Err(format!("resource exact cap: {outcome:?}").into());
            };
            let coverage = answer.coverage.ok_or("resource exact coverage")?;
            assert_eq!(
                fixture
                    .cas
                    .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?,
                bytes
            );
        } else {
            let Outcome::Exhausted { fault, .. } = outcome else {
                return Err(format!("resource below cap: {outcome:?}").into());
            };
            assert!(
                matches!(fault.kind, nrz_cas::FaultKind::Exhausted { budget, limit, needed, .. } if budget == kind && limit == cap && needed > cap),
                "{fault:?}"
            );
            assert_eq!(fault.location, expected_location);
        }
    }
    Ok(())
}

/// The zero-polynomial family's actual bound-coordinate requirement has an isolated exact ceiling.
/// ´test:integration:blind-family-parameter-boundary´
#[test]
fn blind_family_parameter_boundary() -> TestResult {
    service_boundary(
        || polynomial(&[0]),
        BudgetKind::FamilyParameters,
        |usage| usage.family_parameters,
        |limits, cap| limits.family_parameters = Some(cap),
        nrz_cas::Location::new("solving object admission"),
    )
}

/// Regional inversion of a symbolic linear leading coefficient retains its measured denominator work.
/// ´test:integration:blind-denominator-factor-boundary´
#[test]
fn blind_denominator_factor_boundary() -> TestResult {
    service_boundary(
        || crate::testing::solve_blind::symbolic(&["b", "a"]),
        BudgetKind::DenominatorFactors,
        |usage| usage.denominator_factors,
        |limits, cap| limits.denominator_factors = Some(cap),
        nrz_cas::Location::new("exact coefficients"),
    )
}

/// The underlying supplied component certificate succeeds at N and atomically refuses at N-1.
/// ´test:integration:blind-component-replay-boundary´
#[test]
fn blind_component_replay_boundary() -> TestResult {
    let mut fixture = crate::testing::solve_coefficients::SplitFixture::new()?;
    let mut baseline = Budget::new(Limits::blind_cubic())?;
    let witness = fixture.unit.base.cas.check_proof(
        &fixture.unit.base.context,
        &fixture.steps,
        &mut baseline,
    )?;
    assert!(witness.is_discharged());
    let bytes = fixture
        .unit
        .base
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let required = baseline.usage().component_splits;
    assert!(required > 0);
    for cap in [required, required - 1] {
        let mut fixture = crate::testing::solve_coefficients::SplitFixture::new()?;
        let before = fixture.unit.base.cas.counts();
        let mut budget = Budget::new(Limits {
            component_splits: cap,
            ..Limits::blind_cubic()
        })?;
        let result = fixture.unit.base.cas.check_proof(
            &fixture.unit.base.context,
            &fixture.steps,
            &mut budget,
        );
        if cap == required {
            let checked = result?;
            assert_eq!(
                fixture
                    .unit
                    .base
                    .cas
                    .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
                bytes
            );
        } else {
            let fault = result.err().ok_or("component below cap succeeded")?;
            assert!(
                matches!(fault.kind, nrz_cas::FaultKind::Exhausted { budget: BudgetKind::ComponentSplits, limit, needed, .. } if limit == cap && needed > cap),
                "{fault:?}"
            );
            assert_eq!(fixture.unit.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// Regional unit replay reconstructs the supplied identity and rejects its nonzero sibling.
/// ´test:integration:blind-regional-unit-replay´
#[test]
fn blind_regional_unit_replay() -> TestResult {
    let (mut fixture, step) = crate::testing::solve_blind::regional_unit(true)?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert!(witness.is_discharged());
    assert_eq!(
        witness.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    let bytes = fixture
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut fresh, expected_step) = crate::testing::solve_blind::regional_unit(true)?;
    let expected = nrz_cas::proof::Judgment {
        context: fresh.context.clone(),
        layer: nrz_cas::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: expected_step.conclusion,
    };
    let replayed = fresh
        .cas
        .replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(replayed.is_discharged());
    assert_eq!(
        fresh
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    let (mut sibling, changed) = crate::testing::solve_blind::regional_unit(false)?;
    assert!(
        sibling
            .cas
            .check_proof(
                &sibling.context,
                &[changed],
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// The supplied nonzero identity survives fresh current-wire expected-claim replay.
/// ´test:integration:blind-nonzero-consequence-replay´
#[test]
fn blind_nonzero_consequence_replay() -> TestResult {
    let (mut fixture, step) = crate::testing::solve_blind::nonzero_consequence(true)?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[step],
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert!(witness.is_discharged());
    let bytes = fixture
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut fresh, supplied) = crate::testing::solve_blind::nonzero_consequence(true)?;
    let expected = nrz_cas::proof::Judgment {
        context: fresh.context.clone(),
        layer: nrz_cas::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: supplied.conclusion,
    };
    let replayed = fresh
        .cas
        .replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(replayed.is_discharged());
    assert_eq!(
        replayed.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    assert_eq!(
        fresh
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Exponent, cofactor, factor inventory and exact cell are all independently checked.
/// ´test:integration:blind-nonzero-consequence-tampering´
#[test]
fn blind_nonzero_consequence_tampering() -> TestResult {
    use nrz_cas::proof::Certificate;
    use nrz_cas::solving::{IdentityMode, SolveCertificate};
    for mutation in 0..5 {
        let (mut fixture, mut step) = crate::testing::solve_blind::nonzero_consequence(true)?;
        let Certificate::Solve(payload) = &mut step.certificate else {
            return Err("solve certificate".into());
        };
        let SolveCertificate::CoefficientIdentity(certificate) = payload.as_mut() else {
            return Err("identity".into());
        };
        match mutation {
            0 | 4 => {
                assert_eq!(certificate.mode, IdentityMode::NonzeroPower(2));
                let replacement = IdentityMode::NonzeroPower(u32::from(mutation == 0));
                assert_ne!(certificate.mode, replacement);
                certificate.mode = replacement;
            }
            1 => {
                assert_eq!(certificate.combinations.len(), 1);
                certificate.combinations.clear();
                assert_ne!(certificate.combinations.len(), 1);
            }
            2 => {
                let cofactor = &mut certificate
                    .combinations
                    .first_mut()
                    .ok_or("cofactor")?
                    .multiplier;
                assert_eq!(*cofactor, certificate.right);
                assert_ne!(*cofactor, certificate.left);
                cofactor.clone_from(&certificate.left);
            }
            _ => {
                let factor = certificate.denominators.first_mut().ok_or("factor")?;
                assert_eq!(*factor, certificate.right);
                assert_ne!(*factor, certificate.left);
                factor.clone_from(&certificate.left);
            }
        }
        assert!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    &[step],
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
    }
    let original = true;
    let replacement = false;
    assert!(original);
    assert_ne!(original, replacement);
    let (mut sibling, step) = crate::testing::solve_blind::nonzero_consequence(replacement)?;
    assert!(
        sibling
            .cas
            .check_proof(
                &sibling.context,
                &[step],
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}
