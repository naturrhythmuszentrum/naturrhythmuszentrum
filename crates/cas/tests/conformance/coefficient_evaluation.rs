// SPDX-License-Identifier: AGPL-3.0-only
//! Same-sort finite evaluation and prerequisite tampering.

use crate::testing::coefficient_evaluation::{
    canceled_divisor, family, family_image, finite, selected_root, verify,
};
use crate::testing::common::TestResult;
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::Certificate;
use nrz_cas::solving::SolveCertificate;

/// Finite evaluation permits zero specialization and computes all polynomial terms.
/// ´test:integration:scalar-finite-evaluation-specializes-without-injectivity´
#[test]
fn scalar_finite_evaluation_specializes_without_injectivity() -> TestResult {
    for zero in [false, true] {
        let (_, proof) = verify(finite(zero, false, false)?)?;
        assert_eq!(proof.hypotheses().len(), 1);
    }
    verify(family_image()?)?;
    let (_, proof) = verify(family()?)?;
    assert_eq!(proof.hypotheses().len(), 2);
    Ok(())
}

/// A nonzero source variable may specialize to a pole, so source-field units do not transfer.
/// ´test:integration:scalar-finite-evaluation-localization-units-are-target-facts´
#[test]
fn scalar_finite_evaluation_localization_units_are_target_facts() -> TestResult {
    verify(finite(false, true, true)?)?;
    for mut fixture in [finite(false, true, false)?, finite(true, true, true)?] {
        let before = fixture.cas.counts();
        assert!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    &fixture.steps,
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// The original image divisor survives arithmetic cancellation.
/// ´test:integration:scalar-finite-evaluation-retains-actual-divisor´
#[test]
fn scalar_finite_evaluation_retains_actual_divisor() -> TestResult {
    verify(canceled_divisor(true)?)?;
    let mut fixture = canceled_divisor(false)?;
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// A Real selected-root coefficient is evaluated in Real with its exact prior existence.
/// ´test:integration:scalar-finite-evaluation-requires-selected-root-existence´
#[test]
fn scalar_finite_evaluation_requires_selected_root_existence() -> TestResult {
    verify(selected_root()?)?;
    let mut fixture = selected_root()?;
    let step = fixture.steps.last_mut().ok_or("missing evaluation")?;
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("missing certificate".into());
    };
    let SolveCertificate::CoefficientEvaluation(certificate) = certificate.as_mut() else {
        return Err("wrong schema".into());
    };
    certificate.existence.clear();
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// Altered finite support or omitted image-definedness cannot replay a result.
/// ´test:integration:scalar-finite-evaluation-refuses-support-and-definedness-tampering´
#[test]
fn scalar_finite_evaluation_refuses_support_and_definedness_tampering() -> TestResult {
    for support in [false, true] {
        let mut fixture = finite(false, false, false)?;
        let step = fixture.steps.last_mut().ok_or("missing evaluation")?;
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("missing certificate".into());
        };
        let SolveCertificate::CoefficientEvaluation(certificate) = certificate.as_mut() else {
            return Err("wrong schema".into());
        };
        if support {
            certificate.support.clear();
        } else {
            certificate.definedness.clear();
        }
        assert!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    &fixture.steps,
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
    }
    Ok(())
}

/// A family wrapper cannot authorize Gaussian ground coefficients in a Real fibre.
/// ´test:integration:scalar-finite-evaluation-checks-family-ground-space´
#[test]
fn scalar_finite_evaluation_checks_family_ground_space() -> TestResult {
    use crate::testing::coefficient_evaluation::family_ground_constants;
    verify(family_ground_constants(false, true)?)?;
    verify(family_ground_constants(true, false)?)?;
    let mut fixture = family_ground_constants(true, true)?;
    let before = fixture.cas.counts();
    let fault = fixture
        .cas
        .check_proof(
            &fixture.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )
        .expect_err("Gaussian ground domain accepted in Real family");
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::Rule)
    );
    assert_eq!(fault.location.operation, "finite coefficient evaluation");
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
