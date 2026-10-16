// SPDX-License-Identifier: AGPL-3.0-only
//! Adversarial lifecycles assert exact refusals, partial proof stages and costs.

use super::{oracle, support};
use crate::kernel_oracle::Rational;
use nrz_accuracy::BudgetDecision;
use nrz_matrix::{MomentAssurance, OperationError, SolveError, SolveStage, SolveStop};

/// A solvable rational system at the inverse-certificate edge must not be called singular.
/// ´test:integration:integrated-certificate-edge-preserves-success-and-refusal´
#[test]
fn integrated_certificate_edge_preserves_success_and_refusal() -> support::ResultOf {
    let case = support::edge()?;
    assert_eq!(case.storage(), [7, 18]);
    assert_eq!(case.samples.len(), 1);
    assert_eq!(case.refusals.len(), 1);
    assert_eq!(case.factors, 1);
    assert_eq!(case.provenance[0], 2);
    let truth = oracle::fractions(&[(1, 1), (0, 1)])?;
    oracle::values(&case.samples[0], &truth, &truth)?;
    let proof = case.samples[0].solve.as_ref().ok_or("accepted proof")?;
    assert_eq!(proof.work(), 1536);
    assert!(proof.inverse_defect() < 1.0);
    let failure = &case.refusals[0];
    assert!(matches!(
        failure.cause,
        OperationError::Solve(SolveError::CannotCertifyInverse)
    ));
    assert_eq!(failure.stage, SolveStage::Factorization);
    assert_eq!(failure.work, 1536);
    assert!(failure.proof.is_none());
    let adjacent = f64::from_bits(1.0_f64.to_bits() + 1);
    let exact = crate::solve_public::exact(&[1.0, 1.0, 1.0, adjacent], &[1.0, 1.0], 2)?;
    for (actual, expected) in exact.iter().zip(&truth) {
        assert_eq!(actual.subtract(*expected)?.numerator, 0);
    }
    Ok(())
}

/// Shared operator/RHS perturbations yield a negative cross covariance between RHS columns.
/// ´test:integration:integrated-shared-operator-rhs-retains-cross-terms´
#[test]
fn integrated_shared_operator_rhs_retains_cross_terms() -> support::ResultOf {
    let case = support::shared()?;
    assert_eq!(case.storage(), [15, 48]);
    assert_eq!(case.samples.len(), 1);
    assert_eq!(case.factors, 1);
    assert_eq!(case.provenance[0], 2);
    let sample = &case.samples[0];
    let truth = oracle::fractions(&[(1, 1), (2, 1)])?;
    oracle::values(sample, &truth, &truth)?;
    oracle::parameter(
        &sample.parameters[0],
        &oracle::fractions(&[(1, 16), (-1, 32), (-1, 32), (1, 64)])?,
    )?;
    let jacobian = oracle::jacobian(&[2.0], &[2.0, 4.0], 1)?;
    oracle::parameter(&sample.parameters[1], &jacobian)?;
    oracle::parameter(&sample.parameters[2], &jacobian)?;
    assert_eq!(sample.assurance, Some(MomentAssurance::LinearizedMoments));
    assert_eq!(sample.parameters.len(), 3);
    assert_eq!(sample.statistical_work, 2584);
    assert_eq!(
        sample.solve.as_ref().ok_or("proof")?.sensitivities().len(),
        3
    );
    assert_eq!(sample.solve.as_ref().ok_or("proof")?.work(), 384);
    assert_eq!(sample.parameters[0].shape, [2, 2]);
    Ok(())
}

/// Changed A has an independently solvable new problem, but cannot reuse the old resource.
/// ´test:integration:integrated-operator-change-refuses-stale-factor´
#[test]
fn integrated_operator_change_refuses_stale_factor() -> support::ResultOf {
    let case = support::stale()?;
    assert_eq!(case.storage(), [5, 12]);
    assert_eq!(case.factors, 1);
    assert_eq!(case.provenance[0], 2);
    let original = oracle::fractions(&[(1, 1), (2, 1)])?;
    oracle::values(&case.samples[0], &original, &original)?;
    assert_eq!(case.refusals.len(), 1);
    let failure = &case.refusals[0];
    assert!(matches!(
        failure.cause,
        OperationError::Solve(SolveError::ForeignFactor)
    ));
    assert_eq!(failure.stage, SolveStage::Factorization);
    assert_eq!(failure.work, 1536);
    assert!(failure.proof.is_none());
    let changed = crate::solve_public::exact(&[4.0, 1.0, 1.0, 2.0], &[5.0, 5.0], 2)?;
    for (actual, expected) in changed.iter().zip(oracle::fractions(&[(5, 7), (15, 7)])?) {
        assert_eq!(actual.subtract(expected)?.numerator, 0);
    }
    Ok(())
}

/// Refinement cannot publish an unrepresentable zero-error result and stops after stagnation.
/// ´test:integration:integrated-unmet-refinement-retains-bounded-attempt-records´
#[test]
fn integrated_unmet_refinement_retains_bounded_attempt_records() -> support::ResultOf {
    let case = support::stagnation()?;
    assert_eq!(case.storage(), [2, 2]);
    assert!(case.samples.is_empty());
    assert_eq!(case.provenance, [0; 5]);
    assert_eq!(case.factors, 1);
    assert_eq!(case.refusals.len(), 1);
    let failure = &case.refusals[0];
    assert!(matches!(
        failure.cause,
        OperationError::Solve(SolveError::Stagnation)
    ));
    assert_eq!(failure.stage, SolveStage::Evaluation);
    assert_eq!(failure.work, 256);
    let proof = failure
        .proof
        .as_ref()
        .ok_or("valid failed-budget theorem")?;
    assert!(proof.is_complete());
    let recovery = proof.recovery().ok_or("recovery")?;
    assert_eq!(recovery.stop(), SolveStop::Stagnation);
    assert_eq!(recovery.reserved_attempts(), 2);
    assert_eq!(recovery.attempts().len(), 2);
    assert_eq!(recovery.work(), 512);
    let exact = Rational::new(1, 3)?;
    assert_ne!(
        exact.subtract(Rational::binary(1.0_f64 / 3.0)?)?.numerator,
        0
    );
    for attempt in recovery.attempts() {
        assert_eq!(attempt.budget(), BudgetDecision::Exceeded);
        assert!(attempt.radius() > 0.0);
        assert!(crate::statistics_oracle::covered(
            exact,
            1.0 / 3.0,
            attempt.radius()
        )?);
    }
    Ok(())
}
